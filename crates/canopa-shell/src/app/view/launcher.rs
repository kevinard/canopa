use std::rc::Rc;

use iced::widget::{Space, column, container, keyed_column, responsive, row, stack};
use iced::{Element, Length, Padding, Size};

use super::super::launcher::{QuickScope, Target};
use super::super::messages::PanelEvent;
use super::super::panel_menu_session::{PanelMenuEntry, PanelMenuEntryTarget};
use super::super::shell_accessibility::{launcher_announcements, menu_entry_key};
use super::{
    Message, PanelRenderModel, panel_surface, render_menu_scroll, selected_menu_key,
    styled_panel_frame,
};
use crate::{launcher::Action, theme};
use canopa_ui::accessibility::{Node, semantic};

/// The results list, which the search field controls.
const RESULTS: &str = "launcher-results";

/// Browsing and search headings share one height so typing never moves the list.
const LAUNCHER_HEADING_HEIGHT: f32 = 24.0;
const CLOSE_ICON: &[u8] = include_bytes!("../../../assets/close.svg");
/// Result rows the launcher shows before it scrolls.
const LAUNCHER_ROWS: usize = 8;
const SEARCH_ICON: &[u8] = include_bytes!("../../../assets/search.svg");
const APPS_ICON: &[u8] = include_bytes!("../../../assets/app-grid.svg");
const FILES_ICON: &[u8] = include_bytes!("../../../assets/folder.svg");
const WINDOWS_ICON: &[u8] = include_bytes!("../../../assets/window.svg");
const SETTINGS_ICON: &[u8] = include_bytes!("../../../assets/controls.svg");
const CALC_ICON: &[u8] = include_bytes!("../../../assets/calculator.svg");
const DOC_ICON: &[u8] = include_bytes!("../../../assets/document.svg");
const ALIAS_ICON: &[u8] = include_bytes!("../../../assets/search-alias.svg");
const PIN_ICON: &[u8] = include_bytes!("../../../assets/pin.svg");
const SHORTCUT_ICON: &[u8] = include_bytes!("../../../assets/shortcut.svg");
const PLAY_ICON: &[u8] = include_bytes!("../../../assets/play.svg");

pub(super) fn view<'a>(
    model: PanelRenderModel<'a>,
    text_color: Option<iced::Color>,
) -> Element<'a, Message> {
    let model = Rc::new(model);
    responsive(move |bounds| contents(&model, text_color, bounds)).into()
}

