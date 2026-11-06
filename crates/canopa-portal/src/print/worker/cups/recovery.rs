use super::*;
use crate::print::{
    file_target::RecoveryStore,
    printer_jobs::{Intent, Receipt, Report, Server},
};
use std::{cell::RefCell, ffi::CStr, rc::Rc};

impl Api {
    pub(super) fn identity(&self) -> anyhow::Result<Server> {
        // SAFETY: these CUPS-owned NUL-terminated strings belong to the retained
        // library and are copied before the current worker changes any settings.
        let (name, user, port, encryption) = unsafe {
            let name = (self.server)();
            let user = (self.user)();
            anyhow::ensure!(
                !name.is_null() && !user.is_null(),
                "Printer server identity unavailable"
            );
            (
                CStr::from_ptr(name).to_bytes().to_vec(),
                CStr::from_ptr(user).to_bytes().to_vec(),
                (self.port)(),
                (self.encryption)(),
            )
        };
        Ok(Server {
            name,
            user,
            port,
            encryption,
        })
    }
}

pub(super) struct Journal {
    store: Option<RecoveryStore>,
    intent: RefCell<Intent>,
    uuid: RefCell<Option<String>>,
}
impl Journal {
    pub fn begin(
        session: &Session,
        destination: &Destination,
        title: &str,
        registration: Option<super::super::RecoveryRegistration>,
    ) -> anyhow::Result<Rc<Self>> {
        let intent = Intent::new(session.api.identity()?, destination.clone(), title)?;
        if let Some(registration) = &registration {
            registration
                .store
                .printer_begin(&intent, registration.owner)?;
        }
        Ok(Rc::new(Self {
            store: registration.map(|registration| registration.store),
            intent: RefCell::new(intent),
            uuid: RefCell::new(None),
        }))
    }
    pub fn name(&self) -> String {
        self.intent.borrow().name.clone()
    }
    pub fn creation_user(&self, user: &[u8]) -> anyhow::Result<()> {
        let mut intent = self.intent.borrow_mut();
        if let Some(store) = &self.store {
            store.printer_creation_user(&intent.key, user)?;
        }
        intent.server.user = user.to_vec();
        Ok(())
    }
    pub fn created(&self, job: i32) -> anyhow::Result<()> {
        if let Some(store) = &self.store {
            store.printer_created(&self.intent.borrow().key, job)?;
        }
        Ok(())
    }
    pub fn accepted(&self, job: i32) -> anyhow::Result<()> {
        if let Some(store) = &self.store {
            store.printer_accepted(&self.intent.borrow().key, job)?;
        }
        Ok(())
    }
    pub fn intent(&self) -> Intent {
        self.intent.borrow().clone()
    }
    pub fn identified(&self, job: i32, uuid: Option<&str>) -> anyhow::Result<()> {
        if let Some(uuid) = uuid {
            if let Some(store) = &self.store {
                store.printer_identified(&self.intent.borrow().key, job, uuid)?;
            }
            *self.uuid.borrow_mut() = Some(uuid.to_owned());
        }
        Ok(())
    }
    pub fn finish(&self) {
        if let Some(store) = &self.store
            && let Err(error) = store.printer_finish(&self.intent.borrow().key)
        {
            eprintln!("level=warn component=print event=printer_receipt_retained error={error}");
        }
    }
    pub fn retire(
        &self,
        job: i32,
        connections: &mut Option<connection::Connections>,
        withdrawn: bool,
    ) {
        let receipt = Receipt {
            intent: self.intent.borrow().clone(),
            job: (job > 0).then_some(job),
            accepted: false,
            uuid: self.uuid.borrow().clone(),
        };
        let authority = if withdrawn {
            CancellationAuthority::WithdrawnRequest
        } else {
            CancellationAuthority::EmptyIncoming
        };
        match retire(&receipt, connections, false, authority) {
            Ok(true) => self.finish(),
            Ok(false) => eprintln!("level=warn component=print event=printer_job_unidentified"),
            Err(error) => {
                eprintln!("level=warn component=print event=printer_job_retained error={error}")
            }
        }
    }
}

