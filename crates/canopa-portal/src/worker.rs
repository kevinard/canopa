//! Caller-owned preparation performed away from the Iced event loop.
pub(crate) fn run<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> impl Future<Output = Result<T, String>> {
    let (sender, receiver) = futures_channel::oneshot::channel();
    let _ = std::thread::Builder::new()
        .name("portal-prepare".into())
        .spawn(move || {
            let _ = sender.send(work());
        });
    async {
        receiver
            .await
            .map_err(|_| "Preparation worker stopped".into())
    }
}
