//! One manual preference shared by all module instances, with surface-owned
//! Wayland inhibitors. Only one request per surface may be in flight.
use super::{App, Message, messages::ModuleRuntimeEvent};
use crate::modules::data_source::ModuleDataSourceState;
use crate::modules::idle_inhibitor::IdleInhibitorState;
use iced::{Task, window::Id};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Default)]
struct SurfaceInhibitor {
    available: Option<bool>,
    applied: bool,
    pending: Option<bool>,
}

#[derive(Debug, Default)]
pub(super) struct IdleInhibitorRuntime {
    wanted: bool,
    surfaces: HashMap<Id, SurfaceInhibitor>,
    published: Option<IdleInhibitorState>,
}

impl IdleInhibitorRuntime {
    pub(super) fn toggle(&mut self) {
        self.wanted = !self.wanted;
        // A deliberate interaction also retries a previously unavailable adapter.
        for surface in self.surfaces.values_mut() {
            if surface.available == Some(false) {
                surface.available = None;
            }
        }
    }

    fn reconcile(&mut self, configured: bool, windows: HashSet<Id>) -> Vec<(Id, bool)> {
        if !configured {
            self.wanted = false;
        }
        self.surfaces.retain(|id, _| windows.contains(id));
        if configured {
            for id in windows {
                self.surfaces.entry(id).or_default();
            }
        }
        let mut requests = Vec::new();
        for (id, surface) in &mut self.surfaces {
            if surface.pending.is_none()
                && (surface.available.is_none()
                    || (surface.available == Some(true) && surface.applied != self.wanted))
            {
                surface.pending = Some(self.wanted);
                requests.push((*id, self.wanted));
            }
        }
        if !configured {
            self.surfaces
                .retain(|_, surface| surface.pending.is_some() || surface.applied);
        }
        requests
    }

    pub(super) fn completed(&mut self, window: Id, requested: bool, available: bool) {
        let Some(surface) = self.surfaces.get_mut(&window) else {
            return;
        };
        if surface.pending != Some(requested) {
            return;
        }
        surface.pending = None;
        surface.available = Some(available);
        surface.applied = requested && available;
    }

    fn state(&self) -> IdleInhibitorState {
        if self
            .surfaces
            .values()
            .any(|surface| surface.pending.is_some())
        {
            IdleInhibitorState::Pending
        } else if self
            .surfaces
            .values()
            .any(|surface| surface.available == Some(true))
        {
            if self.wanted {
                IdleInhibitorState::Active
            } else {
                IdleInhibitorState::Inactive
            }
        } else {
            IdleInhibitorState::Unavailable
        }
    }
}

impl App {
    pub(super) fn sync_idle_inhibitor(&mut self) -> Task<Message> {
        let configured = self
            .modules
            .has_data_source(crate::modules::data_source::ModuleDataSourceIdentity::IdleInhibitor);
        if !configured && self.data_sources.idle_inhibitor.surfaces.is_empty() {
            return Task::none();
        }
        // Initial size comes from the opened backend window. Never issue a
        // request for a planned surface before its Wayland unit exists.
        let windows = self
            .surfaces
            .keys()
            .copied()
            .filter(|id| {
                self.surfaces
                    .surface(*id)
                    .is_some_and(|s| s.screen_size.0 > 0 && s.screen_size.1 > 0)
            })
            .collect();
        let runtime = &mut self.data_sources.idle_inhibitor;
        let requests = runtime.reconcile(configured, windows);
        let state = runtime.state();
        let changed = configured && runtime.published != Some(state);
        runtime.published = configured.then_some(state);
        let mut tasks: Vec<_> = requests
            .into_iter()
            .map(|(window, enabled)| set_inhibition(window, enabled))
            .collect();
        if changed {
            tracing::debug!(target: "canopa_shell::idle_inhibitor", ?state, "idle inhibitor state changed");
            let transition = self
                .data_sources
                .apply_source_state(ModuleDataSourceState::IdleInhibitor(state));
            tasks.push(self.apply_module_data_source_transition(transition));
        }
        Task::batch(tasks)
    }
}

