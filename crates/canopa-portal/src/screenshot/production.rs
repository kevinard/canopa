use super::*;
use crate::screenshot_image::{DialogInput, Selection, Snapshot};
use std::os::fd::AsFd;

impl Production {
    pub(super) async fn capture_request(
        &self,
        mut request: ScreenshotRequest,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<Option<Outcome>> {
        let captured = match request.kind {
            Kind::Window => {
                let Some(target) = self.windows.choose(&request, cancelled.clone()).await? else {
                    return Ok(None);
                };
                self.windows.capture(target, cancelled.clone()).await?
            }
            Kind::ActiveWindow => {
                // Resolve before opening any helper; the review must never
                // become the active window that gets captured.
                let target = self.windows.focused().await?;
                self.windows.capture(target, cancelled.clone()).await?
            }
            _ => self.captures.capture(cancelled.clone()).await?,
        };
        let mut snapshot = Snapshot::prepare(captured, cancelled.clone()).await?;
        loop {
            let allow_window = request.allow_area
                && request.kind != Kind::Color
                && self.windows.available(&Connection::session().await?).await;
            let selected = if request.needs_review() {
                let input = DialogInput {
                    app_id: request.app_id.clone(),
                    parent_window: request.parent_window.clone(),
                    modal: request.modal,
                    kind: request.kind,
                    allow_area: request.allow_area,
                    allow_window,
                    source: snapshot.source(),
                };
                self.process
                    .exchange_inheriting::<_, Selection>(
                        "--screenshot-dialog",
                        input,
                        cancelled.clone(),
                        None,
                        &[snapshot.file().as_fd()],
                    )
                    .await?
            } else {
                Some(Selection::Image(None))
            };
            if *cancelled.borrow() {
                return Ok(None);
            }
            match selected {
                None => return Ok(None),
                Some(Selection::ChooseWindow) if allow_window => {
                    // A target change never publishes or crops the old screen
                    // snapshot. Only the explicitly chosen window is recorded.
                    drop(snapshot);
                    let Some(target) = self.windows.choose(&request, cancelled.clone()).await?
                    else {
                        return Ok(None);
                    };
                    let captured = self.windows.capture(target, cancelled.clone()).await?;
                    snapshot = Snapshot::prepare(captured, cancelled.clone()).await?;
                    request.kind = Kind::Window;
                    request.allow_area = false;
                }
                Some(Selection::Pixel { frame, x, y }) if request.kind == Kind::Color => {
                    return Ok(Some(Outcome::Color(snapshot.color(frame, x, y)?)));
                }
                Some(Selection::Image(rect))
                    if request.kind != Kind::Color
                        && (request.kind != Kind::Area || rect.is_some())
                        && (request.kind != Kind::Screen
                            || request.allow_area
                            || rect.is_none())
                        && (!matches!(request.kind, Kind::Window | Kind::ActiveWindow)
                            || rect.is_none()) =>
                {
                    let uri = snapshot
                        .publish(self.cache.clone(), rect, cancelled)
                        .await?;
                    return Ok(Some(Outcome::Image(uri)));
                }
                _ => anyhow::bail!("Invalid screenshot decision"),
            }
        }
    }
}
