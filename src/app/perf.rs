use std::collections::BTreeMap;
use std::sync::Mutex;

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct PerfSnapshot {
    pub(super) updates: BTreeMap<&'static str, u64>,
    pub(super) views: BTreeMap<String, u64>,
    pub(super) workspace_rebuilds: u64,
    pub(super) tray_events: u64,
}

#[derive(Default)]
pub(super) struct PerfTracker {
    stats: Mutex<PerfSnapshot>,
}

impl PerfTracker {
    pub(super) fn from_env() -> Option<Self> {
        match std::env::var("NIRIBAR_PERF") {
            Ok(value) if matches!(value.as_str(), "1" | "true" | "yes" | "on") => {
                Some(Self::default())
            }
            _ => None,
        }
    }

    pub(super) fn record_update(&self, label: &'static str) {
        let mut stats = self.stats.lock().expect("perf stats mutex poisoned");
        *stats.updates.entry(label).or_default() += 1;
    }

    pub(super) fn record_view(&self, window: iced::window::Id) {
        let mut stats = self.stats.lock().expect("perf stats mutex poisoned");
        *stats.views.entry(format!("{window:?}")).or_default() += 1;
    }

    pub(super) fn record_workspace_rebuild(&self) {
        let mut stats = self.stats.lock().expect("perf stats mutex poisoned");
        stats.workspace_rebuilds += 1;
    }

    pub(super) fn record_tray_event(&self) {
        let mut stats = self.stats.lock().expect("perf stats mutex poisoned");
        stats.tray_events += 1;
    }

    pub(super) fn snapshot_and_reset(&self) -> PerfSnapshot {
        let mut stats = self.stats.lock().expect("perf stats mutex poisoned");
        std::mem::take(&mut *stats)
    }
}

pub(super) fn format_snapshot(snapshot: &PerfSnapshot) -> String {
    let updates = snapshot
        .updates
        .iter()
        .map(|(label, count)| format!("{label}={count}"))
        .collect::<Vec<_>>()
        .join(", ");
    let views = snapshot
        .views
        .iter()
        .map(|(window, count)| format!("{window}={count}"))
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        "niribar perf: updates [{}]; views [{}]; workspace_rebuilds={}; tray_events={}",
        updates, views, snapshot.workspace_rebuilds, snapshot.tray_events
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracker_records_counts_and_resets_snapshot() {
        let tracker = PerfTracker::default();
        let window = iced::window::Id::unique();

        tracker.record_update("Tick");
        tracker.record_update("Tick");
        tracker.record_update("Volume");
        tracker.record_view(window);
        tracker.record_workspace_rebuild();
        tracker.record_tray_event();

        let snapshot = tracker.snapshot_and_reset();
        assert_eq!(snapshot.updates.get("Tick"), Some(&2));
        assert_eq!(snapshot.updates.get("Volume"), Some(&1));
        assert_eq!(snapshot.views.values().sum::<u64>(), 1);
        assert_eq!(snapshot.workspace_rebuilds, 1);
        assert_eq!(snapshot.tray_events, 1);
        assert_eq!(tracker.snapshot_and_reset(), PerfSnapshot::default());
    }

    #[test]
    fn format_snapshot_includes_counter_groups() {
        let mut snapshot = PerfSnapshot::default();
        snapshot.updates.insert("Tick", 3);
        snapshot.views.insert("Id(1)".into(), 2);
        snapshot.workspace_rebuilds = 1;
        snapshot.tray_events = 4;

        let line = format_snapshot(&snapshot);

        assert!(line.contains("updates [Tick=3]"));
        assert!(line.contains("views [Id(1)=2]"));
        assert!(line.contains("workspace_rebuilds=1"));
        assert!(line.contains("tray_events=4"));
    }
}
