//! The backend receives live file pins from its own Iced helper, never a path grant.
use super::file_target::Pinned;
use crate::{
    print_ui::{Grant, Request, Target},
    process::DialogProcess,
};
use std::os::{
    fd::{AsFd, AsRawFd},
    unix::net::UnixStream,
};
use tokio::sync::watch;

pub struct Approval {
    pub grant: Grant,
    pub file: Option<Pinned>,
    pub(crate) authentication: Option<std::sync::Arc<super::authentication::Broker>>,
}

pub async fn review(
    process: &DialogProcess,
    mut request: Request,
    cancelled: watch::Receiver<bool>,
) -> anyhow::Result<Option<Approval>> {
    request.validate()?;
    let choices = request.prepare.page_choices;
    let formats = request.formats();
    let context = request.prepare.context.clone();
    let (channel, child) = UnixStream::pair()?;
    channel.set_nonblocking(true)?;
    request.channel = Some(child.as_raw_fd());
    let mut descriptors = vec![rustix::io::fcntl_dupfd_cloexec(&child, 3)?];
    // Use the duplicated channel number too, so the inherited set is exact.
    request.channel = Some(descriptors[0].as_raw_fd());
    request.cups_runtime =
        super::worker::runtime::Runtime::configured().inherit(&mut descriptors)?;
    let inherited = descriptors.iter().map(AsFd::as_fd).collect::<Vec<_>>();
    let grant: Option<Grant> = process
        .exchange_inheriting(
            "--print-dialog",
            request,
            cancelled.clone(),
            None,
            &inherited,
        )
        .await?;
    drop(descriptors);
    drop(child);
    let Some(grant) = grant else {
        return Ok(None);
    };
    if *cancelled.borrow() {
        return Ok(None);
    }
    grant.layout.validate(choices)?;
    grant.page_setup.validate()?;
    let (file, authentication) = match &grant.target {
        Target::Printer(destination) => {
            let retained = super::authentication::Retained::receive(&channel)?;
            anyhow::ensure!(
                retained
                    .entries
                    .iter()
                    .all(|entry| match &entry.challenge.kind {
                        super::authentication::Kind::Queue {
                            destination: current,
                            ..
                        } => current == destination,
                        _ => true,
                    }),
                "Credentials belong to another printer"
            );
            (
                None,
                Some(super::authentication::Broker::restore(
                    retained,
                    process.clone(),
                    context,
                )),
            )
        }
        Target::File {
            path,
            format,
            selection,
        } => {
            anyhow::ensure!(
                formats.contains(format) && path == selection.path(),
                "Invalid approved file output"
            );
            (
                Some(Pinned::receive_handoff(&channel, selection.clone())?),
                None,
            )
        }
    };
    if *cancelled.borrow() {
        return Ok(None);
    }
    Ok(Some(Approval {
        grant,
        file,
        authentication,
    }))
}

#[cfg(test)]
mod tests;