fn contents<'a>(
    model: &Rc<PanelRenderModel<'a>>,
    text_color: Option<iced::Color>,
    bounds: Size,
) -> Element<'a, Message> {
    let session = model.launcher_session.expect("launcher render state");
    let child = model.can_go_back;
    let browsing = !child && model.search_query.trim().is_empty();
    let count = model.menu_entries.len();
    let ui = model.ui;
    let width = bounds.width
        - ui.padding.horizontal()
        - ui.margin.horizontal()
        - if ui.custom_border {
            2.0 * ui.border_width.px()
        } else {
            0.0
        };
    let horizontal = Padding {
        left: ui.padding.left.px(),
        right: ui.padding.right.px(),
        ..Default::default()
    };
    let target = selected_target(model);
    let wide = width >= 450.0;
    let close_shortcut = !child && model.search_query.is_empty();
    let mut close_node = Node::button(fl!("shell-launcher-close"))
        .key("launcher-close")
        .on_click(Some(Message::Panel(PanelEvent::Close)));
    if close_shortcut {
        close_node = close_node.shortcut("Escape");
    }
    let close = semantic(
        close_node,
        control_hint(
            theme::icon_button(CLOSE_ICON, Some(Message::Panel(PanelEvent::Close)), false),
            if close_shortcut {
                fl!("shell-launcher-close-hint", key = fl!("shell-key-escape"))
            } else {
                fl!("shell-launcher-close")
            },
        ),
    );
    let chrome = 208.0 + ui.margin.vertical() + ui.padding.vertical();
    let metrics = model.menu_metrics;
    let calculator = !child
        && count == 1
        && target.is_some_and(|target| is_calculator(&model.menu_entries[0], target));
    let empty_recovery = count == 0
        // The status is only the session's own "No matches" (or nothing).
        && (model.launcher_status.is_empty()
            || model.launcher_status == fl!("shell-launcher-no-matches"))
        && !visible_query(model).trim().is_empty();
    let preferred_body = if count == 0 {
        if empty_recovery && width < 445.0 {
            118.0
        } else {
            76.0
        }
    } else if count == 1 && !child && wide {
        if calculator {
            112.0
        } else {
            metrics.extent - metrics.gap
        }
    } else {
        // Results keep the browsing height, with room for one section heading,
        // so typing never resizes the panel or moves its footer.
        let visible_rows = if browsing {
            count.min(LAUNCHER_ROWS)
        } else {
            LAUNCHER_ROWS
        };
        metrics.extent * visible_rows as f32 - metrics.gap
            + if visible_rows == LAUNCHER_ROWS {
                crate::app::launcher::SECTION_HEADING_HEIGHT
            } else {
                0.0
            }
    };
    let compact = model.adaptive && bounds.height >= chrome + preferred_body;
    let mut controls = column![
        row![
            search(model, session, !empty_recovery, width >= 330.0),
            container(close).padding(Padding::default().top(8)),
        ]
        .spacing(10)
    ]
    .spacing(8);
    let has_scopes = !child && !calculator && !model.launcher_scopes.is_empty();
    if has_scopes {
        controls = controls.push(scope_buttons(model, session, width));
        controls = controls.push(footer_rule());
    }
    if child {
        let title = model
            .menu_prompt
            .clone()
            .unwrap_or_else(|| model.title.clone());
        controls = controls.push(
            canopa_ui::direction::row([
                back_button(model.search_query.is_empty()),
                semantic(
                    Node::heading(title.clone(), 2).key("launcher-heading"),
                    theme::strong_text(title)
                        .size(theme::body_size() + 2.0)
                        .wrapping(iced::widget::text::Wrapping::None),
                ),
            ])
            .align_y(iced::Alignment::Center)
            .spacing(12),
        );
    } else if calculator {
        controls = controls.push(
            row![
                theme::icon(CALC_ICON, 18.0, theme::palette().accent),
                semantic(
                    Node::heading(fl!("shell-launcher-calculator"), 2).key("launcher-heading"),
                    theme::strong_text(fl!("shell-launcher-calculator")).size(caption_size()),
                ),
            ]
            .align_y(iced::Alignment::Center)
            .spacing(6),
        );
    } else if browsing && count > 0 {
        let header = row![
            semantic(
                Node::heading(browse_heading(model), 2).key("launcher-heading"),
                theme::strong_text(browse_heading(model)).size(theme::body_size() + 1.0),
            ),
            Space::new().width(Length::Fill),
        ]
        .height(LAUNCHER_HEADING_HEIGHT)
        .align_y(iced::Alignment::Center)
        .spacing(4);
        controls = controls.push(header);
    } else if count > 0 && !visible_query(model).trim().is_empty() {
        // Searching keeps a heading row, so the panel does not jump as you type.
        controls = controls.push(
            row![
                semantic(
                    Node::heading(results_heading(model), 2).key("launcher-heading"),
                    theme::strong_text(results_heading(model)).size(theme::body_size() + 1.0),
                ),
                Space::new().width(Length::Fill),
            ]
            .height(LAUNCHER_HEADING_HEIGHT)
            .align_y(iced::Alignment::Center)
            .spacing(4),
        );
    }
    let frame_height = if compact {
        Length::Shrink
    } else {
        Length::Fill
    };
    let body = if count == 0 {
        empty(
            model,
            session,
            width,
            empty_recovery,
            if compact {
                Length::Fixed(preferred_body)
            } else {
                frame_height
            },
        )
    } else if compact {
        if calculator
            && wide
            && let Some(target) = target
        {
            calculator_result(model, target, preferred_body)
        } else {
            // A steady height: the footer never moves, and a row cut at the
            // bottom fades out to show that the list scrolls.
            padded_results(model, session, Length::Fixed(preferred_body))
        }
    } else {
        let model = Rc::clone(model);
        responsive(move |space| {
            let available = (space.height - 8.0).max(0.0);
            let height = if available.is_finite() {
                fitted_height(&model, available).unwrap_or(available.min(preferred_body))
            } else {
                available.min(preferred_body)
            };
            padded_results(&model, session, Length::Fixed(height))
        })
        .into()
    };
    let top = if ui.custom_padding {
        ui.padding.top.px()
    } else {
        18.0
    };
    let bottom = if ui.custom_padding {
        ui.padding.bottom.px()
    } else {
        16.0
    };
    let mut results = column![].spacing(12);
    if !has_scopes && count > 0 {
        results = results.push(footer_rule());
    }
    results = results.push(body);
    if let Some(target) = target {
        // "Top matches" notes are announced; the list itself shows the ranking.
        let ranking_note = [
            fl!("shell-launcher-top-matches"),
            fl!("shell-files-top-matches"),
        ]
        .contains(&model.launcher_status);
        if !model.launcher_status.is_empty() && !ranking_note {
            results = results.push(muted_label(model.launcher_status.clone()));
        }
        // The bar holds Actions and the primary command only; other commands
        // stay in Actions (Tab) and on their Ctrl+digit keys, so one action
        // leads and the panel keeps its height.
        let mut footer = row![].spacing(7).align_y(iced::Alignment::Center);
        // Beside the selected result's Actions the arrows keep a compact hint.
        let has_actions = target.result.actions.len() >= 2;
        // Only where the footer has room for every control as well.
        let commands_width = 180.0 + 110.0;
        if width >= 380.0 && has_actions && count > 1 {
            if width >= commands_width + 70.0 {
                footer = footer.push(keycap("↑")).push(keycap("↓"));
            }
        } else if width >= 380.0 {
            if count > 1 {
                footer = footer
                    .push(keycap("↑"))
                    .push(keycap("↓"))
                    .push(muted_label(fl!("shell-launcher-select")));
            } else if !child && !calculator {
                footer = footer.push(muted_label(result_count(model)));
            }
        }
        // The selected result's other commands, one key away.
        if let Some(actions) = row_actions(target) {
            footer = footer.push(actions);
        }
        footer = footer.push(Space::new().width(Length::Fill));
        footer = footer.push(command_controls(model, target, width));
        let footer = container(footer).padding([5, 8]);
        results = results.push(if calculator {
            footer
        } else {
            footer.style(|theme| {
                let palette = canopa_ui::colors(theme);
                container::Style::default()
                    .background(palette.text.scale_alpha(0.033))
                    .border(iced::Border::default().rounded(10))
            })
        });
    }
    let matches =
        (count > 0 && !visible_query(model).trim().is_empty()).then(|| result_count(model));
    let (polite, assertive) = launcher_announcements(
        &model.launcher_status,
        model.launcher_error.as_deref(),
        matches.as_deref(),
    );
    // Always present, so each change of their text is announced.
    let controls = stack![
        container(controls).padding(horizontal.top(top)),
        semantic(Node::status(polite).key("launcher-status"), Space::new()),
        semantic(Node::alert(assertive).key("launcher-alert"), Space::new()),
    ];
    let content = column![
        controls,
        container(results).padding(horizontal.bottom(bottom)),
    ]
    .spacing(12)
    .height(frame_height);
    panel_surface(
        styled_panel_frame(content, text_color, model.drawer, ui, frame_height),
        compact,
        false,
    )
}

/// The name of the first section, which heads a sectioned list.
fn first_section(model: &PanelRenderModel<'_>) -> Option<String> {
    model
        .menu_entries
        .first()
        .and_then(|entry| match &entry.target {
            PanelMenuEntryTarget::Launcher(target) => target.section.map(|section| section.label()),
            _ => None,
        })
}

fn browse_heading(model: &PanelRenderModel<'_>) -> String {
    if let Some(section) = first_section(model) {
        return section;
    }
    let apps_only = model.menu_entries.iter().all(|entry| {
        matches!(&entry.target, PanelMenuEntryTarget::Launcher(target)
            if target.result.actions.first().is_some_and(|named| matches!(named.action, Action::Application { .. })))
    });
    if apps_only {
        fl!("shell-launcher-installed-apps")
    } else {
        fl!("shell-launcher-browse")
    }
}

fn is_calculator(entry: &PanelMenuEntry, target: &Target) -> bool {
    entry.icon.as_deref() == Some("accessories-calculator")
        && target
            .result
            .actions
            .first()
            .is_some_and(|named| matches!(named.action, Action::Copy { .. }))
}

