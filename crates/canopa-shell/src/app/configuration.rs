//! Serialized candidate preparation and installation for both reload entry points.
use std::os::unix::fs::MetadataExt;
use std::time::Duration;

use canopa_config::{Request, SourceInfo};
use iced::{Subscription, Task};

use super::messages::IpcResponder;
use super::{App, Message};
use crate::config::Config;
use crate::config::notice::{ConfigNotice, Fallback};
use crate::ipc::protocol::IpcResponse;
use crate::notice::NoticeSlot;

#[derive(Debug, Clone)]
pub enum Event {
    Changed,
    Prepared(Result<Box<(Config, SourceInfo)>, Box<Rejection>>),
}

/// A rejected candidate: the safe reason for logs and IPC callers, and the
/// detailed notice for the person editing the file.
#[derive(Debug, Clone)]
pub struct Rejection {
    reason: String,
    notice: ConfigNotice,
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
    /// The latest rejection, shown in the bar and through Notify until a
    /// candidate installs.
    pub(super) notice: Option<ConfigNotice>,
    /// Set while the Shell runs its embedded defaults because the selected
    /// configuration was invalid at startup.
    fallback: Option<Fallback>,
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
    theme: Option<std::path::PathBuf>,
    theme_metadata: Option<Result<FileStamp, std::io::ErrorKind>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileStamp {
    device: u64,
    inode: u64,
    size: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}

#[cfg(test)]
fn observe(request: &Request) -> Observation {
    observe_after(request, None)
}

fn stamp(path: &std::path::Path) -> Result<FileStamp, std::io::ErrorKind> {
    std::fs::metadata(path)
        .map(|meta| FileStamp {
            device: meta.dev(),
            inode: meta.ino(),
            size: meta.len(),
            modified: (meta.mtime(), meta.mtime_nsec()),
            changed: (meta.ctime(), meta.ctime_nsec()),
        })
        .map_err(|error| error.kind())
}

fn observe_after(request: &Request, previous: Option<&Observation>) -> Observation {
    let selected = request.resolve(canopa_config::Component::Shell);
    let metadata = match &selected {
        Ok(Request::File { path, .. }) => Some(stamp(path)),
        _ => None,
    };
    let theme = if let Some(previous) =
        previous.filter(|p| p.selected == selected && p.metadata == metadata)
    {
        previous.theme.clone()
    } else if let Ok(Request::File {
        path,
        format: canopa_config::Format::CanopaV1,
    }) = &selected
    {
        canopa_config::read(path)
            .ok()
            .and_then(|text| canopa_config::theme_path(&text, Some(path)).ok().flatten())
    } else {
        None
    };
    let theme_metadata = theme.as_ref().map(|path| stamp(path));
    Observation {
        selected,
        metadata,
        theme,
        theme_metadata,
    }
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
                        let prior = previous.clone();
                        let Ok(observation) = tokio::task::spawn_blocking(move || {
                            observe_after(&selection, prior.as_ref())
                        })
                        .await
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

async fn prepare(request: Request) -> Result<Box<(Config, SourceInfo)>, Box<Rejection>> {
    let selection = request.clone();
    tokio::task::spawn_blocking(move || {
        crate::config::load(&selection, true)
            .map(Box::new)
            .map_err(|error| {
                Box::new(Rejection {
                    reason: error.safe_diagnostic().to_string(),
                    notice: ConfigNotice::from_error(&error, &selection),
                })
            })
    })
    .await
    .map_err(|_| {
        Box::new(Rejection {
            reason: "configuration preparation failed".into(),
            notice: ConfigNotice::construction(&request),
        })
    })?
}

fn respond(responder: &IpcResponder, response: IpcResponse) {
    if let Ok(mut sender) = responder.lock()
        && let Some(sender) = sender.take()
    {
        let _ = sender.send(response);
    }
}

impl App {
    pub(crate) fn configure_reload(
        &mut self,
        request: Request,
        source: SourceInfo,
        startup_error: Option<ConfigNotice>,
    ) {
        self.configuration.request = Some(request);
        self.configuration.source = Some(source);
        if let Some(notice) = startup_error {
            self.configuration.fallback = Some(Fallback::EmbeddedDefaults);
            self.show_configuration_notice(notice);
        }
    }

    fn show_configuration_notice(&mut self, notice: ConfigNotice) {
        let fallback = self
            .configuration
            .fallback
            .unwrap_or(Fallback::PreviousConfiguration);
        self.notices
            .show(NoticeSlot::Configuration, notice.notice(fallback));
        self.configuration.notice = Some(notice);
    }

    /// A button on one of the Shell's notices, or on the bar's error badge.
    pub(super) fn handle_notice_action(
        &mut self,
        action: crate::notice::NoticeAction,
    ) -> Task<Message> {
        Task::batch(
            self.apply_app_effects(vec![super::app_effects::AppEffect::LaunchApplication(
                action.argv(),
            )]),
        )
    }

    /// The bar's configuration-error badge: its click and right-click actions.
    pub(super) fn configuration_notice_actions(&self) -> Option<Vec<crate::notice::NoticeAction>> {
        self.configuration
            .notice
            .as_ref()
            .map(ConfigNotice::actions)
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
            let task = self.install_configuration(config).map_err(|reason| {
                let request = self.configuration.request.clone().unwrap_or_default();
                Box::new(Rejection {
                    reason,
                    notice: ConfigNotice::construction(&request),
                })
            })?;
            self.configuration.source = Some(source);
            self.configuration.fallback = None;
            if self.configuration.notice.take().is_some() {
                self.notices.clear(NoticeSlot::Configuration);
            }
            tracing::info!(target: "canopa_shell::configuration", generation = self.configuration.generation,
                source = ?self.configuration.source, "configuration reloaded");
            Ok(task)
        });
        let (response, task) = match result {
            Ok(task) => (IpcResponse::Accepted, task),
            Err(rejection) => {
                let Rejection { reason, notice } = *rejection;
                tracing::warn!(target: "canopa_shell::configuration", error = %reason, "configuration reload rejected; keeping previous configuration");
                self.show_configuration_notice(notice);
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
        crate::theme::install(self.config.appearance.clone());
        self.configuration.generation += 1;
        self.data_sources.reconfigure(&mut modules);
        self.modules = modules;
        let workspaces = self.niri.state().workspaces.clone();
        self.reconcile_workspace_dynamic_modules(&workspaces);
        if self.niri.state().connected {
            self.push_focused_window_state();
            if self.niri.state().output_sizes.is_empty() {
                tasks.push(self.refresh_niri_output_sizes());
            }
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
        self.taskbar.invalidate();
        self.refresh_taskbar();
        let effects = self.surfaces.reconcile_bar_surfaces(desired);
        tasks.extend(self.apply_bar_surface_lifecycle_effects(effects));
        tasks.extend(
            self.surfaces
                .keys()
                .map(|window| self.drawer_bar_input_region(*window)),
        );
        tasks.extend(self.reconcile_module_visibility());
        Ok(Task::batch(tasks))
    }
}