struct Jobs<'a> {
    api: &'a Api,
    count: c_int,
    raw: *mut RawJob,
}
impl Drop for Jobs<'_> {
    fn drop(&mut self) {
        if !self.raw.is_null() && self.count >= 0 {
            // SAFETY: the counted array was allocated by this retained API.
            unsafe {
                (self.api.free_jobs)(self.count, self.raw);
            }
        }
    }
}

pub(super) fn queue_matches(encoded: &[u8], queue: &[u8]) -> bool {
    let mut decoded = Vec::with_capacity(encoded.len());
    let mut offset = 0;
    while offset < encoded.len() {
        if encoded[offset] == b'%' {
            let Some(pair) = encoded.get(offset + 1..offset + 3) else {
                return false;
            };
            let Ok(pair) = std::str::from_utf8(pair) else {
                return false;
            };
            let Ok(byte) = u8::from_str_radix(pair, 16) else {
                return false;
            };
            decoded.push(byte);
            offset += 3;
        } else {
            decoded.push(encoded[offset]);
            offset += 1;
        }
    }
    decoded == queue
}

fn owned_jobs(session: &Session, intent: &Intent) -> anyhow::Result<Vec<(i32, i32)>> {
    let queue = CString::new(intent.destination.queue.as_str())?;
    let mut raw = ptr::null_mut();
    // SAFETY: a live connection and queue are retained until Jobs frees the
    // counted public cups_job_t array; only this user's selected queue is read.
    let count =
        unsafe { (session.api.jobs)(session.http.as_ptr(), &mut raw, queue.as_ptr(), 1, -1) };
    let jobs = Jobs {
        api: &session.api,
        count,
        raw,
    };
    // CUPS can return zero on transport failure too; an empty list must itself
    // be acknowledged before it can be interpreted as absence.
    let (status, http) = unsafe {
        (
            (session.api.last_error)(),
            (session.api.http_status)(session.http.as_ptr()),
        )
    };
    anyhow::ensure!(
        succeeded(status)
            && http == 200
            && (0..=4096).contains(&count)
            && (count == 0 || !raw.is_null()),
        "Printer job identities were not acknowledged"
    );
    if count == 0 {
        return Ok(vec![]);
    }
    // SAFETY: count initialized records are owned by Jobs for this whole loop.
    let records = unsafe { std::slice::from_raw_parts(jobs.raw, count as usize) };
    let mut matched = vec![];
    for record in records {
        if record.id <= 0
            || record.title.is_null()
            || record.user.is_null()
            || record.destination.is_null()
        {
            continue;
        }
        // SAFETY: string fields are public CUPS-owned strings in the live array.
        let owns = unsafe {
            CStr::from_ptr(record.title).to_bytes() == intent.name.as_bytes()
                && CStr::from_ptr(record.user).to_bytes() == intent.server.user
                && queue_matches(
                    CStr::from_ptr(record.destination).to_bytes(),
                    intent.destination.queue.as_bytes(),
                )
        };
        if owns {
            matched.push((record.id, record.state));
        }
    }
    anyhow::ensure!(
        matched.len() <= crate::print::printer_jobs::MAX_JOBS,
        "Too many jobs share a printer recovery identity"
    );
    Ok(matched)
}

