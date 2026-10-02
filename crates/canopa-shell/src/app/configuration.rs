//! Serialized candidate preparation and installation for both reload entry points.
use std::os::unix::fs::MetadataExt;
use std::time::Duration;

use canopa_config::{Request, SourceInfo};
use iced::{Subscription, Task};

use super::messages::IpcResponder;
use super::{App, Message};
use crate::config::Config;
use crate::ipc::protocol::IpcResponse;

#[derive(Debug, Clone)]
pub enum Event {
    Changed,
    Prepared(Result<Box<(Config, SourceInfo)>, String>),
}

#[cfg(test)]
mod tests;

#[derive(Default)]
pub(super) struct ConfigurationRuntime {
    request: Option<Request>,
    source: Option<SourceInfo>,
    loading: bool,
    pending: bool,
    responders: Vec<IpcResponder>,
    last_error: Option<String>,
    pub(super) generation: u64,
}

impl ConfigurationRuntime {
    pub(super) fn subscription(&self) -> Subscription<Message> {
        match &self.request {
            None | Some(Request::Defaults) => Subscription::none(),
            Some(request) => watch(request.clone()).map(Message::Configuration),
        }
    }
}

// Poll metadata, not source text. Resolving discovery every time also observes a
// newly created common file, directory replacement, and symlink target changes.
// Two equal observations debounce saves; all filesystem work stays off the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Observation {
    selected: Result<Request, canopa_config::Diagnostic>,
    metadata: Option<Result<FileStamp, std::io::ErrorKind>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileStamp {
    device: u64,
    inode: u64,
    size: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}

fn observe(request: &Request) -> Observation {
    let selected = request.resolve(canopa_config::Component::Shell);
    let metadata = match &selected {
        Ok(Request::File { path, .. }) => Some(
            std::fs::metadata(path)
                .map(|meta| FileStamp {
                    device: meta.dev(),
                    inode: meta.ino(),
                    size: meta.len(),
                    modified: (meta.mtime(), meta.mtime_nsec()),
                    changed: (meta.ctime(), meta.ctime_nsec()),
                })
                .map_err(|error| error.kind()),
        ),
        _ => None,
    };
    Observation { selected, metadata }
}

fn watch(request: Request) -> Subscription<Event> {
    // The serialized selection is stable subscription identity, including its format.
    Subscription::run_with(
        serde_json::to_string(&request).expect("configuration request"),
        |key| {
            let request: Request = serde_json::from_str(key).expect("configuration request");
            iced::futures::stream::unfold(
                (request, None, None),
                |(request, mut previous, mut emitted)| async move {
                    loop {
                        tokio::time::sleep(Duration::from_millis(250)).await;
                        let selection = request.clone();
                        let Ok(observation) =
                            tokio::task::spawn_blocking(move || observe(&selection)).await
                        else {
                            continue;
                        };
                        if previous.as_ref() == Some(&observation)
                            && emitted.as_ref() != Some(&observation)
                        {
                            emitted = Some(observation);
                            return Some((Event::Changed, (request, previous, emitted)));
                        }
                        previous = Some(observation);
                    }
                },
            )
        },
    )
}

async fn prepare(request: Request) -> Result<Box<(Config, SourceInfo)>, String> {
    tokio::task::spawn_blocking(move || {
        crate::config::load(&request, false)
            .map(Box::new)
            .map_err(|error| error.safe_diagnostic().to_string())
    })
    .await
    .map_err(|_| "configuration preparation failed".to_string())?
}

fn respond(responder: &IpcResponder, response: IpcResponse) {
    if let Ok(mut sender) = responder.lock()
        && let Some(sender) = sender.take()
    {
        let _ = sender.send(response);
    }
}

impl App {
    pub(crate) fn configure_reload(&mut self, request: Request, source: SourceInfo) {
        self.configuration.request = Some(request);
        self.configuration.source = Some(source);
    }

    pub(super) fn request_configuration_reload(
        &mut self,
        responder: Option<IpcResponder>,
    ) -> Task<Message> {
        let Some(request) = self.configuration.request.clone() else {
            if let Some(responder) = responder {
                respond(
                    &responder,
                    IpcResponse::Rejected {
                        reason: "configuration selection unavailable".into(),
                    },
                );
            }
            return Task::none();
        };
        if let Some(responder) = responder {
            if self.configuration.responders.len() >= 64 {
                respond(
                    &responder,
                    IpcResponse::Rejected {
                        reason: "too many pending reload requests".into(),
                    },
                );
                return Task::none();
            }
            self.configuration.responders.push(responder);
        }
        if self.configuration.loading {
            self.configuration.pending = true;
            return Task::none();
        }
        self.configuration.loading = true;
        Task::perform(prepare(request), |candidate| {
            Message::Configuration(Event::Prepared(candidate))
        })
    }

