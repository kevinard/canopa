use super::*;
use crate::app::messages::{IpcEvent, ModuleRuntimeEvent, ipc_responder};
use crate::config::{BarConfig, Edge, OutputTarget};
use crate::modules::script::{ScriptEvent, ScriptState};
use crate::modules::{ModuleEvent, ModuleId, ModuleState};

fn source() -> SourceInfo {
    SourceInfo {
        path: None,
        format: None,
        fingerprint: "original".into(),
        defaults: true,
    }
}

fn app(config: Config) -> App {
    let (mut app, _) = App::boot(config, vec!["DP-1".into(), "DP-2".into()]);
    app.configure_reload(Request::Defaults, source());
    app
}

fn install(app: &mut App, config: Config) {
    let _ = app.update(Message::Configuration(Event::Prepared(Ok(Box::new((
        config,
        source(),
    ))))));
    assert!(app.configuration.last_error.is_none());
}

fn reload_request(app: &mut App) -> tokio::sync::oneshot::Receiver<IpcResponse> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let _ = app.update(Message::Ipc(IpcEvent::Reload {
        responder: ipc_responder(tx),
    }));
    rx
}

fn window(app: &App, bar: &str, output: &str) -> iced::window::Id {
    *app.surfaces
        .surfaces()
        .find(|(_, s)| s.bar_name == bar && s.output_name == output)
        .unwrap()
        .0
}

fn open_panel(app: &mut App) {
    let origin_window = *app.surfaces.keys().next().unwrap();
    let _ = app.update(Message::Panel(crate::app::messages::PanelEvent::Open {
        origin_window,
        origin: crate::surface::panel::PanelOrigin::Bar,
        content: crate::surface::panel::PanelContent::BarPanel,
        anchor: None,
    }));
    assert!(app.foreground.has_active_panel());
}

#[test]
fn content_reload_retains_windows_and_dismisses_foreground_but_noop_does_not() {
    let mut app = app(Config::default());
    let original = window(&app, "main", "DP-1");
    open_panel(&mut app);
    let config = app.config.clone();
    install(&mut app, config);
    assert!(app.foreground.has_active_panel());
    assert_eq!(app.configuration.generation, 0);
    let mut config = app.config.clone();
    config.bars[0].modules.start = vec!["clock".into()];
    install(&mut app, config);
    assert!(!app.foreground.has_active_panel());
    assert_eq!(window(&app, "main", "DP-1"), original);
    assert!(app.modules.contains_instance(&ModuleId::new("clock")));
    assert_eq!(app.configuration.generation, 1);
}

#[test]
fn changed_geometry_recreates_surfaces_and_new_targets_use_all_observed_outputs() {
    let mut config = Config::default();
    config.bars[0].output_target = OutputTarget::Include(vec!["DP-1".into()]);
    let mut app = app(config);
    let original = window(&app, "main", "DP-1");
    let mut config = app.config.clone();
    config.bars[0].edge = Edge::Top;
    config.bars[0].collapsed_width = 36;
    config.bars.push(BarConfig {
        name: "second".into(),
        output_target: OutputTarget::Include(vec!["DP-2".into()]),
        ..BarConfig::default()
    });
    install(&mut app, config);
    assert_ne!(window(&app, "main", "DP-1"), original);
    assert!(!app.surfaces.contains_key(&original));
    assert_eq!(app.surfaces.len(), 2);
    window(&app, "second", "DP-2");
    // Queued input from the retired window must not recreate it.
    let _ = app.update(Message::Surface(
        crate::app::messages::SurfaceEvent::ModuleHoverEnter(ModuleId::new("clock"), original),
    ));
    assert!(!app.surfaces.contains_key(&original));
}

#[tokio::test]
async fn invalid_candidate_keeps_foreground_configuration_source_and_generation() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.kdl");
    std::fs::write(&path, "schema-version 1\nshell { private-secret 12; }\n").unwrap();
    let mut app = app(Config::default());
    app.configuration.request = Some(Request::common(&path));
    open_panel(&mut app);
    let before = app.config.clone();
    let mut response = reload_request(&mut app);
    assert!(response.try_recv().is_err());
    let prepared = prepare(Request::common(&path)).await;
    let _ = app.update(Message::Configuration(Event::Prepared(prepared)));
    let IpcResponse::Rejected { reason } = response.await.unwrap() else {
        panic!("reload must fail")
    };
    assert!(!reason.contains("private-secret"));
    assert_eq!(app.config, before);
    assert_eq!(app.configuration.source, Some(source()));
    assert_eq!(app.configuration.generation, 0);
    assert!(app.foreground.has_active_panel());
    std::fs::write(
        &path,
        "schema-version 1\nshell { bar \"main\" { edge \"bottom\"; }; }\n",
    )
    .unwrap();
    let response = reload_request(&mut app);
    let _ = app.update(Message::Configuration(Event::Prepared(
        prepare(Request::common(path)).await,
    )));
    assert_eq!(response.await.unwrap(), IpcResponse::Accepted);
    assert_eq!(app.config.bars[0].edge, Edge::Bottom);
    assert!(app.configuration.last_error.is_none());
}