fn calculator_result<'a>(
    model: &PanelRenderModel<'a>,
    target: &Target,
    height: f32,
) -> Element<'a, Message> {
    let entry = &model.menu_entries[0];
    let activate = Message::Panel(PanelEvent::ActivateLauncher(target.clone(), 0));
    let node = Node::option(entry.label.clone(), true)
        .key(menu_entry_key(entry, 0))
        .description(target.result.description.clone())
        .position(1, 1)
        .on_click(Some(activate));
    let expression = model.search_query.trim().trim_start_matches('=').trim();
    let labels = column![
        theme::text(expression.to_owned())
            .font(iced::Font::MONOSPACE)
            .size(theme::body_size() + 2.0)
            .line_height(1.1)
            .wrapping(iced::widget::text::Wrapping::None),
        footer_rule(),
        theme::strong_text(entry.label.clone())
            .size(theme::body_size() + 20.0)
            .line_height(1.05)
            .wrapping(iced::widget::text::Wrapping::None),
    ]
    .spacing(8)
    .align_x(iced::Alignment::Center)
    .width(Length::Fill);
    let result = theme::action(
        container(labels)
            .padding([8, 28])
            .center(Length::Fill)
            .clip(true),
        Some(Message::Panel(PanelEvent::ActivateLauncher(
            target.clone(),
            0,
        ))),
        canopa_ui::Action::Quiet,
        false,
    )
    .width(Length::Fill)
    .height(height)
    .style(|theme, status| {
        let palette = canopa_ui::colors(theme);
        let mut style = canopa_ui::action_style(theme, status, canopa_ui::Action::Quiet, false);
        style.background = Some(palette.accent.scale_alpha(0.11).into());
        style.border.radius = 11.0.into();
        style
    });
    semantic(results_node(model), semantic(node, result))
}

/// The results list: what the search found, or the browse-all list.
fn results_node(model: &PanelRenderModel<'_>) -> Node<Message> {
    let label = if model.can_go_back {
        model
            .menu_prompt
            .clone()
            .unwrap_or_else(|| model.title.clone())
    } else if model.search_query.trim().is_empty() {
        browse_heading(model)
    } else {
        fl!("shell-launcher-results")
    };
    Node::list(label).key(RESULTS)
}

fn results_heading(model: &PanelRenderModel<'_>) -> String {
    first_section(model).unwrap_or_else(|| fl!("shell-launcher-results"))
}

/// A section heading inside the results list; the column adds `gap` after it.
pub(super) fn section_heading<'a>(label: String, gap: f32) -> Element<'a, Message> {
    semantic(
        Node::heading(label.clone(), 3),
        container(
            theme::strong_text(label)
                .size(theme::body_size() + 1.0)
                .wrapping(iced::widget::text::Wrapping::None),
        )
        .height(crate::app::launcher::SECTION_HEADING_HEIGHT - gap)
        .width(Length::Fill)
        .align_y(iced::Alignment::End)
        .padding(Padding::default().bottom(2.0)),
    )
}

fn command_controls(
    model: &PanelRenderModel<'_>,
    target: &Target,
    width: f32,
) -> Element<'static, Message> {
    let mut toolbar = row![].spacing(6).align_y(iced::Alignment::Center);
    if let Some(primary) = target.result.actions.first() {
        let label = if model.can_go_back && target.result.id.starts_with("action:") {
            target.result.label.clone()
        } else if primary.label == fl!("shell-launcher-open")
            && model.menu_entries.len() == 1
            && width >= 450.0
        {
            fl!(
                "shell-launcher-open-named",
                name = target.result.label.clone()
            )
        } else {
            primary.label.clone()
        };
        let node = Node::button(label.clone())
            .key("launcher-primary")
            .shortcut("Enter")
            .on_click(Some(Message::Panel(PanelEvent::ActivateLauncher(
                target.clone(),
                0,
            ))));
        let label = fit_line(
            &label,
            if width >= 465.0 { 170.0 } else { 92.0 },
            caption_size(),
            theme::font(),
        );
        toolbar = toolbar.push(semantic(
            node,
            theme::action(
                row![
                    theme::strong_text(label)
                        .size(caption_size())
                        .wrapping(iced::widget::text::Wrapping::None),
                    primary_keycap(),
                ]
                .spacing(7)
                .align_y(iced::Alignment::Center),
                Some(Message::Panel(PanelEvent::ActivateLauncher(
                    target.clone(),
                    0,
                ))),
                canopa_ui::Action::Primary,
                false,
            )
            .padding([6, 10]),
        ));
    }
    toolbar.into()
}

/// The selected result's Actions button, on its own row: a result with more
/// than one action lists them (Tab) without leaving the row it belongs to.
fn row_actions(target: &Target) -> Option<Element<'static, Message>> {
    if target.result.actions.len() < 2 {
        return None;
    }
    let event = Message::Panel(PanelEvent::LauncherActions(Some(target.clone())));
    let name = fl!(
        "shell-launcher-actions-for",
        name = target.result.label.clone()
    );
    let button = theme::action(
        command_label(&fl!("shell-launcher-actions"), fl!("shell-key-tab")),
        Some(event.clone()),
        canopa_ui::Action::Normal,
        false,
    )
    .padding([4, 8]);
    Some(semantic(
        Node::button(name.clone())
            .key("launcher-actions")
            .shortcut("Tab")
            .on_click(Some(event)),
        control_hint(button, name),
    ))
}

fn control_hint<'a>(
    content: impl Into<Element<'a, Message>>,
    hint: impl iced::widget::text::IntoFragment<'a>,
) -> Element<'a, Message> {
    iced::widget::tooltip(
        content,
        theme::text(hint).size(caption_size()),
        iced::widget::tooltip::Position::Bottom,
    )
    .style(theme::card)
    .padding(6)
    .gap(4)
    .into()
}

fn back_button(escape: bool) -> Element<'static, Message> {
    let node = Node::button(fl!("shell-back"))
        .key("launcher-back")
        .on_click(Some(Message::Panel(PanelEvent::Back)));
    if escape {
        return semantic(
            node.shortcut("Escape"),
            command_button(
                &canopa_ui::direction::back_label(),
                fl!("shell-key-escape"),
                PanelEvent::Back,
            ),
        );
    }
    semantic(
        node,
        control_hint(
            theme::action(
                theme::strong_text(canopa_ui::direction::back_label()).size(caption_size()),
                Some(Message::Panel(PanelEvent::Back)),
                canopa_ui::Action::Normal,
                false,
            )
            .padding([6, 10]),
            fl!("shell-back"),
        ),
    )
}