    pub(super) fn handle_configuration_event(&mut self, event: Event) -> Task<Message> {
        let candidate = match event {
            Event::Changed => return self.request_configuration_reload(None),
            Event::Prepared(candidate) => candidate,
        };
        self.configuration.loading = false;
        // A save/command arrived during preparation. Prepare the latest version
        // before acknowledging any callers instead of installing a stale candidate.
        if std::mem::take(&mut self.configuration.pending) {
            return self.request_configuration_reload(None);
        }
        let result = candidate.and_then(|candidate| {
            let (config, source) = *candidate;
            let task = self.install_configuration(config)?;
            self.configuration.source = Some(source);
            self.configuration.last_error = None;
            tracing::info!(target: "canopa_shell::configuration", generation = self.configuration.generation,
                source = ?self.configuration.source, "configuration reloaded");
            Ok(task)
        });
        let (response, task) = match result {
            Ok(task) => (IpcResponse::Accepted, task),
            Err(reason) => {
                tracing::warn!(target: "canopa_shell::configuration", error = %reason, "configuration reload rejected; keeping previous configuration");
                self.configuration.last_error = Some(reason.clone());
                (IpcResponse::Rejected { reason }, Task::none())
            }
        };
        for responder in self.configuration.responders.drain(..) {
            respond(&responder, response.clone());
        }
        task
    }

    fn install_configuration(&mut self, config: Config) -> Result<Task<Message>, String> {
        if config == self.config {
            return Ok(Task::none());
        }
        // Complete all fallible construction before touching live state.
        let graph = super::module_graph::build_module_graph(&config)
            .map_err(|_| "configuration module construction failed".to_string())?;
        let mut modules =
            super::module_runtime::ModuleRuntime::from_modules_with_actions(graph.modules);
        modules.set_dynamic_module_family_inputs(
            super::dynamic_modules::family_inputs_from_config(&config),
        );
        modules.inherit_configuration(
            &self.modules,
            &self.config,
            &config,
            self.configuration.generation + 1,
        );

        self.close_external_panel_session_for_replacement();
        let effects = self.foreground.configuration_changed();
        let mut tasks = self.apply_foreground_effects(effects);
        tasks.extend(self.restore_inactive_drawer_bars());

        let desired =
            super::bar_surface_plan::plan_bar_surfaces(&config.bars, &self.eligible_outputs);
        let old_plan =
            super::bar_surface_plan::plan_bar_surfaces(&self.config.bars, &self.eligible_outputs);
        let recreate: Vec<_> = self
            .surfaces
            .keys()
            .copied()
            .filter(|window| {
                let identity = self
                    .surfaces
                    .bar_surface_identity_for_window(*window)
                    .unwrap();
                let before = old_plan
                    .iter()
                    .enumerate()
                    .find(|(_, entry)| entry.identity == identity);
                let after = desired
                    .iter()
                    .enumerate()
                    .find(|(_, entry)| entry.identity == identity);
                before != after
            })
            .collect();
        for window in recreate {
            let effects = self.surfaces.retire_bar_surface(window);
            tasks.extend(self.apply_bar_surface_lifecycle_effects(effects));
        }
        for surface in self.surfaces.values_mut() {
            surface.focused_module = None;
            surface.hovered_module = None;
            surface.tooltip_pending = None;
            surface.module_activated = false;
        }

        self.config = config;
        crate::theme::install(self.config.appearance);
        self.configuration.generation += 1;
        self.data_sources.reconfigure(&mut modules);
        self.modules = modules;
        let workspaces = self.niri.state().workspaces.clone();
        self.reconcile_workspace_dynamic_modules(&workspaces);
        if self.niri.state().connected {
            self.push_focused_window_state();
        } else {
            for kind in [
                crate::modules::ModuleKind::Workspaces,
                crate::modules::ModuleKind::FocusedWindow,
            ] {
                self.modules.handle(
                    super::module_runtime::ModuleRuntimeIntent::ApplyStateToKind {
                        kind,
                        state: crate::modules::ModuleState::Unavailable,
                    },
                );
            }
        }
        self.reconcile_tray_dynamic_modules(&self.tray.snapshot());
        let effects = self.surfaces.reconcile_bar_surfaces(desired);
        tasks.extend(self.apply_bar_surface_lifecycle_effects(effects));
        tasks.extend(self.reconcile_module_visibility());
        Ok(Task::batch(tasks))
    }
}
