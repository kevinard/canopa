//! Best-effort reclamation of free GNU allocator pages after a surface retires.
use super::BannerUiMessage;
use std::sync::atomic::{AtomicBool, Ordering};

static RECLAIMING: AtomicBool = AtomicBool::new(false);

struct Reclaiming;

impl Drop for Reclaiming {
    fn drop(&mut self) {
        RECLAIMING.store(false, Ordering::Release);
    }
}

pub(super) fn reclaim() -> iced::Task<BannerUiMessage> {
    // At most one admitted job, including jobs queued in Tokio's blocking pool.
    if RECLAIMING
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        return iced::Task::none();
    }
    let guard = Reclaiming;
    let job = tokio::runtime::Handle::current().spawn_blocking(move || {
        let _guard = guard;
        // SAFETY: malloc_trim is MT-safe on GNU libc; pad=0 requests release of
        // unused pages without touching live allocations. This is not a signal handler.
        unsafe { libc::malloc_trim(0) };
    });
    iced::Task::perform(
        async move {
            let _ = job.await;
        },
        |_| BannerUiMessage::IdleMemoryReclaimed,
    )
}