fn padded_results<'a>(
    model: &PanelRenderModel<'a>,
    session: u64,
    height: Length,
) -> Element<'a, Message> {
    let gutter = scrollbar_gutter(model, height);
    let inset = if gutter > 0.0 { 7.0 } else { 0.0 };
    let list = keyed_column([(
        (session, model.launcher_generation),
        render_menu_scroll(model, height),
    )])
    .height(height);
    // More rows below: the list fades out above the footer instead of cutting a
    // row in half.
    let metrics = model.menu_metrics;
    let content_height = model.menu_entries.len() as f32 * metrics.extent - metrics.gap
        + crate::app::launcher::section_starts(&model.menu_entries).len() as f32
            * crate::app::launcher::SECTION_HEADING_HEIGHT;
    let viewport = match height {
        Length::Fixed(height) => height,
        _ => model.menu_viewport_height,
    };
    let more_below = viewport > 0.0 && model.menu_scroll_y + viewport < content_height - 1.0;
    let list: Element<'a, Message> = if more_below {
        // The panel's glass colour, nearly opaque where the list meets the
        // footer, so a cut row is hidden rather than ghosted.
        let fade = theme::glass_fill(theme::Glass::Panel);
        let solid = iced::Color { a: 0.97, ..fade };
        iced::widget::stack![
            list,
            container(
                container(Space::new().width(Length::Fill).height(14)).style(move |_| {
                    container::Style::default().background(iced::Gradient::Linear(
                        iced::gradient::Linear::new(iced::Radians(std::f32::consts::PI))
                            .add_stop(0.0, fade.scale_alpha(0.0))
                            .add_stop(0.6, fade)
                            .add_stop(1.0, solid),
                    ))
                })
            )
            .height(height)
            .align_y(iced::Alignment::End),
        ]
        .into()
    } else {
        list.into()
    };
    semantic(
        results_node(model),
        container(list)
            .padding(Padding::default().right(inset))
            .width(Length::Fill),
    )
}

fn visible_query<'a>(model: &'a PanelRenderModel<'_>) -> &'a str {
    let prefix = model
        .launcher_input_scope
        .map_or("", |scope| scope.prefix());
    model
        .search_query
        .strip_prefix(prefix)
        .unwrap_or(&model.search_query)
}

fn clear_query(session: u64) -> PanelEvent {
    PanelEvent::LauncherSetQuery {
        session,
        query: String::new(),
    }
}

fn search<'a>(
    model: &PanelRenderModel<'a>,
    session: u64,
    show_clear: bool,
    show_clear_shortcut: bool,
) -> Element<'a, Message> {
    let scope = model.launcher_input_scope;
    let placeholder: String = if let Some(scope) = scope {
        scope.search_placeholder()
    } else if let Some(placeholder) = &model.launcher_filter_placeholder {
        placeholder.clone()
    } else {
        model
            .menu_prompt
            .clone()
            .unwrap_or_else(|| fl!("shell-search"))
    };
    let placeholder = placeholder.as_str();
    let prefix = scope.map_or("", |scope| scope.prefix());
    let query = visible_query(model);
    use canopa_theme::{ColorRole, ColorValue};
    let mut input_ui = theme::ui(canopa_theme::UiKind::Input);
    if input_ui.background == ColorValue::Role(ColorRole::Raised) {
        // On glass the field is a soft well of the text colour; opaque
        // surfaces keep the darker background fill.
        input_ui.background = if theme::glass_enabled() {
            ColorValue::Rgba(theme::appearance().palette().text, 16)
        } else {
            ColorValue::Role(ColorRole::Background)
        };
    }
    if !input_ui.custom_border && input_ui.states[4].border_color.is_none() {
        input_ui.states[4].border_color = Some(ColorValue::Rgba(
            theme::appearance().role(ColorRole::Focus),
            255,
        ));
    }
    let mut padding = canopa_ui::content_padding(input_ui);
    if !input_ui.custom_padding {
        padding.top = 14.0;
        padding.bottom = 14.0;
    }
    let clearable = show_clear && !query.is_empty();
    // The search counts every match beside the query it answers, not beside
    // the first section's heading.
    let count = (clearable && !model.can_go_back && !model.menu_entries.is_empty())
        .then(|| result_count(model));
    if let Some(count) = &count {
        padding.right += theme::text_width(count, caption_size()) + 8.0;
    }
    if clearable {
        padding.right += theme::text_width(&fl!("shell-launcher-clear"), caption_size()) + 27.0;
        if show_clear_shortcut {
            padding.right += theme::text_width(&fl!("shell-key-escape"), caption_size()) + 16.0;
        }
    }
    let icon_inset = padding.left;
    padding.left += 28.0;
    // The field keeps keyboard focus; the highlighted result is its active
    // descendant, so moving the selection announces the row.
    let node = Node::search_input(placeholder, query)
        .key("launcher-search")
        .placeholder(placeholder)
        .controls(RESULTS)
        .active_descendant(selected_menu_key(model))
        .on_text(move |query| {
            Message::Panel(PanelEvent::LauncherSearchChanged {
                session,
                query: format!("{prefix}{query}"),
            })
        });
    let input = semantic(
        node,
        theme::input_with_style(placeholder, query, input_ui)
            .id(model.search_input_id.clone())
            .on_input(move |query| {
                Message::Panel(PanelEvent::LauncherSearchChanged {
                    session,
                    query: format!("{prefix}{query}"),
                })
            })
            .size(theme::body_size() + 2.0)
            .padding(padding)
            .width(Length::Fill),
    );
    let glyph = container(theme::icon(SEARCH_ICON, 20.0, theme::palette().accent))
        .height(Length::Fill)
        .align_y(iced::Alignment::Center)
        .padding(Padding::default().left(icon_inset));
    if !clearable {
        return stack![input, glyph].into();
    }

    let mut clear_label = row![
        theme::strong_text(fl!("shell-launcher-clear"))
            .size(caption_size())
            .style(canopa_ui::link)
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center);
    if show_clear_shortcut {
        clear_label = clear_label.push(keycap(fl!("shell-key-escape")));
    }
    let clear_node = Node::button(fl!("shell-launcher-clear-search"))
        .key("launcher-clear")
        .on_click(Some(Message::Panel(clear_query(session))));
    let clear_node = if show_clear_shortcut {
        clear_node.shortcut("Escape")
    } else {
        clear_node
    };
    let clear = control_hint(
        theme::action(
            clear_label,
            Some(Message::Panel(clear_query(session))),
            canopa_ui::Action::Quiet,
            false,
        )
        .padding([4, 8])
        .style(|theme, status| {
            let mut style = canopa_ui::action_style(theme, status, canopa_ui::Action::Quiet, false);
            if matches!(
                status,
                iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
            ) {
                style.background = Some(theme::accent_surface().into());
            }
            style.border.radius = 8.0.into();
            style
        }),
        fl!(
            "shell-launcher-clear-search-hint",
            key = fl!("shell-key-escape")
        ),
    );
    let mut trailing = row![].spacing(4).align_y(iced::Alignment::Center);
    if let Some(count) = count {
        trailing = trailing.push(muted_label(count).size(caption_size()));
    }
    trailing = trailing.push(semantic(clear_node, clear));
    stack![
        input,
        glyph,
        container(trailing)
            .height(Length::Fill)
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Right)
            .align_y(iced::Alignment::Center)
            .padding(Padding::default().right(6.0)),
    ]
    .into()
}

