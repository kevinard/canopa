//! One adapter owns the Iced presenter and shared bounded document worker.
use super::{Backend, Delivery, Rendering, presentation, render, worker::Client};
use crate::{
    print_ui::{Request, Target},
    process::DialogProcess,
};
use futures_util::future::BoxFuture;
use std::{os::fd::OwnedFd, sync::Arc};
use tokio::sync::watch;

pub struct Production {
    process: Arc<DialogProcess>,
    client: Client,
    job_reviews: Arc<tokio::sync::Semaphore>,
}
impl Production {
    pub fn new(process: Arc<DialogProcess>, recovery: super::file_target::RecoveryStore) -> Self {
        Self {
            client: Client::new((*process).clone()).with_recovery(recovery),
            job_reviews: Arc::new(tokio::sync::Semaphore::new(1)),
            process,
        }
    }
}
impl Backend for Production {
    fn review_jobs(
        &self,
        context: super::Context,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async move {
            let permit = self.job_reviews.clone().try_acquire_owned()?;
            super::jobs_review::present(self.process.clone(), context, cancelled, permit).await
        })
    }
    fn verify_file<'a>(
        &'a self,
        pin: &'a super::file_target::Pinned,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'a, anyhow::Result<Option<()>>> {
        Box::pin(self.client.verify_file(pin, cancelled))
    }
    fn review(
        &self,
        request: Request,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Option<presentation::Approval>>> {
        Box::pin(presentation::review(&self.process, request, cancelled))
    }
    fn snapshot(
        &self,
        source: OwnedFd,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Option<super::worker::Frozen>>> {
        Box::pin(self.client.snapshot(source, cancelled))
    }
    fn deliver(
        &self,
        delivery: Delivery,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Option<()>>> {
        Box::pin(async move {
            let Delivery {
                approval,
                document,
                rendering,
                title,
            } = delivery;
            let presentation::Approval {
                grant,
                file,
                authentication,
            } = approval;
            let printer = matches!(grant.target, Target::Printer(_));
            let duplex = printer
                && matches!(
                    grant.device.sides,
                    Some(super::worker::Sides::LongEdge | super::worker::Sides::ShortEdge)
                );
            grant.layout.validate(super::PageChoices {
                current: true,
                selection: true,
            })?;
            grant.page_setup.validate()?;
            anyhow::ensure!(
                grant.device.copies == grant.layout.copies,
                "Inconsistent approved copy count"
            );
            anyhow::ensure!(
                !printer || grant.layout.copies == 1 || grant.device.sides.is_some(),
                "Multiple printer copies require a known side setting"
            );
            if let Target::File {
                path, selection, ..
            } = &grant.target
            {
                anyhow::ensure!(
                    authentication.is_none(),
                    "Unexpected printer credentials for file output"
                );
                let pin = file
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Missing approved file destination"))?;
                anyhow::ensure!(
                    path == selection.path() && selection == pin.selection(),
                    "Inconsistent approved file destination"
                );
            }
            let output = match rendering {
                Rendering::Prepared
                    if printer
                        && (grant.layout.copies > 1
                            || matches!(
                                document.document.format,
                                Some(super::Format::Pdf | super::Format::Ps | super::Format::Svg)
                            )) =>
                {
                    // Normalize prepared printer streams even for one copy:
                    // printable annotations must become page content before
                    // delivery. Copy-only conversion keeps the client's layout.
                    let request = render::Request::copies(
                        grant.layout.copies,
                        grant.layout.collate,
                        duplex,
                        super::Format::Pdf,
                    );
                    let Some(rendered) = self
                        .client
                        .render(&document, request, cancelled.clone())
                        .await?
                    else {
                        return Ok(None);
                    };
                    rendered
                }
                Rendering::Prepared => document,
                Rendering::Direct => {
                    let format = match &grant.target {
                        Target::File { format, .. } => *format,
                        Target::Printer(_) => super::Format::Pdf,
                    };
                    let request = render::Request {
                        layout: grant.layout,
                        page_setup: grant.page_setup,
                        format,
                        mode: render::Mode::Layout,
                        duplex,
                    };
                    let Some(rendered) = self
                        .client
                        .render(&document, request, cancelled.clone())
                        .await?
                    else {
                        return Ok(None);
                    };
                    rendered
                }
            };
            match grant.target {
                Target::File { format, .. } => {
                    let pin =
                        file.ok_or_else(|| anyhow::anyhow!("Missing approved file destination"))?;
                    self.client
                        .publish_file(&output, pin, format, cancelled)
                        .await
                        .map(|result| result.map(|_| ()))
                }
                Target::Printer(destination) => {
                    anyhow::ensure!(file.is_none(), "Unexpected file destination for printer");
                    // Both paths now applied physical copies exactly once.
                    let mut device = grant.device;
                    device.copies = 1;
                    self.client
                        .clone()
                        .with_authentication(authentication)
                        .submit(&output, destination, title, device, cancelled)
                        .await
                        .map(|result| result.map(|_| ()))
                }
            }
        })
    }
    fn shutdown(&self) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            self.job_reviews.close();
            self.client.shutdown().await;
            if tokio::time::timeout(std::time::Duration::from_secs(15), async {
                while self.job_reviews.available_permits() != 1 {
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            })
            .await
            .is_err()
            {
                eprintln!("level=warn component=print_jobs event=shutdown_retirement_pending");
            }
        })
    }
}

#[cfg(test)]
mod tests;
