//! Print request/approval ownership, independent of toolkit rendering policy.
mod jobs;
#[cfg(test)]
mod native;
mod policy;
pub use jobs::PrinterJobs;
#[cfg(test)]
mod tests;
use super::{
    Format, PrepareRequest, PrintOptions,
    authority::Owner,
    presentation::Approval,
    tokens::{Issued, Tokens},
    worker::Frozen,
};
use crate::{
    lockdown::{Lockdown, Property},
    print_ui::{Purpose, Request, Target},
    requests::{Completion, Options, Requests},
};
use futures_util::future::BoxFuture;
use policy::{Permit, Scope};
use std::{os::fd::OwnedFd, sync::Arc};
use tokio::sync::watch;
use zbus::{
    Connection, fdo,
    message::Header,
    zvariant::{OwnedObjectPath, OwnedValue, Value},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rendering {
    Prepared,
    Direct,
}

/// The backend receives the complete approved layout together with explicit
/// rendering provenance. Prepared printer bodies contain one formatted copy;
/// the backend applies physical copies without repeating the client's layout.
pub struct Delivery {
    pub approval: Approval,
    pub document: Frozen,
    pub rendering: Rendering,
    pub title: String,
}
pub trait Backend: Send + Sync + 'static {
    fn review_jobs(
        &self,
        _context: super::Context,
        _cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { anyhow::bail!("Printer job review is unavailable") })
    }
    fn verify_file<'a>(
        &'a self,
        pin: &'a super::file_target::Pinned,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, anyhow::Result<Option<()>>>;
    fn review(
        &self,
        request: Request,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Option<Approval>>>;
    fn snapshot(
        &self,
        source: OwnedFd,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Option<Frozen>>>;
    fn deliver(
        &self,
        delivery: Delivery,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Option<()>>>;
    fn shutdown(&self) -> BoxFuture<'_, ()>;
}

struct Prepared {
    approval: Approval,
    permit: Permit,
}
struct Inner {
    backend: Arc<dyn Backend>,
    requests: Requests,
    tokens: Arc<Tokens<Prepared>>,
    policy: Lockdown,
    shutdown: watch::Sender<bool>,
}
impl Drop for Inner {
    fn drop(&mut self) {
        self.shutdown.send_replace(true);
        self.tokens.clear();
    }
}
#[derive(Clone)]
pub struct Print(Arc<Inner>);

struct Preparation {
    values: Options,
    token: Issued<Prepared>,
}

impl Print {
    pub fn with_requests(backend: Arc<dyn Backend>, policy: Lockdown, requests: Requests) -> Self {
        let (shutdown, _) = watch::channel(false);
        Self(Arc::new(Inner {
            backend,
            requests,
            tokens: Arc::default(),
            policy,
            shutdown,
        }))
    }
    pub async fn shutdown(&self) {
        self.0.shutdown.send_replace(true);
        self.0.tokens.clear();
        self.0.backend.shutdown().await;
    }
    fn begin(&self, file: bool) -> fdo::Result<Permit> {
        let observed = self.0.policy.observe();
        let initial = *observed.borrow();
        let permit = Permit::new(observed, self.0.shutdown.subscribe(), initial, file);
        if !permit.current() {
            return Err(fdo::Error::AccessDenied(
                "Printing disabled or unavailable".into(),
            ));
        }
        Ok(permit)
    }
    async fn prepare(
        &self,
        owner: Owner,
        mut request: PrepareRequest,
        cancelled: watch::Receiver<bool>,
    ) -> fdo::Result<Option<Preparation>> {
        let observed = self.0.policy.observe();
        let initial = *observed.borrow();
        if initial.disabled(Property::SaveToDisk) {
            request.formats.clear();
        }
        let permit = Permit::new(
            observed.clone(),
            self.0.shutdown.subscribe(),
            initial,
            !request.formats.is_empty(),
        );
        if !permit.current() {
            return Err(fdo::Error::AccessDenied(
                "Printing disabled or unavailable".into(),
            ));
        }
        let scope = Scope::new(cancelled, owner.cancelled(), permit.cancelled());
        let input = Request {
            prepare: request.clone(),
            purpose: Purpose::Prepare,
            channel: None,
            cups_runtime: Default::default(),
        };
        let Some(approval) = self
            .0
            .backend
            .review(input, scope.cancelled())
            .await
            .map_err(failed)?
        else {
            return Ok(None);
        };
        validate(&approval, &request, None).map_err(failed)?;
        if let Some(pin) = &approval.file
            && self
                .0
                .backend
                .verify_file(pin, scope.cancelled())
                .await
                .map_err(failed)?
                .is_none()
        {
            return Ok(None);
        }
        if !permit.current() || *scope.cancelled().borrow() || !owner.confirm().await {
            return Ok(None);
        }
        // Save-to-disk policy need only outlive this preparation when a file was
        // actually approved. Printer tokens survive unrelated save-policy changes.
        let retained = Permit::new(
            observed,
            self.0.shutdown.subscribe(),
            initial,
            matches!(approval.grant.target, Target::File { .. }),
        );
        if !retained.current() {
            return Ok(None);
        }
        let values = results(&approval, request.page_choices).map_err(failed)?;
        let revoked = retained.cancelled();
        let token = self
            .0
            .tokens
            .issue_retained(
                owner,
                Prepared {
                    approval,
                    permit: retained,
                },
                revoked,
            )
            .await?;
        if *scope.cancelled().borrow() || !permit.current() {
            return Ok(None);
        }
        Ok(Some(Preparation { values, token }))
    }

    async fn perform(
        &self,
        owner: Owner,
        options: PrintOptions,
        source: OwnedFd,
        cancelled: watch::Receiver<bool>,
    ) -> fdo::Result<Option<()>> {
        let printing = self.begin(false)?;
        let prepared = match options.token {
            Some(token) => self.0.tokens.consume(&owner, token).await?,
            None => None,
        };
        if prepared
            .as_ref()
            .is_some_and(|prepared| !prepared.permit.current())
        {
            return Ok(None);
        }
        let permission = prepared.as_ref().map_or_else(
            || printing.cancelled(),
            |prepared| prepared.permit.cancelled(),
        );
        let scope = Scope::new(cancelled.clone(), owner.cancelled(), permission);
        let Some(document) = self
            .0
            .backend
            .snapshot(source, scope.cancelled())
            .await
            .map_err(failed)?
        else {
            return Ok(None);
        };
        if !printing.current() || *scope.cancelled().borrow() || !owner.confirm().await {
            return Ok(None);
        }
        let (approval, permit, rendering) = match prepared {
            Some(prepared) => {
                if !prepared.permit.current() {
                    return Ok(None);
                }
                if let Target::File { format, .. } = &prepared.approval.grant.target
                    && document.document.format != Some(*format)
                {
                    return Err(super::invalid(
                        "Document does not match the prepared file format",
                    ));
                }
                (prepared.approval, prepared.permit, Rendering::Prepared)
            }
            None => {
                let mut request = PrepareRequest::parse(
                    options.context.app_id.clone(),
                    options.context.parent_window.clone(),
                    options.context.title.clone(),
                    Options::new(),
                    Options::new(),
                    Options::new(),
                )?;
                request.context = options.context.clone();
                request.formats = options.formats;
                let observed = self.0.policy.observe();
                let initial = *observed.borrow();
                if initial.disabled(Property::SaveToDisk) {
                    request.formats.clear();
                }
                let pending = Permit::new(
                    observed.clone(),
                    self.0.shutdown.subscribe(),
                    initial,
                    !request.formats.is_empty(),
                );
                if !pending.current() {
                    return Ok(None);
                }
                let review_scope =
                    Scope::new(scope.cancelled(), owner.cancelled(), pending.cancelled());
                let format = document.document.format;
                let Some(approval) = self
                    .0
                    .backend
                    .review(
                        Request {
                            prepare: request.clone(),
                            purpose: Purpose::Direct { format },
                            channel: None,
                            cups_runtime: Default::default(),
                        },
                        review_scope.cancelled(),
                    )
                    .await
                    .map_err(failed)?
                else {
                    return Ok(None);
                };
                validate(&approval, &request, Some(format)).map_err(failed)?;
                if !pending.current() || *review_scope.cancelled().borrow() {
                    return Ok(None);
                }
                let permit = Permit::new(
                    observed,
                    self.0.shutdown.subscribe(),
                    initial,
                    matches!(approval.grant.target, Target::File { .. }),
                );
                (approval, permit, Rendering::Direct)
            }
        };
        let scope = Scope::new(scope.cancelled(), owner.cancelled(), permit.cancelled());
        if !owner.confirm().await
            || !printing.current()
            || !permit.current()
            || *scope.cancelled().borrow()
        {
            return Ok(None);
        }
        if let Some(pin) = &approval.file
            && self
                .0
                .backend
                .verify_file(pin, scope.cancelled())
                .await
                .map_err(failed)?
                .is_none()
        {
            return Ok(None);
        }
        let delivery = Delivery {
            approval,
            document,
            rendering,
            title: options.context.title,
        };
        // Retain both permits and the request scope until the worker returns.
        // Drop still signals independent worker cleanup if Requests cancels us.
        self.0
            .backend
            .deliver(delivery, scope.cancelled())
            .await
            .map_err(failed)
    }
}

fn failed(error: impl std::fmt::Display) -> fdo::Error {
    fdo::Error::Failed(format!("Print operation failed: {error}"))
}
fn validate(
    approval: &Approval,
    request: &PrepareRequest,
    direct: Option<Option<Format>>,
) -> anyhow::Result<()> {
    approval.grant.layout.validate(request.page_choices)?;
    approval.grant.page_setup.validate()?;
    anyhow::ensure!(
        approval.grant.device.copies == approval.grant.layout.copies,
        "Inconsistent approved copies"
    );
    match (&approval.grant.target, &approval.file) {
        (Target::Printer(destination), None) => destination.validate()?,
        (
            Target::File {
                path,
                format,
                selection,
            },
            Some(pin),
        ) => {
            anyhow::ensure!(
                request.formats.contains(format)
                    && direct.is_none_or(|actual| actual.is_some())
                    && path == selection.path()
                    && selection == pin.selection(),
                "Invalid approved file destination"
            );
        }
        _ => anyhow::bail!("Invalid retained print approval"),
    }
    Ok(())
}
fn results(approval: &Approval, choices: super::PageChoices) -> anyhow::Result<Options> {
    let mut layout = approval.grant.layout.clone();
    if matches!(approval.grant.target, Target::Printer(_)) {
        // The app formats one copy. The retained approval drives physical
        // copy grouping in the backend, including duplex boundaries.
        layout.copies = 1;
    }
    let mut settings = layout.options(choices)?;
    if let Target::Printer(destination) = &approval.grant.target {
        settings.insert(
            "printer".into(),
            Value::from(destination.name()).try_into_owned()?,
        );
        for (key, value) in approval.grant.device.settings.renderer_hints()? {
            settings.insert(key.into(), Value::from(value).try_into_owned()?);
        }
        if let Some(sides) = approval.grant.device.sides {
            let value = match sides {
                super::worker::Sides::OneSided => "simplex",
                super::worker::Sides::LongEdge => "vertical",
                super::worker::Sides::ShortEdge => "horizontal",
            };
            settings.insert("duplex".into(), Value::from(value).try_into_owned()?);
        }
        if let Some(color) = approval.grant.device.color {
            settings.insert(
                "use-color".into(),
                Value::from(if color { "true" } else { "false" }).try_into_owned()?,
            );
        }
    }
    if let Target::File { format, .. } = approval.grant.target {
        settings.insert(
            "printer".into(),
            Value::from("Print to File").try_into_owned()?,
        );
        settings.insert(
            "output-file-format".into(),
            Value::from(format.name()).try_into_owned()?,
        );
    }
    Ok(Options::from([
        (
            "settings".into(),
            OwnedValue::try_from(Value::from(settings))?,
        ),
        (
            "page-setup".into(),
            OwnedValue::try_from(Value::from(approval.grant.page_setup.options()?))?,
        ),
    ]))
}

#[zbus::interface(name = "org.freedesktop.impl.portal.Print")]
impl Print {
    #[allow(clippy::too_many_arguments)]
    async fn prepare_print(
        &self,
        handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        title: String,
        settings: Options,
        page_setup: Options,
        options: Options,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<(u32, Options)> {
        if header.primary().body_len() > super::MAX_REQUEST_BYTES {
            return Err(super::invalid("Print request exceeds limits"));
        }
        let request =
            PrepareRequest::parse(app_id, parent_window, title, settings, page_setup, options)?;
        let identity_handle = handle.clone();
        let outcome = self
            .0
            .requests
            .run(connection, header, handle, |peer, cancelled| async move {
                let owner = Owner::authenticate(
                    connection,
                    peer,
                    &identity_handle,
                    &request.context.app_id,
                )
                .await?;
                self.prepare(owner, request, cancelled).await
            })
            .await?;
        match outcome {
            Completion::Cancelled | Completion::Finished(Ok(None)) => Ok((1, Options::new())),
            Completion::Finished(Ok(Some(mut prepared))) => {
                prepared
                    .values
                    .insert("token".into(), prepared.token.publish().into());
                Ok((0, prepared.values))
            }
            Completion::Finished(Err(fdo::Error::Failed(_))) => Ok((2, Options::new())),
            Completion::Finished(Err(error)) => Err(error),
        }
    }
    #[allow(clippy::too_many_arguments)]
    async fn print(
        &self,
        handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        title: String,
        fd: zbus::zvariant::OwnedFd,
        options: Options,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<(u32, Options)> {
        if header.primary().body_len() > super::MAX_REQUEST_BYTES {
            return Err(super::invalid("Print request exceeds limits"));
        }
        let options = PrintOptions::parse(app_id, parent_window, title, options)?;
        let identity_handle = handle.clone();
        let outcome = self
            .0
            .requests
            .run(connection, header, handle, |peer, cancelled| async move {
                let owner = Owner::authenticate(
                    connection,
                    peer,
                    &identity_handle,
                    &options.context.app_id,
                )
                .await?;
                self.perform(owner, options, fd.into(), cancelled).await
            })
            .await?;
        match outcome {
            Completion::Cancelled | Completion::Finished(Ok(None)) => Ok((1, Options::new())),
            Completion::Finished(Ok(Some(()))) => Ok((0, Options::new())),
            Completion::Finished(Err(fdo::Error::Failed(_))) => Ok((2, Options::new())),
            Completion::Finished(Err(error)) => Err(error),
        }
    }
}