fn scope_buttons(
    model: &PanelRenderModel<'_>,
    session: u64,
    width: f32,
) -> Element<'static, Message> {
    let scopes = std::iter::once(None).chain(model.launcher_scopes.iter().copied().map(Some));
    let equal_width = width >= 465.0;
    let choices =
        row(scopes.map(|scope| scope_button(model, session, scope, equal_width))).spacing(4);
    let node = Node::tab_list(fl!("shell-launcher-search-in")).key("launcher-scopes");
    semantic(node, scope_choices(choices, equal_width))
}

fn scope_choices(
    choices: iced::widget::Row<'static, Message>,
    equal_width: bool,
) -> Element<'static, Message> {
    if equal_width {
        container(choices)
            .padding(4)
            .style(|theme| {
                let palette = canopa_ui::colors(theme);
                container::Style::default()
                    .background(palette.text.scale_alpha(0.045))
                    .border(iced::Border::default().rounded(11))
            })
            .into()
    } else {
        choices.wrap().into()
    }
}

fn scope_button(
    model: &PanelRenderModel<'_>,
    session: u64,
    scope: Option<QuickScope>,
    equal_width: bool,
) -> Element<'static, Message> {
    let selected = model.launcher_scope == scope;
    let label = scope.map_or_else(|| fl!("shell-launcher-scope-all"), |scope| scope.label());
    let mut content = row![].spacing(6).align_y(iced::Alignment::Center);
    if equal_width {
        let glyph = match scope {
            None => SEARCH_ICON,
            Some(QuickScope::Apps) => APPS_ICON,
            Some(QuickScope::Files) => FILES_ICON,
            Some(QuickScope::Windows) => WINDOWS_ICON,
            Some(QuickScope::Settings) => SETTINGS_ICON,
        };
        let palette = theme::palette();
        let color = if selected {
            palette.accent
        } else {
            palette.text.scale_alpha(0.73)
        };
        content = content.push(theme::icon(glyph, 16.0, color));
    }
    content = content.push(if selected {
        theme::strong_text(label.clone())
            .size(caption_size())
            .style(canopa_ui::link)
    } else {
        theme::text(label.clone()).size(caption_size())
    });
    let width = if equal_width {
        Length::Fill
    } else {
        Length::Shrink
    };
    let key = match scope {
        None => "all",
        Some(QuickScope::Apps) => "apps",
        Some(QuickScope::Files) => "files",
        Some(QuickScope::Windows) => "windows",
        Some(QuickScope::Settings) => "settings",
    };
    let node = Node::tab(label, selected)
        .key(format!("launcher-scope-{key}"))
        .on_click(Some(Message::Panel(PanelEvent::LauncherSelectScope {
            session,
            scope,
        })));
    let button = iced::widget::button(container(content).center_x(width))
        .on_press(Message::Panel(PanelEvent::LauncherSelectScope {
            session,
            scope,
        }))
        .padding([6, 6])
        .width(width)
        .style(move |theme, status| {
            let palette = canopa_ui::colors(theme);
            let mut style = canopa_ui::action_style(theme, status, canopa_ui::Action::Quiet, false);
            style.border.radius = 8.0.into();
            if selected {
                // The same accent surface marks every selected or active control.
                style.background = Some(theme::accent_surface().into());
            } else if matches!(
                status,
                iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
            ) {
                style.background = Some(palette.text.scale_alpha(0.09).into());
            }
            style
        });
    semantic(node, button)
}

fn empty<'a>(
    model: &PanelRenderModel<'a>,
    session: u64,
    width: f32,
    recovery: bool,
    height: Length,
) -> Element<'a, Message> {
    let title = if model.launcher_status.is_empty() {
        fl!("shell-launcher-no-matches")
    } else {
        model.launcher_status.clone()
    };
    let mut labels = column![theme::strong_text(title).size(theme::body_size() + 2.0)].spacing(5);
    if recovery {
        let advice = if model.launcher_scope.is_some() {
            fl!("shell-launcher-advice-scope")
        } else {
            fl!("shell-launcher-advice-spelling")
        };
        labels = labels.push(muted_label(advice).size(theme::body_size()));
    }
    let message = row![
        badge(
            theme::icon(SEARCH_ICON, 23.0, theme::palette().accent),
            42.0,
            theme::accent_surface()
        ),
        labels,
    ]
    .spacing(12)
    .align_y(iced::Alignment::Center);
    let contents: Element<'_, Message> = if recovery {
        let clear = semantic(
            Node::button(fl!("shell-launcher-clear-search"))
                .key("launcher-clear-empty")
                .shortcut("Escape")
                .on_click(Some(Message::Panel(clear_query(session)))),
            command_button(
                &fl!("shell-launcher-clear-search"),
                fl!("shell-key-escape"),
                clear_query(session),
            ),
        );
        if width >= 445.0 {
            row![message, Space::new().width(Length::Fill), clear]
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .into()
        } else {
            column![message, clear].spacing(8).into()
        }
    } else {
        message.into()
    };
    container(contents)
        .padding([4, 9])
        .width(Length::Fill)
        .center_y(height)
        .into()
}

fn selected_target<'a>(model: &'a PanelRenderModel<'_>) -> Option<&'a Target> {
    let entry = model.menu_entries.get(model.focused_index?)?;
    let PanelMenuEntryTarget::Launcher(target) = &entry.target else {
        return None;
    };
    Some(target)
}