#[tokio::test]
async fn concurrent_requests_wait_for_latest_candidate_and_reply_to_every_caller() {
    let mut app = app(Config::default());
    let mut first = reload_request(&mut app);
    let mut second = reload_request(&mut app);
    let mut obsolete = app.config.clone();
    obsolete.bars[0].edge = Edge::Top;
    let _ = app.update(Message::Configuration(Event::Prepared(Ok(Box::new((
        obsolete,
        source(),
    ))))));
    assert!(first.try_recv().is_err());
    assert!(second.try_recv().is_err());
    assert_eq!(app.config.bars[0].edge, Edge::Left);
    let mut newest = app.config.clone();
    newest.bars[0].edge = Edge::Bottom;
    install(&mut app, newest);
    assert_eq!(first.await.unwrap(), IpcResponse::Accepted);
    assert_eq!(second.await.unwrap(), IpcResponse::Accepted);
    assert_eq!(app.config.bars[0].edge, Edge::Bottom);
    assert_eq!(app.configuration.generation, 1);
}

fn script_config() -> Config {
    let mut config = Config::default();
    config.bars[0].modules.start = vec!["script".into()];
    config.modules.push(crate::config::ModuleConfig {
        name: "script".into(),
        module_type: "script".into(),
        actions: Default::default(),
        params: crate::config::ModuleParams::Script(crate::config::ScriptConfig {
            command: crate::config::ConfiguredCommand::argv_for_test("true", vec![]),
            interval: 10,
            format: Default::default(),
            content_rotation: Default::default(),
            tooltip: Default::default(),
        }),
    });
    config
}

fn script_event(generation: u64, text: &str) -> Message {
    let id = ModuleId::new("script");
    Message::ConfiguredModule {
        id: id.clone(),
        generation,
        message: Box::new(Message::Modules(ModuleRuntimeEvent::Module(
            id,
            ModuleEvent::Script(ScriptEvent::Output(text.into())),
        ))),
    }
}

#[test]
fn unchanged_modules_keep_state_and_subscriptions_changed_modules_reject_old_emissions() {
    let mut app = app(script_config());
    let _ = app.update(script_event(0, "first"));
    let mut config = app.config.clone();
    config.bars[0].tooltip_delay_ms += 1;
    install(&mut app, config);
    let _ = app.update(script_event(0, "still running"));
    assert_eq!(
        app.modules.state(&ModuleId::new("script")),
        Some(&ModuleState::Script(ScriptState {
            output: "still running".into()
        }))
    );
    let mut config = app.config.clone();
    let crate::config::ModuleParams::Script(script) = &mut config.modules[0].params else {
        unreachable!()
    };
    script.command = crate::config::ConfiguredCommand::argv_for_test("false", vec![]);
    install(&mut app, config);
    let _ = app.update(script_event(0, "stale"));
    assert!(app.modules.state(&ModuleId::new("script")).is_none());
    let _ = app.update(script_event(2, "new"));
    assert_eq!(
        app.modules.state(&ModuleId::new("script")),
        Some(&ModuleState::Script(ScriptState {
            output: "new".into()
        }))
    );
}

#[test]
fn shared_readings_repopulate_changed_modules_without_restarting_the_source() {
    let mut config = Config::default();
    config.bars[0].modules.start = vec!["volume".into()];
    let mut app = app(config);
    let _ = app.update(Message::Modules(ModuleRuntimeEvent::Volume(
        crate::modules::volume::VolumeEvent::StateChanged {
            volume: 42.0,
            muted: false,
        },
    )));
    let expected = app.modules.state(&ModuleId::new("volume")).cloned();
    let mut config = app.config.clone();
    config.modules.push(crate::config::ModuleConfig {
        name: "volume".into(),
        module_type: "volume".into(),
        params: crate::config::ModuleParams::Volume(Default::default()),
        actions: Default::default(),
    });
    install(&mut app, config);
    assert_eq!(
        app.modules.state(&ModuleId::new("volume")),
        expected.as_ref()
    );
    let mut config = app.config.clone();
    config.bars[0].modules.start.clear();
    install(&mut app, config);
    let _ = app.update(Message::Modules(ModuleRuntimeEvent::Volume(
        crate::modules::volume::VolumeEvent::Unavailable,
    )));
    assert!(
        app.data_sources.active_identities_for_test().is_empty(),
        "late source events cannot restart a removed source"
    );
}

