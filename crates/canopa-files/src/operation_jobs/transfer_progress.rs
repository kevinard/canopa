//! Transfer totals, smoothed rate and remaining time (ADR0165).
//!
//! A cancellable background scan counts the selected entries without following
//! links. Its partial counts are visible while it runs ("counting"). The
//! estimator converts completed items and bytes into one work measure, so many
//! small files and one large file both get a stable estimate.
use std::{
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

/// Totals of the selection, possibly still being counted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TransferTotal {
    pub items: u64,
    pub bytes: u64,
    /// False while the scan is still counting; the numbers are then a floor.
    pub complete: bool,
}

#[derive(Default)]
struct Shared {
    items: AtomicU64,
    bytes: AtomicU64,
    complete: AtomicBool,
    stop: AtomicBool,
}

/// Counts the selection on its own thread. Dropping it stops the scan.
pub(super) struct Scan {
    shared: Arc<Shared>,
}

impl Scan {
    pub(super) fn start(sources: Vec<PathBuf>) -> Self {
        let shared = Arc::new(Shared::default());
        let worker = shared.clone();
        let spawned = std::thread::Builder::new()
            .name("transfer-scan".into())
            .spawn(move || {
                let mut pending = sources;
                while let Some(path) = pending.pop() {
                    if worker.stop.load(Ordering::Relaxed) {
                        return;
                    }
                    let Ok(metadata) = fs::symlink_metadata(&path) else {
                        continue;
                    };
                    worker.items.fetch_add(1, Ordering::Relaxed);
                    if metadata.is_file() {
                        worker.bytes.fetch_add(metadata.len(), Ordering::Relaxed);
                    } else if metadata.is_dir()
                        && let Ok(entries) = fs::read_dir(&path)
                    {
                        pending.extend(entries.flatten().map(|entry| entry.path()));
                    }
                }
                worker.complete.store(true, Ordering::Release);
            });
        if spawned.is_err() {
            // Without a scan, progress keeps its open-ended form.
            shared.stop.store(true, Ordering::Relaxed);
        }
        Self { shared }
    }

    pub(super) fn total(&self) -> Option<TransferTotal> {
        let complete = self.shared.complete.load(Ordering::Acquire);
        if self.shared.stop.load(Ordering::Relaxed) && !complete {
            return None;
        }
        Some(TransferTotal {
            items: self.shared.items.load(Ordering::Relaxed),
            bytes: self.shared.bytes.load(Ordering::Relaxed),
            complete,
        })
    }
}

impl Drop for Scan {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
    }
}

/// One item costs about as much as this many bytes of content: the journal,
/// identity checks and barriers dominate small files.
const ITEM_WORK: f64 = 256.0 * 1024.0;
/// Smoothing time constant for the rate.
const RATE_TAU: f64 = 4.0;
/// Smoothing time constant for the remaining time shown.
const REMAINING_TAU: f64 = 3.0;
/// No estimate before this much observation.
const WARMUP: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Estimate {
    pub bytes_per_second: Option<f64>,
    pub remaining: Option<Duration>,
}

/// Exponentially smoothed work rate. Each sample covers the interval since the
/// previous one, so the 4 Hz publication cadence sets the resolution.
#[derive(Debug)]
pub(super) struct Estimator {
    start: Instant,
    last: Option<(Instant, f64, u64)>,
    work_rate: Option<f64>,
    byte_rate: Option<f64>,
    remaining: Option<f64>,
}

fn work(items: u64, bytes: u64) -> f64 {
    items as f64 * ITEM_WORK + bytes as f64
}

impl Estimator {
    pub(super) fn new(start: Instant) -> Self {
        Self {
            start,
            last: None,
            work_rate: None,
            byte_rate: None,
            remaining: None,
        }
    }