fn result_count(model: &PanelRenderModel<'_>) -> String {
    let count = model.menu_entries.len();
    let approximate = model.menu_entries.iter().filter(|entry| {
        matches!(&entry.target, PanelMenuEntryTarget::Launcher(target) if is_approximate(target))
    }).count();
    if count > 1 && approximate > 0 && approximate < count {
        fl!(
            "shell-launcher-match-count-approximate",
            count = count,
            approximate = approximate
        )
    } else {
        fl!("shell-launcher-match-count", count = count)
    }
}

fn footer_rule<'a>() -> iced::widget::Rule<'a, iced::Theme> {
    iced::widget::rule::horizontal(1).style(|theme| iced::widget::rule::Style {
        color: canopa_ui::colors(theme).border.scale_alpha(0.40),
        radius: 0.into(),
        fill_mode: iced::widget::rule::FillMode::Full,
        snap: true,
    })
}

fn caption_size() -> f32 {
    theme::label_size() + 2.0
}

fn muted_label<'a>(label: impl iced::widget::text::IntoFragment<'a>) -> iced::widget::Text<'a> {
    theme::text(label)
        .size(caption_size())
        .style(|theme| iced::widget::text::Style {
            color: Some(canopa_ui::colors(theme).text.scale_alpha(0.82)),
        })
}

fn command_button(
    label: &str,
    shortcut: impl iced::widget::text::IntoFragment<'static>,
    event: PanelEvent,
) -> iced::widget::Button<'static, Message> {
    // Secondary commands are quiet buttons beside the one filled action.
    let style = canopa_ui::Action::Quiet;
    theme::action(
        command_label(label, shortcut),
        Some(Message::Panel(event)),
        style,
        false,
    )
    .padding([6, 10])
    .clip(true)
}

fn command_label<'a>(
    label: &str,
    shortcut: impl iced::widget::text::IntoFragment<'a>,
) -> iced::widget::Row<'a, Message> {
    row![
        theme::strong_text(label.to_owned())
            .size(caption_size())
            .wrapping(iced::widget::text::Wrapping::None),
        keycap(shortcut),
    ]
    .spacing(7)
    .align_y(iced::Alignment::Center)
}

fn keycap<'a>(label: impl iced::widget::text::IntoFragment<'a>) -> Element<'a, Message> {
    container(theme::text(label).size(theme::label_size() + 1.0))
        .padding([2, 5])
        .style(|theme| {
            let palette = canopa_ui::colors(theme);
            container::Style::default()
                .background(palette.text.scale_alpha(0.08))
                .color(palette.text)
                .border(iced::Border::default().rounded(5))
        })
        .into()
}

fn primary_keycap() -> Element<'static, Message> {
    container(
        theme::text(fl!("shell-key-enter"))
            .size(theme::label_size() + 1.0)
            .style(|theme| iced::widget::text::Style {
                color: Some(canopa_ui::colors(theme).on_accent),
            }),
    )
    .padding([2, 5])
    .style(|theme| {
        let palette = canopa_ui::colors(theme);
        container::Style::default()
            .background(palette.on_accent.scale_alpha(0.10))
            .color(palette.on_accent)
            .border(iced::Border::default().rounded(5))
    })
    .into()
}

fn badge<'a>(
    icon: Element<'a, Message>,
    size: f32,
    background: iced::Color,
) -> Element<'a, Message> {
    container(icon)
        .center(size)
        .style(move |_| {
            container::Style::default()
                .background(background)
                .border(iced::Border::default().rounded(9))
        })
        .into()
}

/// The tallest list of whole rows, with the section headings among them, that fits
/// in `available`.
fn fitted_height(model: &PanelRenderModel<'_>, available: f32) -> Option<f32> {
    let metrics = model.menu_metrics;
    let sections = crate::app::launcher::section_starts(&model.menu_entries);
    let height = |rows: usize| {
        rows as f32 * metrics.extent - metrics.gap
            + sections.iter().filter(|start| **start < rows).count() as f32
                * crate::app::launcher::SECTION_HEADING_HEIGHT
    };
    let mut rows = 0;
    while rows < model.menu_entries.len().max(1) + 64 && height(rows + 1) <= available {
        rows += 1;
    }
    (rows >= 1).then(|| height(rows).min(available))
}

pub(super) fn scrollbar_gutter(model: &PanelRenderModel<'_>, height: Length) -> f32 {
    if let Length::Fixed(height) = height {
        let metrics = model.menu_metrics;
        let content_height = model.menu_entries.len() as f32 * metrics.extent - metrics.gap
            + crate::app::launcher::section_starts(&model.menu_entries).len() as f32
                * crate::app::launcher::SECTION_HEADING_HEIGHT;
        if content_height <= height {
            return 0.0;
        }
    }
    model.ui.padding.right.px().min(12.0)
}

fn is_approximate(target: &Target) -> bool {
    target.approximate
}

fn default_selection(style: canopa_theme::ResolvedStyle) -> bool {
    !style.custom_border
        && style.background == canopa_theme::ColorValue::Rgba(canopa_theme::Rgb(0, 0, 0), 0)
}

fn selection_indicator(selected: bool) -> Element<'static, Message> {
    container(Space::new().width(3).height(27))
        .style(move |theme| {
            let color = if selected {
                canopa_ui::colors(theme).accent
            } else {
                iced::Color::TRANSPARENT
            };
            container::Style::default()
                .background(color)
                .border(iced::Border::default().rounded(2))
        })
        .into()
}