#[test]
fn reload_keeps_disconnected_compositor_modules_unavailable() {
    let mut config = Config::default();
    config.bars[0].modules.start = vec!["focused-window".into(), "workspaces".into()];
    let mut app = app(config);
    let _ = app.update(Message::Niri(crate::app::messages::AppNiriEvent::Ipc(
        crate::niri::NiriEvent::Unavailable,
    )));
    let mut config = app.config.clone();
    config.bars[0].tooltip_delay_ms += 1;
    install(&mut app, config);
    for name in ["focused-window", "workspaces"] {
        assert_eq!(
            app.modules.state(&ModuleId::new(name)),
            Some(&ModuleState::Unavailable)
        );
    }
}

#[tokio::test]
async fn watcher_observes_in_place_saves_atomic_replacement_and_symlink_targets() {
    use iced::futures::{StreamExt, stream};
    let directory = tempfile::tempdir().unwrap();
    let target = directory.path().join("target.kdl");
    let path = directory.path().join("config.kdl");
    std::fs::write(&target, "initial").unwrap();
    std::os::unix::fs::symlink(&target, &path).unwrap();
    let recipe = iced::advanced::subscription::into_recipes(watch(Request::common(&path)))
        .pop()
        .unwrap();
    let mut events = recipe.stream(Box::pin(stream::pending()));
    async fn changed(events: &mut (impl iced::futures::Stream<Item = Event> + Unpin)) {
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(3), events.next())
                .await
                .unwrap(),
            Some(Event::Changed)
        ));
    }
    changed(&mut events).await;
    std::fs::write(&target, "changed target").unwrap();
    changed(&mut events).await;
    let replacement = directory.path().join("replacement");
    std::fs::write(&replacement, "replacement").unwrap();
    std::fs::rename(&replacement, &path).unwrap();
    changed(&mut events).await;
    std::fs::remove_file(&path).unwrap();
    changed(&mut events).await;
    std::fs::write(&path, "recreated").unwrap();
    changed(&mut events).await;
    assert!(
        tokio::time::timeout(Duration::from_millis(600), events.next())
            .await
            .is_err(),
        "unchanged files emit nothing"
    );
}

#[test]
fn reload_preserves_acknowledged_idle_inhibition_and_release_on_removal() {
    use crate::modules::idle_inhibitor::IdleInhibitorState;
    let mut config = Config::default();
    config.bars[0].modules.start = vec!["idle-inhibitor".into()];
    let mut app = app(config);
    let windows: Vec<_> = app.surfaces.keys().copied().collect();
    for window in &windows {
        app.surfaces.surface_mut(*window).screen_size = (48, 800);
    }
    let _ = app.sync_idle_inhibitor();
    for window in &windows {
        let _ = app.update(Message::Modules(ModuleRuntimeEvent::IdleInhibitorApplied {
            window: *window,
            requested: false,
            available: true,
        }));
    }
    let _ = app.update(Message::Modules(ModuleRuntimeEvent::ToggleIdleInhibitor));
    for window in &windows {
        let _ = app.update(Message::Modules(ModuleRuntimeEvent::IdleInhibitorApplied {
            window: *window,
            requested: true,
            available: true,
        }));
    }
    let mut config = app.config.clone();
    config.bars[0].tooltip_delay_ms += 1;
    install(&mut app, config);
    assert_eq!(
        app.modules.state(&ModuleId::new("idle-inhibitor")),
        Some(&ModuleState::IdleInhibitor(IdleInhibitorState::Active))
    );
    let mut config = app.config.clone();
    config.bars[0].modules.start.clear();
    install(&mut app, config.clone());
    for window in &windows {
        let _ = app.update(Message::Modules(ModuleRuntimeEvent::IdleInhibitorApplied {
            window: *window,
            requested: false,
            available: true,
        }));
    }
    config.bars[0].modules.start.push("idle-inhibitor".into());
    install(&mut app, config);
    for window in &windows {
        let _ = app.update(Message::Modules(ModuleRuntimeEvent::IdleInhibitorApplied {
            window: *window,
            requested: false,
            available: true,
        }));
    }
    assert_eq!(
        app.modules.state(&ModuleId::new("idle-inhibitor")),
        Some(&ModuleState::IdleInhibitor(IdleInhibitorState::Inactive))
    );
}
