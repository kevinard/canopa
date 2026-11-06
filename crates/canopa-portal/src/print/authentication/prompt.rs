//! The authentication helper's ordinary output contains only an approval bit.
//! Credentials use its inherited private stream, never JSON or the command line.
use super::{Challenge, Credentials, MAX_FRAME, Secret, broker::Presenter};
use crate::{print::Context, print_authentication_ui::Request, process::DialogProcess};
use futures_util::future::BoxFuture;
use std::{
    os::{
        fd::{AsFd, AsRawFd},
        unix::net::UnixStream,
    },
    time::Duration,
};
use tokio::{io::AsyncReadExt, sync::watch};

pub(super) struct Dialog {
    pub process: DialogProcess,
    pub context: Context,
}
impl Presenter for Dialog {
    fn show(
        &self,
        challenge: Challenge,
        mut cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Option<Credentials>>> {
        Box::pin(async move {
            let helper_cancelled = cancelled.clone();
            let interaction = async {
                let (parent, child) = UnixStream::pair()?;
                parent.set_nonblocking(true)?;
                let request = Request {
                    challenge: challenge.clone(),
                    context: self.context.clone(),
                    channel: child.as_raw_fd(),
                };
                let result = self
                    .process
                    .exchange_inheriting::<_, bool>(
                        "--print-authentication-dialog",
                        request,
                        helper_cancelled.clone(),
                        None,
                        &[child.as_fd()],
                    )
                    .await?;
                drop(child);
                if result != Some(true) || *helper_cancelled.borrow() {
                    return Ok(None);
                }
                let mut channel = tokio::net::UnixStream::from_std(parent)?;
                let mut length = [0; 4];
                channel.read_exact(&mut length).await?;
                let length = u32::from_be_bytes(length) as usize;
                anyhow::ensure!(
                    (6..=MAX_FRAME).contains(&length),
                    "Invalid printer credential reply"
                );
                let mut frame = Secret::buffer(length);
                channel.read_exact(frame.bytes_mut()).await?;
                let mut extra = [0];
                anyhow::ensure!(
                    channel.read(&mut extra).await? == 0,
                    "Trailing printer credentials"
                );
                Credentials::decode(&challenge, frame.bytes())
            };
            // Approval and helper exit do not complete the interaction: the
            // private length, body and final EOF share its original deadline
            // and caller lifetime. A retained writer must not extend consent.
            tokio::select! {
                biased;
                _ = async { if !*cancelled.borrow() { let _ = cancelled.changed().await; } } => Ok(None),
                result = tokio::time::timeout(Duration::from_secs(90), interaction) => {
                    result.map_err(|_| anyhow::anyhow!("Printer authentication timed out"))?
                }
            }
        })
    }
}

#[cfg(test)]
mod tests;