pub(super) fn result_row<'a>(
    model: &PanelRenderModel<'a>,
    index: usize,
    entry: &PanelMenuEntry,
    target: &Target,
    selected: bool,
) -> Element<'a, Message> {
    let mut node = Node::option(entry.label.clone(), selected)
        .key(menu_entry_key(entry, index))
        .position(index + 1, model.menu_entries.len())
        .on_click(Some(Message::Panel(PanelEvent::ActivateLauncher(
            target.clone(),
            0,
        ))));
    if let Some(description) = result_description(model, target) {
        node = node.description(description);
    }
    let metrics = model.menu_metrics;
    let style = metrics.row;
    let padding = if style.custom_padding {
        canopa_ui::content_padding(style)
    } else {
        Padding {
            top: 3.0,
            bottom: 3.0,
            left: 5.0,
            right: 10.0,
        }
    };
    let default_selection = default_selection(style);
    let keyboard_focus = selected && model.keyboard_focus;
    let row_height = metrics.extent - metrics.gap - style.margin.vertical();
    let mut cells = Vec::new();
    if default_selection {
        cells.push(selection_indicator(selected));
    }
    cells.push(result_content(model, entry, target));

    let contents = canopa_ui::direction::row(cells)
        .spacing(7)
        .align_y(iced::Alignment::Center);
    let frame = move |hovered| {
        let mut frame = theme::highlighted_frame(style, selected, hovered, false, keyboard_focus);
        if default_selection {
            frame.border.width = 0.0;
        }
        frame
    };
    let label = iced::widget::button(
        container(contents)
            .height(Length::Fill)
            .align_y(iced::Alignment::Center),
    )
    .on_press(Message::Panel(PanelEvent::ActivateLauncher(
        target.clone(),
        0,
    )))
    .padding(padding)
    .height(row_height)
    .width(Length::Fill)
    .clip(true)
    .style(move |_, status| {
        let hovered = status == iced::widget::button::Status::Hovered;
        let frame = frame(hovered);
        iced::widget::button::Style {
            background: frame.background,
            text_color: frame.text_color.unwrap_or(theme::palette().text),
            border: frame.border,
            ..Default::default()
        }
    });

    semantic(
        node,
        crate::surface::pointer_focus::pointer_focus(
            container(label).padding(canopa_ui::padding(style.margin)),
            Message::Panel(PanelEvent::FocusLauncher(target.clone())),
        ),
    )
}

/// The secondary line a result shows, which also describes it to assistive technologies.
fn result_description(model: &PanelRenderModel<'_>, target: &Target) -> Option<String> {
    let result = &target.result;
    let is_action = result.id.starts_with("action:") && model.can_go_back;
    let browsing = !model.can_go_back && model.search_query.trim().is_empty();
    let description = target
        .display_description
        .as_deref()
        .unwrap_or(&result.description);
    let description = if !model.can_go_back && model.menu_entries.len() == 1 {
        target.display_detail.as_deref().unwrap_or(description)
    } else {
        description
    };
    if is_action {
        action_description(target)
    } else if (!browsing || description != fl!("shell-launcher-application"))
        && !description.is_empty()
    {
        Some(description.to_owned())
    } else {
        None
    }
}

fn result_content<'a>(
    model: &PanelRenderModel<'a>,
    entry: &PanelMenuEntry,
    target: &Target,
) -> Element<'a, Message> {
    let is_character = target.provider == crate::launcher::characters::PROVIDER;
    let browsing = !model.can_go_back && model.search_query.trim().is_empty();
    let icon_size = 30.0;
    // Icon then text in reading order (mirrored for right-to-left languages).
    let mut cells: Vec<Element<'a, Message>> = Vec::new();
    if let Some(thumbnail) = target.thumbnail.as_ref() {
        cells.push(result_image(thumbnail, icon_size));
    } else if model.show_icons && !is_character {
        let icon = entry.icon.as_deref().and_then(|name| model.icons.get(name));
        cells.push(match icon {
            Some(icon) => result_image(icon, icon_size),
            None => result_tile(
                theme::icon(
                    result_glyph(entry.icon.as_deref(), target),
                    icon_size - 8.0,
                    theme::palette().accent,
                ),
                icon_size,
            ),
        });
    }
    let title = entry.label.clone();
    let query = if !model.can_go_back && model.launcher_scope.is_some() {
        model
            .search_query
            .split_once(' ')
            .map_or("", |(_, term)| term)
    } else {
        &model.search_query
    };
    let query = query.trim().to_owned();
    let approximate = is_approximate(target);
    // A sectioned list names each kind once, in its heading; rows drop the same
    // leading word from their second line.
    let sectioned = target.section.is_some() && !browsing;
    // Every row of a mixed list reads the same way, selected or not. Browsing
    // lists names alone, one even line each; searching adds what each match is.
    let description = (!browsing)
        .then(|| result_description(model, target))
        .flatten()
        .map(|description| match description.split_once(" · ") {
            Some((kind, rest)) if sectioned && !kind.contains(' ') => rest.to_owned(),
            _ => description,
        });
    // Sections say what each row is (a window to switch to, an app to open);
    // the command bar says once what Enter does for the selected one. Only a
    // window listed beside its own application (two rows alike) ends with
    // what each does: "Switch to window", "New window".
    let cue = sectioned.then(|| paired_cue(model, target)).flatten();
    cells.push(
        responsive(move |bounds| {
            let font = theme::font();
            let title_font = iced::Font {
                weight: if approximate {
                    iced::font::Weight::Normal
                } else {
                    iced::font::Weight::Semibold
                },
                ..font
            };
            let title_size = theme::body_size() + 2.0;
            let detail_size = theme::label_size() + 1.0;
            let title = fit_line(&title, bounds.width, title_size, title_font);
            let mut labels = column![match_title(title, &query, title_size, title_font)]
                .spacing(if browsing { 3 } else { 2 });
            if let Some(description) = &description {
                labels = labels.push(
                    muted_label(fit_line(description, bounds.width, detail_size, font))
                        .size(detail_size)
                        .line_height(1.14)
                        .shaping(iced::widget::text::Shaping::Advanced)
                        .wrapping(iced::widget::text::Wrapping::None),
                );
            }
            container(labels)
                .center_y(Length::Fill)
                .width(Length::Fill)
                .align_x(canopa_ui::direction::start())
                .clip(true)
                .into()
        })
        .into(),
    );
    if let Some(cue) = cue {
        cells.push(
            theme::text(cue)
                .size(theme::label_size())
                .color(theme::palette().muted)
                .wrapping(iced::widget::text::Wrapping::None)
                .into(),
        );
    }
    canopa_ui::direction::row(cells)
        .spacing(7)
        .width(Length::Fill)
        .align_y(iced::Alignment::Center)
        .into()
}

/// The application a result belongs to: the app a window row would launch
/// again, or the app a running application's row opens.
fn result_application(target: &Target) -> Option<&str> {
    target
        .result
        .actions
        .iter()
        .find_map(|named| match &named.action {
            Action::Application { id } => Some(id.as_str()),
            _ => None,
        })
}

