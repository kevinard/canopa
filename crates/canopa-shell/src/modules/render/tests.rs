use super::*;
use crate::config::{ContentRotation, Edge, FormatConfig, FormatTemplate, Style};
use iced::advanced::{layout, renderer::Headless, widget::Tree};

#[tokio::test]
async fn horizontal_media_and_idle_templates_obey_explicit_rotation() {
    let renderer = iced::Renderer::new(iced::Font::DEFAULT, iced::Pixels(16.0), Some("tiny-skia"))
        .await
        .unwrap();
    let format = FormatConfig {
        horizontal: Some(
            FormatTemplate::parse(&["A long literal module label"], &[], "test").unwrap(),
        ),
        ..Default::default()
    };
    for media in [false, true] {
        let mut widths = vec![];
        for rotation in [ContentRotation::None, ContentRotation::Clockwise] {
            let module: Box<dyn ModuleRender> = if media {
                Box::new(crate::modules::media::MediaModule {
                    id: crate::modules::ModuleId::new("media"),
                    config: crate::config::MediaConfig {
                        format: format.clone(),
                        content_rotation: rotation,
                        ..Default::default()
                    },
                })
            } else {
                Box::new(crate::modules::idle_inhibitor::IdleInhibitorModule {
                    id: crate::modules::ModuleId::new("idle"),
                    config: crate::config::IdleInhibitorConfig {
                        format: format.clone(),
                        content_rotation: rotation,
                        ..Default::default()
                    },
                })
            };
            let context = BarRenderContext {
                window: iced::window::Id::unique(),
                edge: &Edge::Top,
            };
            let mut element =
                module.render_bar(&ModuleState::Unavailable, &context, &Style::default());
            let mut tree = Tree::new(element.as_widget());
            let node = element.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(iced::Size::ZERO, iced::Size::new(1000.0, 1000.0)),
            );
            widths.push(node.size().width);
        }
        assert!(
            widths[1] < widths[0] / 2.0,
            "explicit rotation ignored (media={media}): {widths:?}"
        );
    }
}

fn fixture(
    name: &str,
    format: FormatConfig,
    rotation: ContentRotation,
) -> (Box<dyn crate::modules::Module>, ModuleState) {
    use crate::{config::*, modules::*};
    let params = match name {
        "clock" => ModuleParams::Clock(ClockConfig {
            format,
            content_rotation: rotation,
            ..Default::default()
        }),
        "battery" => ModuleParams::Battery(BatteryConfig {
            format,
            content_rotation: rotation,
            ..Default::default()
        }),
        "network" => ModuleParams::Network(NetworkConfig {
            format,
            content_rotation: rotation,
            ..Default::default()
        }),
        "volume" => ModuleParams::Volume(VolumeConfig {
            format,
            content_rotation: rotation,
            ..Default::default()
        }),
        "media" => ModuleParams::Media(MediaConfig {
            format,
            content_rotation: rotation,
            ..Default::default()
        }),
        "idle-inhibitor" => ModuleParams::IdleInhibitor(IdleInhibitorConfig {
            format,
            content_rotation: rotation,
            ..Default::default()
        }),
        "notifications" => ModuleParams::Notifications(NotificationsConfig {
            format,
            ..Default::default()
        }),
        "focused-window" => ModuleParams::FocusedWindow(FocusedWindowConfig {
            format,
            content_rotation: rotation,
            ..Default::default()
        }),
        "button" => ModuleParams::Button(ButtonConfig {
            label: "Launch".into(),
            icon: ">".into(),
            format,
            content_rotation: rotation,
            tooltip: Default::default(),
        }),
        "script" => ModuleParams::Script(ScriptConfig {
            command: ConfiguredCommand::argv("true".into(), vec![], &CommandPolicy::default()),
            interval: 60,
            format,
            content_rotation: rotation,
            tooltip: Default::default(),
        }),
        "workspaces" => {
            return (
                Box::new(workspaces::WorkspaceItemModule::new(
                    niri_ipc::Workspace {
                        id: 1,
                        idx: 1,
                        name: None,
                        output: None,
                        is_urgent: false,
                        is_active: true,
                        is_focused: true,
                        active_window_id: None,
                    },
                    WorkspacesConfig {
                        format,
                        content_rotation: rotation,
                        ..Default::default()
                    },
                    "workspaces".into(),
                )),
                ModuleState::Empty,
            );
        }
        _ => unreachable!(),
    };
    let descriptor = crate::modules::MODULE_DESCRIPTORS
        .iter()
        .find(|d| d.config_name == name)
        .unwrap();
    let module = (descriptor.constructor)(&ModuleConfig {
        name: name.into(),
        module_type: name.into(),
        params,
        actions: Default::default(),
    })
    .unwrap();
    let state = match name {
        "clock" => ModuleState::Clock(clock::ClockState {
            time: "10:15".into(),
            date: "2026-10-01".into(),
        }),
        "button" => ModuleState::Button(button::ButtonState),
        "script" => ModuleState::Script(script::ScriptState {
            output: "script output".into(),
        }),
        "battery" => ModuleState::Battery(battery::BatteryState {
            percentage: 75.0,
            present: true,
            ..Default::default()
        }),
        "volume" => ModuleState::Volume(volume::VolumeState {
            volume: 50.0,
            muted: false,
        }),
        "network" => ModuleState::Network(network::NetworkState::default()),
        "focused-window" => ModuleState::FocusedWindow(focused_window::FocusedWindowState {
            window: Some(focused_window::FocusedWindowInfo {
                title: Some("Window".into()),
                app_id: Some("Application".into()),
                application_icon: None,
            }),
        }),
        "idle-inhibitor" => {
            ModuleState::IdleInhibitor(idle_inhibitor::IdleInhibitorState::Inactive)
        }
        "media" => ModuleState::Media(canopa_shell::media::Player {
            id: canopa_shell::media::PlayerId {
                bus: "player".into(),
                owner: ":1.2".into(),
                server: "server".into(),
            },
            name: "Player".into(),
            title: "Song".into(),
            artist: "Artist".into(),
            track: "/Track".into(),
            status: "Playing".into(),
            can_play: true,
            can_pause: true,
            can_next: true,
            can_previous: true,
        }),
        "notifications" => ModuleState::Unavailable,
        _ => unreachable!(),
    };
    (module, state)
}