    pub(super) fn sample(
        &mut self,
        now: Instant,
        items: u64,
        bytes: u64,
        total: Option<TransferTotal>,
    ) -> Estimate {
        let done = work(items, bytes);
        let (since, before, bytes_before) = self.last.unwrap_or((self.start, 0.0, 0));
        let elapsed = now.saturating_duration_since(since).as_secs_f64();
        if elapsed > 0.0 {
            let blend = |old: Option<f64>, new: f64| {
                let alpha = 1.0 - (-elapsed / RATE_TAU).exp();
                Some(old.map_or(new, |old| old + alpha * (new - old)))
            };
            self.work_rate = blend(self.work_rate, (done - before).max(0.0) / elapsed);
            self.byte_rate = blend(
                self.byte_rate,
                bytes.saturating_sub(bytes_before) as f64 / elapsed,
            );
            self.last = Some((now, done, bytes));
        }
        let warmed = now.saturating_duration_since(self.start) >= WARMUP;
        let estimate = match (total, self.work_rate) {
            (Some(total), Some(rate)) if total.complete && warmed && rate > 0.0 => {
                let left = (work(total.items, total.bytes) - done).max(0.0) / rate;
                // Count down between samples and absorb new estimates slowly,
                // so a burst of small files does not make the time jump.
                let alpha = 1.0 - (-elapsed / REMAINING_TAU).exp();
                Some(self.remaining.map_or(left, |shown| {
                    let shown = (shown - elapsed).max(0.0);
                    shown + alpha * (left - shown)
                }))
            }
            _ => None,
        };
        self.remaining = estimate;
        Estimate {
            bytes_per_second: self.byte_rate.filter(|_| warmed),
            remaining: estimate.map(Duration::from_secs_f64),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn total(items: u64, bytes: u64) -> Option<TransferTotal> {
        Some(TransferTotal {
            items,
            bytes,
            complete: true,
        })
    }

    #[test]
    fn steady_work_gives_a_steady_countdown() {
        let start = Instant::now();
        let mut estimator = Estimator::new(start);
        let mut shown = Vec::new();
        // 8 items and 8 MiB per second, 80 items and 80 MiB in total.
        for tick in 1..=40u64 {
            let now = start + Duration::from_millis(250 * tick);
            let items = tick * 2;
            let estimate = estimator.sample(now, items, items << 20, total(80, 80 << 20));
            if tick < 8 {
                assert!(estimate.remaining.is_none(), "no estimate during warmup");
            } else {
                shown.push(estimate.remaining.unwrap().as_secs_f64());
                let rate = estimate.bytes_per_second.unwrap();
                assert!((rate - 8.0 * 1024.0 * 1024.0).abs() < 1024.0, "{rate}");
            }
        }
        for (tick, remaining) in (8..=40u64).zip(&shown) {
            let actual = 10.0 - tick as f64 / 4.0;
            assert!(
                (remaining - actual).abs() < 0.01,
                "{tick}: {remaining} vs {actual}"
            );
        }
    }

    #[test]
    fn a_burst_moves_the_estimate_gradually() {
        let start = Instant::now();
        let mut estimator = Estimator::new(start);
        let mut last = None;
        for tick in 1..=20u64 {
            // Steady 1 MiB per 250 ms, then one tick copies 20 MiB at once.
            let bytes = if tick < 16 { tick } else { tick + 20 } << 20;
            let now = start + Duration::from_millis(250 * tick);
            let estimate = estimator.sample(now, 0, bytes, total(0, 200 << 20));
            if let (Some(previous), Some(current)) = (last, estimate.remaining) {
                let change: f64 = current.as_secs_f64() - Duration::as_secs_f64(&previous);
                assert!(change.abs() < 12.0, "tick {tick}: jumped by {change}");
            }
            last = estimate.remaining;
        }
    }

    #[test]
    fn counting_and_unknown_totals_have_no_remaining_time() {
        let start = Instant::now();
        let mut estimator = Estimator::new(start);
        let counting = Some(TransferTotal {
            items: 5,
            bytes: 5,
            complete: false,
        });
        let later = start + Duration::from_secs(3);
        assert!(estimator.sample(later, 1, 1, counting).remaining.is_none());
        assert!(
            estimator
                .sample(later + Duration::from_secs(1), 2, 2, None)
                .remaining
                .is_none()
        );
    }

    #[test]
    fn the_scan_counts_entries_without_following_links_and_stops_on_drop() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("folder");
        fs::create_dir_all(folder.join("nested")).unwrap();
        fs::write(folder.join("a"), b"12345").unwrap();
        fs::write(folder.join("nested/b"), b"123").unwrap();
        std::os::unix::fs::symlink("/", folder.join("link")).unwrap();
        let scan = Scan::start(vec![folder, root.path().join("missing")]);
        let deadline = Instant::now() + Duration::from_secs(10);
        let total = loop {
            let total = scan.total().unwrap();
            if total.complete || Instant::now() > deadline {
                break total;
            }
            std::thread::yield_now();
        };
        assert_eq!(
            total,
            TransferTotal {
                items: 5,
                bytes: 8,
                complete: true
            }
        );
        drop(scan);
        let stopped = Scan::start(vec![root.path().to_owned()]);
        stopped.shared.stop.store(true, Ordering::Relaxed);
        drop(stopped);
    }
}