fn set_inhibition(window: Id, enabled: bool) -> Task<Message> {
    use iced_layershell::actions::{
        IdleInhibitionCallback, LayerShellCustomAction, LayerShellCustomActionWithId,
    };
    use std::sync::{Arc, Mutex};
    let (sender, receiver) = iced::futures::channel::oneshot::channel();
    let sender = Mutex::new(Some(sender));
    let callback = IdleInhibitionCallback(Arc::new(move |available| {
        if let Ok(mut sender) = sender.lock()
            && let Some(sender) = sender.take()
        {
            let _ = sender.send(available);
        }
    }));
    Task::batch([
        Task::done(Message::LayerShell(LayerShellCustomActionWithId(
            Some(window),
            LayerShellCustomAction::SetIdleInhibition { enabled, callback },
        ))),
        Task::perform(
            async move { receiver.await.unwrap_or(false) },
            move |available| {
                Message::Modules(ModuleRuntimeEvent::IdleInhibitorApplied {
                    window,
                    requested: enabled,
                    available,
                })
            },
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_off_and_serializes_rapid_toggles() {
        let mut runtime = IdleInhibitorRuntime::default();
        let window = Id::unique();
        let windows = HashSet::from([window]);
        assert_eq!(
            runtime.reconcile(true, windows.clone()),
            vec![(window, false)]
        );
        runtime.completed(window, false, true);
        assert_eq!(runtime.state(), IdleInhibitorState::Inactive);
        runtime.toggle();
        assert_eq!(
            runtime.reconcile(true, windows.clone()),
            vec![(window, true)]
        );
        runtime.toggle();
        assert!(runtime.reconcile(true, windows.clone()).is_empty());
        assert_eq!(runtime.state(), IdleInhibitorState::Pending);
        runtime.completed(window, true, true);
        assert_eq!(
            runtime.reconcile(true, windows.clone()),
            vec![(window, false)]
        );
        runtime.completed(window, false, true);
        assert!(runtime.reconcile(true, windows).is_empty());
        assert_eq!(runtime.state(), IdleInhibitorState::Inactive);
    }

    #[test]
    fn output_replacement_preserves_preference_and_ignores_retired_acknowledgement() {
        let mut runtime = IdleInhibitorRuntime::default();
        let old = Id::unique();
        let new = Id::unique();
        runtime.toggle();
        assert_eq!(
            runtime.reconcile(true, HashSet::from([old])),
            vec![(old, true)]
        );
        runtime.completed(old, true, true);
        assert_eq!(runtime.state(), IdleInhibitorState::Active);
        runtime.reconcile(true, HashSet::new());
        assert_eq!(runtime.state(), IdleInhibitorState::Unavailable);
        assert_eq!(
            runtime.reconcile(true, HashSet::from([new])),
            vec![(new, true)]
        );
        runtime.completed(old, true, false);
        runtime.completed(new, true, true);
        assert_eq!(runtime.state(), IdleInhibitorState::Active);
        assert!(!runtime.surfaces.contains_key(&old));
    }

    #[test]
    fn removing_last_module_releases_inhibition_and_resets_preference() {
        let mut runtime = IdleInhibitorRuntime::default();
        let window = Id::unique();
        let windows = HashSet::from([window]);
        runtime.toggle();
        runtime.reconcile(true, windows.clone());
        // Removal while enable is in flight still releases after it completes.
        assert!(runtime.reconcile(false, windows.clone()).is_empty());
        runtime.completed(window, true, true);
        assert_eq!(
            runtime.reconcile(false, windows.clone()),
            vec![(window, false)]
        );
        runtime.completed(window, false, true);
        assert!(runtime.reconcile(false, windows.clone()).is_empty());
        assert!(runtime.surfaces.is_empty());
        assert_eq!(runtime.reconcile(true, windows), vec![(window, false)]);
    }

    #[test]
    fn unsupported_protocol_stays_unavailable_without_polling_or_false_active_state() {
        let mut runtime = IdleInhibitorRuntime::default();
        let window = Id::unique();
        let windows = HashSet::from([window]);
        runtime.reconcile(true, windows.clone());
        runtime.completed(window, false, false);
        assert_eq!(runtime.state(), IdleInhibitorState::Unavailable);
        assert!(runtime.reconcile(true, windows.clone()).is_empty());
        runtime.toggle();
        assert_eq!(
            runtime.reconcile(true, windows.clone()),
            vec![(window, true)]
        );
        runtime.completed(window, true, false);
        assert_eq!(runtime.state(), IdleInhibitorState::Unavailable);
        assert!(runtime.reconcile(true, windows).is_empty());
    }

    #[test]
    fn configured_instances_share_acknowledged_state() {
        use crate::config::{Config, ModuleConfig, ModuleParams};
        use crate::modules::{ModuleId, ModuleState};
        let mut config = Config {
            modules: ["first", "second"]
                .map(|name| ModuleConfig {
                    name: name.into(),
                    module_type: "idle-inhibitor".into(),
                    params: ModuleParams::IdleInhibitor(Default::default()),
                    actions: Default::default(),
                })
                .into(),
            ..Config::default()
        };
        config.bars[0].modules.start = vec!["first".into(), "second".into()];
        config.bars[0].modules.center.clear();
        config.bars[0].modules.end.clear();
        let (mut app, _) = App::boot(config, vec!["DP-1".into()]);
        let window = *app.surfaces.keys().next().unwrap();
        app.surfaces.surface_mut(window).screen_size = (1000, 40);
        let _ = app.sync_idle_inhibitor();
        for (requested, expected) in [
            (false, IdleInhibitorState::Inactive),
            (true, IdleInhibitorState::Active),
        ] {
            if requested {
                let _ = app.update(Message::Modules(ModuleRuntimeEvent::ToggleIdleInhibitor));
            }
            let _ = app.update(Message::Modules(ModuleRuntimeEvent::IdleInhibitorApplied {
                window,
                requested,
                available: true,
            }));
            for name in ["first", "second"] {
                assert_eq!(
                    app.modules.state(&ModuleId::new(name)),
                    Some(&ModuleState::IdleInhibitor(expected))
                );
            }
        }
    }
}