fn profile(api: &Api, receipt: &Receipt, interactive: bool) -> anyhow::Result<()> {
    let current = api.identity()?;
    let recorded = &receipt.intent.server;
    anyhow::ensure!(
        current.name == recorded.name
            && current.port == recorded.port
            && current.encryption == recorded.encryption
            && (interactive || current.user == recorded.user),
        "Print server or user changed; printer record retained"
    );
    Ok(())
}
fn targets(
    session: &Session,
    receipt: &Receipt,
    interactive: bool,
    bindings: &mut Option<Vec<Receipt>>,
) -> anyhow::Result<Vec<(i32, i32)>> {
    if !interactive {
        return owned_jobs(session, &receipt.intent);
    }
    profile(&session.api, receipt, true)?;
    let selected = session.destination(&receipt.intent.destination, true)?;
    if bindings.is_none() {
        *bindings = Some(if receipt.uuid.is_some() {
            vec![receipt.clone()]
        } else {
            identity::Query::new(&selected, &receipt.intent.server)?.owned(session, receipt)?
        });
    }
    let mut result = vec![];
    for bound in bindings.as_ref().expect("bound recovery identities") {
        let job = bound.job.expect("identified job");
        let context = identity::Context::new(&selected, job, &bound.intent.server)?;
        let metadata = context.inspect(session, Some(&bound.intent.server.user))?;
        result.push((job, metadata.verify_receipt(bound, job)?));
    }
    profile(&session.api, receipt, true)?;
    Ok(result)
}

#[derive(Clone, Copy)]
enum CancellationAuthority {
    EmptyIncoming,
    WithdrawnRequest,
}

fn retire(
    receipt: &Receipt,
    connections: &mut Option<connection::Connections>,
    interactive: bool,
    authority: CancellationAuthority,
) -> anyhow::Result<bool> {
    receipt.validate()?;
    profile(&Api::load()?, receipt, interactive)?;
    let mut session = Session::open()?;
    profile(&session.api, receipt, interactive)?;
    let mut bindings = None;
    let mut challenges = 0;
    let mut reconnects = 0;
    loop {
        let jobs = targets(&session, receipt, interactive, &mut bindings)?;
        if jobs.is_empty() {
            return Ok(false);
        }
        let mut revalidate = false;
        for (job, state) in jobs {
            if [7, 8, 9].contains(&state) {
                continue;
            }
            let selected = session.destination(
                &receipt.intent.destination,
                interactive || matches!(authority, CancellationAuthority::EmptyIncoming),
            )?;
            anyhow::ensure!(
                targets(&session, receipt, interactive, &mut bindings)?
                    .iter()
                    .any(|(id, _)| *id == job),
                "Printer job identity changed"
            );
            // A failed or missing local receipt cannot revoke scheduler
            // acceptance. Only withdrawal of the original live request can
            // cancel a job that already contains its document. Recovery and
            // ordinary error cleanup can cancel only a proven empty job.
            if matches!(authority, CancellationAuthority::EmptyIncoming) {
                let bound = bindings
                    .as_ref()
                    .and_then(|items| items.iter().find(|item| item.job == Some(job)))
                    .unwrap_or(receipt);
                identity::Context::new(&selected, job, &receipt.intent.server)?
                    .inspect(&session, Some(&receipt.intent.server.user))?
                    .verify_incomplete(bound, job)?;
            }
            if connections.is_none() {
                *connections = Some(connection::Connections::new()?);
            }
            let guard = connections.as_ref().expect("initialized connection guard");
            // An explicit review must approve outside this mutation and query
            // the same job again after login. CUPS must not retry Cancel while
            // a password dialog has made the previous ownership check stale.
            let mutation = if interactive {
                Some(identity::Context::new(
                    &selected,
                    job,
                    &receipt.intent.server,
                )?)
            } else {
                None
            };
            let deferred = interactive
                .then(|| session.authentication.defer())
                .transpose()?;
            if interactive {
                // Only a new challenge from this operation may request login.
                // Metadata200 can leave CUPS's rejected password cached, and its
                // failed internal reconnect overwrites the final HTTP status.
                unsafe {
                    (session.api.protocol.set_header)(session.http.as_ptr(), 26, c"".as_ptr());
                }
            }
            let (acknowledged, changed) = guard.cancel(|| {
                if let Some(mutation) = &mutation {
                    mutation.cancel(&session).is_ok()
                } else {
                    succeeded(unsafe {
                        (session.api.cancel)(session.http.as_ptr(), selected.raw.as_ptr(), job)
                    })
                }
            })?;
            let callback_resource = deferred
                .as_ref()
                .and_then(authentication::Deferral::resource);
            drop(deferred);
            drop(selected);
            let challenge = if interactive && !acknowledged {
                discovery::string(unsafe {
                    (session.api.protocol.header)(session.http.as_ptr(), 26)
                })?
                .filter(|challenge| !challenge.is_empty())
            } else {
                None
            };
            let unauthorized = callback_resource.is_some()
                || unsafe { (session.api.http_status)(session.http.as_ptr()) } == 401
                || (changed && challenge.is_some());
            if interactive && unauthorized {
                anyhow::ensure!(
                    challenges < crate::print::authentication::MAX_ATTEMPTS,
                    "Too many recovery authentication challenges"
                );
                challenges += 1;
                let challenge = challenge
                    .ok_or_else(|| anyhow::anyhow!("Printer authentication challenge missing"))?;
                let resource = mutation.as_ref().expect("explicit mutation").resource();
                anyhow::ensure!(
                    callback_resource
                        .as_ref()
                        .is_none_or(|actual| actual.as_bytes() == resource.to_bytes()),
                    "Printer authentication resource changed"
                );
                let next = authentication::continue_session(&session, resource, &challenge)?;
                session = next;
                session.authentication.activate();
                revalidate = true;
                break;
            }
            if changed {
                reconnects += 1;
                anyhow::ensure!(
                    reconnects < 3,
                    "Printer connection kept changing; cancellation remains unconfirmed"
                );
                revalidate = true;
                break;
            }
            if !acknowledged {
                anyhow::ensure!(
                    targets(&session, receipt, interactive, &mut bindings)?
                        .iter()
                        .any(|(id, state)| *id == job && [7, 8, 9].contains(state)),
                    "Owned printer cancellation was not confirmed"
                );
            }
        }
        if revalidate {
            continue;
        }
        let verified = targets(&session, receipt, interactive, &mut bindings)?;
        return Ok(
            !verified.is_empty() && verified.iter().all(|(_, state)| [7, 8, 9].contains(state))
        );
    }
}