/// What choosing a row does, where the same application is listed both as an
/// open window and as an app: the row's first action ("Switch to window",
/// "New window"). Other rows need no label.
fn paired_cue(model: &PanelRenderModel<'_>, target: &Target) -> Option<String> {
    let id = result_application(target)?;
    let window = is_window_result(target);
    let paired = model.menu_entries.iter().any(|entry| match &entry.target {
        PanelMenuEntryTarget::Launcher(other) => {
            is_window_result(other) != window && result_application(other) == Some(id)
        }
        _ => false,
    });
    paired
        .then(|| target.result.actions.first())
        .flatten()
        .map(|first| first.label.clone())
}

/// Open-window results, as opposed to the rows of running applications.
fn is_window_result(target: &Target) -> bool {
    target.result.id.starts_with("window:")
}

fn action_description(target: &Target) -> Option<String> {
    let primary = target.result.actions.first()?;
    if matches!(primary.action, Action::Application { .. }) {
        return Some(fl!(
            "shell-launcher-launch",
            name = target.result.description.clone()
        ));
    }
    action_hint(&primary.action)
}

fn action_hint(action: &Action) -> Option<String> {
    Some(match action {
        Action::TogglePin { shortcut } if shortcut.pinned => fl!("shell-launcher-hint-unpin"),
        Action::TogglePin { .. } => fl!("shell-launcher-hint-pin"),
        Action::EditAlias { .. } => fl!("shell-launcher-hint-alias"),
        _ => return None,
    })
}

fn match_title(
    title: String,
    query: &str,
    size: f32,
    font: iced::Font,
) -> Element<'static, Message> {
    use iced::widget::text::Wrapping;
    if query.chars().count() >= 2
        && let Some(std::ops::Range { start, end }) = highlight_range(&title, query)
    {
        // Text shaping applies one weight per word, so the match is marked by
        // the accent color, which follows single characters, and an underline.
        let spans: [iced::widget::text::Span<'static, (), iced::Font>; 3] = [
            iced::widget::span(title[..start].to_owned()),
            iced::widget::span(title[start..end].to_owned())
                .color(theme::palette().accent)
                .underline(true),
            iced::widget::span(title[end..].to_owned()),
        ];
        return iced::widget::rich_text(spans)
            .font(font)
            .size(size)
            .line_height(1.14)
            .wrapping(Wrapping::None)
            .into();
    }
    theme::text(title)
        .font(font)
        .size(size)
        .line_height(1.14)
        .shaping(iced::widget::text::Shaping::Advanced)
        .wrapping(Wrapping::None)
        .into()
}

/// Where to highlight `query` in `title`: at the start of a word where it
/// matches one ("te" in "Text Editor"), else where it first matches ("te" in
/// "Notes"), so every result shows why it is listed.
fn highlight_range(title: &str, query: &str) -> Option<std::ops::Range<usize>> {
    word_start_match(title, query).or_else(|| crate::app::panel_search::find_folded(title, query))
}

/// Where `query` matches the start of a word of `title`.
fn word_start_match(title: &str, query: &str) -> Option<std::ops::Range<usize>> {
    let mut previous: Option<char> = None;
    for (index, character) in title.char_indices() {
        let starts_word = previous.is_none_or(|before| !before.is_alphanumeric());
        previous = Some(character);
        if !starts_word || !character.is_alphanumeric() {
            continue;
        }
        if let Some(found) = crate::app::panel_search::find_folded(&title[index..], query)
            && found.start == 0
        {
            return Some(index..index + found.end);
        }
    }
    None
}

fn fit_line(text: &str, width: f32, size: f32, font: iced::Font) -> String {
    use unicode_segmentation::UnicodeSegmentation;

    let measure = |value: &str| theme::text_width_in_font(value, size, font);
    if measure(text) <= width {
        return text.to_owned();
    }
    let text_width = width - measure("…");
    if text_width <= 0.0 {
        return String::new();
    }
    let cuts: Vec<_> = text
        .grapheme_indices(true)
        .map(|(index, _)| index)
        .collect();
    let end = cuts.partition_point(|&cut| measure(&text[..cut]) <= text_width);
    let prefix = &text[..cuts[end.saturating_sub(1)]];
    format!("{}…", prefix.trim_end())
}

fn result_image<'a>(handle: &iced::widget::image::Handle, size: f32) -> Element<'a, Message> {
    container(
        iced::widget::image(handle.clone())
            .width(size - 4.0)
            .height(size - 4.0)
            .content_fit(iced::ContentFit::Contain),
    )
    .center(size)
    .into()
}

fn result_tile(icon: Element<'_, Message>, size: f32) -> Element<'_, Message> {
    container(icon)
        .center(size)
        .style(|theme| {
            let palette = canopa_ui::colors(theme);
            container::Style::default()
                .background(palette.text.scale_alpha(0.065))
                .border(iced::Border::default().rounded(8))
        })
        .into()
}

fn result_glyph(name: Option<&str>, target: &Target) -> &'static [u8] {
    match name {
        Some("system-file-manager" | "folder") => return FILES_ICON,
        Some("preferences-system" | "preferences-desktop") => return SETTINGS_ICON,
        Some("text-x-generic") => return DOC_ICON,
        Some("accessories-calculator") => return CALC_ICON,
        _ => {}
    }
    match target.result.actions.first().map(|named| &named.action) {
        Some(Action::FocusWindow { .. }) if is_window_result(target) => WINDOWS_ICON,
        Some(Action::FocusWindow { .. }) => APPS_ICON,
        Some(Action::Provider { .. }) => SETTINGS_ICON,
        Some(Action::TogglePin { .. }) => PIN_ICON,
        Some(Action::EditAlias { .. } | Action::SaveAlias { .. }) => ALIAS_ICON,
        Some(Action::OpenUrl { .. }) => SHORTCUT_ICON,
        Some(Action::Open { .. } | Action::ChooseApplication { .. }) => DOC_ICON,
        Some(Action::Application { .. }) if target.result.id.starts_with("action:") => PLAY_ICON,
        Some(Action::Application { .. } | Action::OpenWith { .. } | Action::Launch { .. }) => {
            APPS_ICON
        }
        _ => SHORTCUT_ICON,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn highlights_mark_the_start_of_a_word() {
        assert_eq!(super::word_start_match("Text Editor", "te"), Some(0..2));
        assert_eq!(super::word_start_match("Notes — Weekly plan", "te"), None);
        assert_eq!(
            super::highlight_range("Notes — Weekly plan", "te"),
            Some(2..4)
        );
        assert_eq!(
            super::word_start_match("Notes — Weekly plan", "we"),
            Some(10..12)
        );
    }
}