fn size(
    mut element: iced::Element<'_, crate::app::Message>,
    renderer: &iced::Renderer,
) -> iced::Size {
    let mut tree = Tree::new(element.as_widget());
    element
        .as_widget_mut()
        .layout(
            &mut tree,
            renderer,
            &layout::Limits::new(iced::Size::ZERO, iced::Size::new(1000.0, 1000.0)),
        )
        .size()
}

#[tokio::test]
async fn focused_window_length_limit_reaches_bar_layout_on_every_edge() {
    let renderer = iced::Renderer::new(iced::Font::DEFAULT, iced::Pixels(16.0), Some("tiny-skia"))
        .await
        .unwrap();
    // Exercise configuration parsing, the module constructor and the actual Iced layout.
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        file.path(),
        r#"modules { module "focused-window" { max-length 9 } }"#,
    )
    .unwrap();
    let (config, _) =
        crate::config::load(&canopa_config::Request::legacy(file.path()), true).unwrap();
    let module_config = config
        .modules
        .iter()
        .find(|m| m.name == "focused-window")
        .unwrap();
    let constructor = crate::modules::build_registry()["focused-window"];
    let module = constructor(module_config).unwrap();
    let state = |title: String| {
        ModuleState::FocusedWindow(crate::modules::focused_window::FocusedWindowState {
            window: Some(crate::modules::focused_window::FocusedWindowInfo {
                title: Some(title),
                app_id: None,
                application_icon: None,
            }),
        })
    };
    let long_title = state(format!("ABCD{}WXYZ", "long title ".repeat(100)));
    let shortened_title = state("ABCD…WXYZ".into());
    let unbounded = constructor(&crate::config::ModuleConfig {
        params: crate::config::ModuleParams::FocusedWindow(crate::config::FocusedWindowConfig {
            max_length: 0,
            ..Default::default()
        }),
        ..module_config.clone()
    })
    .unwrap();

    for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
        let context = BarRenderContext {
            window: iced::window::Id::unique(),
            edge: &edge,
        };
        let style = Style::default();
        let bounded_size = size(module.render_bar(&long_title, &context, &style), &renderer);
        let expected_size = size(
            module.render_bar(&shortened_title, &context, &style),
            &renderer,
        );
        let unbounded_size = size(
            unbounded.render_bar(&long_title, &context, &style),
            &renderer,
        );
        assert_eq!(bounded_size, expected_size, "{edge:?}");
        if edge.is_horizontal() {
            assert!(bounded_size.width < unbounded_size.width / 2.0, "{edge:?}");
        } else {
            assert!(
                bounded_size.height < unbounded_size.height / 2.0,
                "{edge:?}"
            );
        }
    }
}

