//! Input-only surfaces behind the active Panel, including outputs without a Bar.
use std::collections::BTreeMap;

use iced::window::Id;

#[derive(Default)]
pub(super) struct DismissalSurfaces {
    outputs: Option<Vec<String>>,
    owner: Option<Id>,
    windows: BTreeMap<Option<String>, Id>,
}

pub(in crate::app) struct Changes {
    pub(in crate::app) remove: Vec<Id>,
    pub(in crate::app) open: Vec<(Id, Option<String>)>,
}

impl DismissalSurfaces {
    pub(super) fn windows(&self) -> impl Iterator<Item = Id> + '_ {
        self.windows.values().copied()
    }

    fn reconcile(&mut self, owner: Option<Id>, fallback_output: Option<String>) -> Changes {
        let mut changes = Changes {
            remove: Vec::new(),
            open: Vec::new(),
        };
        if self.owner != owner {
            changes.remove.extend(self.windows.values().copied());
            self.windows.clear();
            self.owner = owner;
        }
        let outputs = if owner.is_some() {
            self.outputs
                .as_ref()
                .map(|outputs| outputs.iter().cloned().map(Some).collect())
                .unwrap_or_else(|| vec![fallback_output])
        } else {
            Vec::new()
        };
        self.windows.retain(|output, window| {
            if outputs.contains(output) {
                true
            } else {
                changes.remove.push(*window);
                false
            }
        });
        for output in outputs {
            self.windows.entry(output.clone()).or_insert_with(|| {
                let window = Id::unique();
                changes.open.push((window, output));
                window
            });
        }
        changes
    }
}

impl super::Foreground {
    pub(in crate::app) fn remember_dismissal_outputs(&mut self, mut outputs: Vec<String>) {
        outputs.retain(|output| !output.is_empty());
        outputs.sort();
        outputs.dedup();
        self.dismissal.outputs = Some(outputs);
    }

    pub(in crate::app) fn is_dismissal_window(&self, window: Id) -> bool {
        self.dismissal.windows().any(|id| id == window)
    }

    pub(in crate::app) fn reconcile_dismissal_surfaces(&mut self) -> Changes {
        let output = self
            .active_panel_tree()
            .map(|tree| tree.origin_output_name.clone())
            .filter(|output| !output.is_empty());
        self.dismissal.reconcile(self.active_panel_window(), output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dismissal_follows_panel_replacement_close_and_output_changes() {
        let mut surfaces = DismissalSurfaces {
            outputs: Some(vec!["DP-1".into(), "DP-2".into()]),
            ..Default::default()
        };
        let first = Id::unique();
        let opened = surfaces.reconcile(Some(first), None);
        assert_eq!(opened.open.len(), 2);
        assert!(surfaces.reconcile(Some(first), None).open.is_empty());

        surfaces.outputs = Some(vec!["DP-2".into(), "DP-3".into()]);
        let changed = surfaces.reconcile(Some(first), None);
        assert_eq!(changed.remove, vec![opened.open[0].0]);
        assert_eq!(changed.open.len(), 1);
        assert_eq!(changed.open[0].1.as_deref(), Some("DP-3"));

        let replaced = surfaces.reconcile(Some(Id::unique()), None);
        assert_eq!(replaced.remove.len(), 2);
        assert_eq!(replaced.open.len(), 2);
        for (old, _) in opened.open {
            assert!(!surfaces.windows().any(|id| id == old));
        }

        let closed = surfaces.reconcile(None, None);
        assert_eq!(closed.remove.len(), 2);
        assert!(surfaces.windows().next().is_none());
    }
}