pub(crate) fn recover(
    store: &RecoveryStore,
    cancellation: &Cancellation,
    interactive: bool,
) -> anyhow::Result<Report> {
    let _credentials = (!interactive).then(crate::print::authentication::CacheOnly::enter);
    cancellation.check()?;
    let (receipts, busy) = store.printer_claim()?;
    let mut report = Report {
        busy,
        ..Report::default()
    };
    let mut connections = None;
    for receipt in receipts {
        cancellation.check()?;
        if interactive
            && crate::print::authentication::current().is_some_and(|channel| channel.declined())
        {
            anyhow::bail!("Printer recovery authentication cancelled");
        }
        // A durable final acknowledgement preserves an accepted job; startup
        // only forgets this metadata. It never retries or cancels that job.
        let retired = if receipt.accepted {
            true
        } else {
            match retire(
                &receipt,
                &mut connections,
                interactive,
                CancellationAuthority::EmptyIncoming,
            ) {
                Ok(retired) => retired,
                Err(error) => {
                    eprintln!(
                        "level=warn component=print_recovery event=printer_job_unconfirmed error={error}"
                    );
                    false
                }
            }
        };
        if retired {
            store.printer_finish(&receipt.intent.key)?;
            report.retired += 1;
        } else {
            report.unresolved += 1;
        }
    }
    cancellation.check()?;
    if interactive
        && crate::print::authentication::current().is_some_and(|channel| channel.declined())
    {
        anyhow::bail!("Printer recovery authentication cancelled");
    }
    report.validate()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scheduler_uri_queue_names_match_only_the_exact_decoded_queue() {
        assert!(queue_matches(b"Caf%C3%A9", "Café".as_bytes()));
        assert!(queue_matches(b"Fixture", b"Fixture"));
        assert!(!queue_matches(b"Fixture%2Fother", b"Fixture"));
        assert!(!queue_matches(b"Fixture%", b"Fixture"));
        assert!(!queue_matches(b"Fixture%XY", b"Fixture"));
    }
}