#[tokio::test]
async fn format_capable_modules_share_mode_inheritance_style_and_rotation_on_all_edges() {
    use crate::config::{FormatMode, ResolvedContentRotation};
    let renderer = iced::Renderer::new(iced::Font::DEFAULT, iced::Pixels(16.0), Some("tiny-skia"))
        .await
        .unwrap();
    let literal = |value| FormatTemplate::parse(&[value], &[], "test").unwrap();
    let formats = [
        FormatConfig::default(),
        FormatConfig {
            collapsed: Some(literal("Collapsed long literal content")),
            ..Default::default()
        },
        FormatConfig {
            expanded: Some(literal("Expanded long literal content")),
            ..Default::default()
        },
        FormatConfig {
            collapsed: Some(literal("Collapsed long literal content")),
            expanded: Some(literal("Expanded long literal content")),
            horizontal: Some(literal("Horizontal long literal content")),
        },
    ];
    for name in [
        "clock",
        "button",
        "script",
        "battery",
        "network",
        "volume",
        "media",
        "idle-inhibitor",
        "notifications",
        "focused-window",
        "workspaces",
    ] {
        for format in &formats {
            for edge in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
                let context = BarRenderContext {
                    window: iced::window::Id::unique(),
                    edge: &edge,
                };
                let layout = if edge.is_horizontal() {
                    crate::modules::view::IconLabelLayout::Horizontal
                } else {
                    crate::modules::view::IconLabelLayout::Collapsed
                };
                let mode: FormatMode = layout.into();
                let (baseline, state) = fixture(name, format.clone(), ContentRotation::None);
                let style = Style {
                    size: Some(17.0),
                    ..Default::default()
                };
                let normal = size(baseline.render_bar(&state, &context, &style), &renderer);
                for rotation in [
                    ContentRotation::None,
                    ContentRotation::Auto,
                    ContentRotation::Clockwise,
                    ContentRotation::CounterClockwise,
                ] {
                    let (module, state) = fixture(name, format.clone(), rotation);
                    if let Some(presentation) = module.presentation() {
                        let segments =
                            presentation.styled_segments_for_layout(&state, layout, &style);
                        assert_eq!(
                            segments.is_some(),
                            format.template_for(mode).is_some(),
                            "{name}/{mode:?}"
                        );
                        if let Some(segments) = segments {
                            let expected = if mode == FormatMode::Horizontal
                                && format.horizontal.is_some()
                            {
                                "Horizontal long literal content"
                            } else if mode == FormatMode::Horizontal && format.expanded.is_some() {
                                "Expanded long literal content"
                            } else {
                                "Collapsed long literal content"
                            };
                            assert_eq!(segments[0].0, expected, "{name}/{mode:?}");
                            assert_eq!(segments[0].1.size, Some(17.0), "style inheritance: {name}");
                        }
                    }
                    let actual = size(module.render_bar(&state, &context, &style), &renderer);
                    let rotated = name != "notifications"
                        && format.template_for(mode).is_some()
                        && rotation.resolve_for_edge(&edge) != ResolvedContentRotation::None;
                    if name == "workspaces" && edge.is_horizontal() {
                        // Workspace pills intentionally keep a fixed outer frame.
                        assert_eq!(actual.width, 32.0);
                    } else if rotated {
                        if edge.is_horizontal() {
                            assert!(
                                actual.width < normal.width / 2.0,
                                "{name}/{edge:?}/{rotation:?}: {normal:?} -> {actual:?}"
                            );
                        } else {
                            assert!(
                                actual.height > normal.height * 2.0,
                                "{name}/{edge:?}/{rotation:?}: {normal:?} -> {actual:?}"
                            );
                        }
                    } else {
                        assert_eq!(
                            actual, normal,
                            "fallback geometry: {name}/{edge:?}/{rotation:?}"
                        );
                    }
                    // Workspace items deliberately have no expanded Panel form.
                    if name != "workspaces" {
                        assert!(
                            size(module.render_panel_with_style(&state, &style), &renderer).height
                                > 0.0,
                            "panel: {name}"
                        );
                    }
                }
            }
        }
    }
}
