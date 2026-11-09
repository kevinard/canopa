use std::rc::Rc;

use iced::widget::{Space, column, container, keyed_column, responsive, row, stack};
use iced::{Element, Length, Padding, Size};

use super::super::launcher::{APPROXIMATE_KEYWORD_MATCH, QuickScope, Target};
use super::super::messages::PanelEvent;
use super::super::panel_menu_session::{PanelMenuEntry, PanelMenuEntryTarget};
use super::{Message, PanelRenderModel, panel_surface, render_menu_scroll, styled_panel_frame};
use crate::{launcher::Action, theme};

const CLOSE_ICON: &[u8] = include_bytes!("../../../assets/close.svg");
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
    let close_icon = theme::icon(CLOSE_ICON, 16.0, theme::palette().text);
    let close_label: Element<'a, Message> = if wide {
        row![close_icon, theme::strong_text("Close").size(caption_size())]
            .spacing(5)
            .align_y(iced::Alignment::Center)
            .into()
    } else {
        container(close_icon).center(20).into()
    };
    let close = control_hint(
        theme::action(
            close_label,
            Some(Message::Panel(PanelEvent::Close)),
            canopa_ui::Action::Quiet,
            false,
        )
        .padding([6, 9]),
        if !child && model.search_query.is_empty() {
            "Close launcher · Esc"
        } else {
            "Close launcher"
        },
    );
    let chrome = 208.0 + ui.margin.vertical() + ui.padding.vertical();
    let metrics = model.menu_metrics;
    let calculator = !child
        && count == 1
        && target.is_some_and(|target| is_calculator(&model.menu_entries[0], target));
    let empty_recovery = count == 0
        && matches!(model.launcher_status.as_str(), "No matches" | "")
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
        let visible_rows = count.min(if browsing { 5 } else { 3 });
        metrics.extent * visible_rows as f32 - metrics.gap
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
            row![
                back_button(model.search_query.is_empty()),
                theme::strong_text(title)
                    .size(theme::body_size() + 2.0)
                    .wrapping(iced::widget::text::Wrapping::None),
            ]
            .align_y(iced::Alignment::Center)
            .spacing(12),
        );
    } else if calculator {
        controls = controls.push(
            row![
                theme::icon(CALC_ICON, 18.0, theme::palette().accent),
                theme::strong_text("Calculator").size(caption_size()),
            ]
            .align_y(iced::Alignment::Center)
            .spacing(6),
        );
    } else if browsing && count > 0 {
        let mut header = row![
            theme::strong_text(browse_heading(model)).size(theme::body_size() + 1.0),
            Space::new().width(Length::Fill),
        ]
        .align_y(iced::Alignment::Center)
        .spacing(4);
        if wide {
            header = header
                .push(muted_label("Type"))
                .push(keycap("="))
                .push(muted_label("to calculate"));
        }
        controls = controls.push(header);
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
            padded_results(model, session, Length::Fixed(preferred_body))
        }
    } else {
        let model = Rc::clone(model);
        responsive(move |space| {
            let metrics = model.menu_metrics;
            let available = (space.height - 8.0).max(0.0);
            let rows = ((available + metrics.gap) / metrics.extent).floor();
            let height = if rows >= 1.0 && available.is_finite() {
                (rows * metrics.extent - metrics.gap).min(available)
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
    let mut results = column![].spacing(8);
    if !has_scopes && count > 0 {
        results = results.push(footer_rule());
    }
    results = results.push(body);
    if let Some(target) = target {
        if !model.launcher_status.is_empty() {
            results = results.push(muted_label(model.launcher_status.clone()));
        }
        let shortcuts = secondary_commands(target);
        if !shortcuts.is_empty() {
            results = results.push(row(shortcuts).spacing(4).wrap());
        }
        let mut footer = row![].spacing(7).align_y(iced::Alignment::Center);
        if width >= 380.0 {
            if count > 1 {
                footer = footer
                    .push(keycap("↑"))
                    .push(keycap("↓"))
                    .push(muted_label("Select"));
            } else if !child && !calculator {
                footer = footer.push(muted_label(result_count(model)));
            }
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
    let content = column![
        container(controls).padding(horizontal.top(top)),
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

fn browse_heading(model: &PanelRenderModel<'_>) -> &'static str {
    let apps_only = model.menu_entries.iter().all(|entry| {
        matches!(&entry.target, PanelMenuEntryTarget::Launcher(target)
            if target.result.actions.first().is_some_and(|named| matches!(named.action, Action::Application { .. })))
    });
    if apps_only {
        "Installed apps"
    } else {
        "Browse"
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
    theme::action(
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
    })
    .into()
}

fn command_controls(
    model: &PanelRenderModel<'_>,
    target: &Target,
    width: f32,
) -> Element<'static, Message> {
    let grouped = target.result.actions.len() > 1;
    let mut toolbar = row![].spacing(6).align_y(iced::Alignment::Center);
    if grouped {
        let action = theme::action(
            command_label("Actions", "Tab"),
            Some(Message::Panel(PanelEvent::LauncherActions(Some(
                target.clone(),
            )))),
            canopa_ui::Action::Normal,
            false,
        )
        .padding([5, 9]);
        toolbar = toolbar.push(control_hint(
            action,
            format!("Actions for {}", target.result.label),
        ));
    }
    if let Some(primary) = target.result.actions.first() {
        let label = if model.can_go_back && target.result.id.starts_with("action:") {
            target.result.label.clone()
        } else if primary.label == "Open" && model.menu_entries.len() == 1 && width >= 450.0 {
            format!("Open {}", target.result.label)
        } else {
            primary.label.clone()
        };
        let label = fit_line(
            &label,
            if width >= 465.0 { 170.0 } else { 92.0 },
            caption_size(),
            theme::font(),
        );
        toolbar = toolbar.push(
            theme::action(
                row![
                    theme::strong_text(label).size(caption_size()),
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
        );
    }
    toolbar.into()
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
    if escape {
        return command_button("‹ Back", "Esc", PanelEvent::Back).into();
    }
    control_hint(
        theme::action(
            theme::strong_text("‹ Back").size(caption_size()),
            Some(Message::Panel(PanelEvent::Back)),
            canopa_ui::Action::Normal,
            false,
        )
        .padding([6, 10]),
        "Back",
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
    container(list)
        .padding(Padding::default().right(inset))
        .width(Length::Fill)
        .into()
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
    let scoped_placeholder;
    let placeholder = if let Some(scope) = scope {
        scoped_placeholder = format!("Search {}", scope.label().to_ascii_lowercase());
        &scoped_placeholder
    } else if let Some(placeholder) = model.launcher_filter_placeholder {
        placeholder
    } else {
        model.menu_prompt.as_deref().unwrap_or("Search")
    };
    let prefix = scope.map_or("", |scope| scope.prefix());
    let query = visible_query(model);
    use canopa_theme::{ColorRole, ColorValue};
    let mut input_ui = theme::ui(canopa_theme::UiKind::Input);
    if input_ui.background == ColorValue::Role(ColorRole::Raised) {
        input_ui.background = ColorValue::Role(ColorRole::Background);
    }
    if !input_ui.custom_border && input_ui.states[4].border_color.is_none() {
        input_ui.states[4].border_color = Some(ColorValue::Rgba(
            theme::appearance().role(ColorRole::Focus),
            145,
        ));
    }
    let mut padding = canopa_ui::content_padding(input_ui);
    if !input_ui.custom_padding {
        padding.top = 14.0;
        padding.bottom = 14.0;
    }
    let clearable = show_clear && !query.is_empty();
    if clearable {
        padding.right += theme::text_width("Clear", caption_size()) + 27.0;
        if show_clear_shortcut {
            padding.right += theme::text_width("Esc", caption_size()) + 16.0;
        }
    }
    let icon_inset = padding.left;
    padding.left += 28.0;
    let input = theme::input_with_style(placeholder, query, input_ui)
        .id(model.search_input_id.clone())
        .on_input(move |query| {
            Message::Panel(PanelEvent::LauncherSearchChanged {
                session,
                query: format!("{prefix}{query}"),
            })
        })
        .size(theme::body_size() + 2.0)
        .padding(padding)
        .width(Length::Fill);
    let glyph = container(theme::icon(SEARCH_ICON, 20.0, theme::palette().accent))
        .height(Length::Fill)
        .align_y(iced::Alignment::Center)
        .padding(Padding::default().left(icon_inset));
    if !clearable {
        return stack![input, glyph].into();
    }

    let mut clear_label = row![
        theme::strong_text("Clear")
            .size(caption_size())
            .style(canopa_ui::link)
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center);
    if show_clear_shortcut {
        clear_label = clear_label.push(keycap("Esc"));
    }
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
        "Clear search · Esc",
    );
    stack![
        input,
        glyph,
        container(clear)
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
    let label = scope.map_or("All", |scope| scope.label());
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
        theme::strong_text(label)
            .size(caption_size())
            .style(canopa_ui::link)
    } else {
        theme::text(label).size(caption_size())
    });
    let width = if equal_width {
        Length::Fill
    } else {
        Length::Shrink
    };
    iced::widget::button(container(content).center_x(width))
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
                style.background = Some(palette.accent.scale_alpha(0.16).into());
            } else if matches!(
                status,
                iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
            ) {
                style.background = Some(palette.text.scale_alpha(0.09).into());
            }
            style
        })
        .into()
}

fn empty<'a>(
    model: &PanelRenderModel<'a>,
    session: u64,
    width: f32,
    recovery: bool,
    height: Length,
) -> Element<'a, Message> {
    let title = if model.launcher_status.is_empty() {
        "No matches".to_owned()
    } else {
        model.launcher_status.clone()
    };
    let mut labels = column![theme::strong_text(title).size(theme::body_size() + 2.0)].spacing(5);
    if recovery {
        let advice = if model.launcher_scope.is_some() {
            "Try another term or choose All."
        } else {
            "Check the spelling or try a shorter query."
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
        let clear = command_button("Clear search", "Esc", clear_query(session));
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
    if count == 1 {
        return "1 match".into();
    }
    let approximate = model.menu_entries.iter().filter(|entry| {
        matches!(&entry.target, PanelMenuEntryTarget::Launcher(target) if is_approximate(target))
    }).count();
    if approximate > 0 && approximate < count {
        format!("{count} matches · {approximate} approximate")
    } else {
        format!("{count} matches")
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

fn secondary_commands(target: &Target) -> Vec<Element<'static, Message>> {
    target
        .result
        .actions
        .iter()
        .enumerate()
        .skip(1)
        .take(4)
        .filter(|(_, named)| {
            !matches!(
                named.action,
                Action::TogglePin { .. } | Action::EditAlias { .. }
            )
        })
        .map(|(position, named)| {
            command_button(
                &named.label,
                format!("Ctrl+{}", position + 1),
                PanelEvent::ActivateLauncher(target.clone(), position),
            )
            .into()
        })
        .collect()
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
    let style = canopa_ui::Action::Normal;
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
    container(theme::text("Enter").size(theme::label_size() + 1.0))
        .padding([2, 5])
        .style(|theme| {
            let palette = canopa_ui::colors(theme);
            container::Style::default()
                .background(palette.on_accent.scale_alpha(0.14))
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

pub(super) fn scrollbar_gutter(model: &PanelRenderModel<'_>, height: Length) -> f32 {
    if let Length::Fixed(height) = height {
        let metrics = model.menu_metrics;
        let content_height = model.menu_entries.len() as f32 * metrics.extent - metrics.gap;
        if content_height <= height {
            return 0.0;
        }
    }
    model.ui.padding.right.px().min(12.0)
}

fn is_approximate(target: &Target) -> bool {
    target.display_description.as_deref() == Some(APPROXIMATE_KEYWORD_MATCH)
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
    entry: &PanelMenuEntry,
    target: &Target,
    selected: bool,
) -> Element<'a, Message> {
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
    let mut contents = row![].spacing(7).align_y(iced::Alignment::Center);
    if default_selection {
        contents = contents.push(selection_indicator(selected));
    }
    contents = contents.push(result_content(model, entry, target));
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

    crate::surface::pointer_focus::pointer_focus(
        container(label).padding(canopa_ui::padding(style.margin)),
        Message::Panel(PanelEvent::FocusLauncher(target.clone())),
    )
}

fn result_content<'a>(
    model: &PanelRenderModel<'a>,
    entry: &PanelMenuEntry,
    target: &Target,
) -> Element<'a, Message> {
    let result = &target.result;
    let is_action = result.id.starts_with("action:") && model.can_go_back;
    let is_character = target.provider == crate::launcher::characters::PROVIDER;
    let browsing = !model.can_go_back && model.search_query.trim().is_empty();
    let icon_size = 30.0;
    let mut content = row![]
        .spacing(7)
        .width(Length::Fill)
        .align_y(iced::Alignment::Center);
    if let Some(thumbnail) = target.thumbnail.as_ref() {
        content = content.push(result_image(thumbnail, icon_size));
    } else if model.show_icons && !is_character {
        let icon = entry.icon.as_deref().and_then(|name| model.icons.get(name));
        content = content.push(match icon {
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
    let query = query.trim().to_ascii_lowercase();
    let approximate = is_approximate(target);
    let description = target
        .display_description
        .as_deref()
        .unwrap_or(&result.description);
    let description = if !model.can_go_back && model.menu_entries.len() == 1 {
        target.display_detail.as_deref().unwrap_or(description)
    } else {
        description
    };
    let description = if is_action {
        action_description(target)
    } else if (!browsing || description != "Application") && !description.is_empty() {
        Some(description.to_owned())
    } else {
        None
    };
    content = content.push(responsive(move |bounds| {
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
        let detail_size = if browsing {
            theme::label_size() + 1.0
        } else {
            caption_size()
        };
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
            .clip(true)
            .into()
    }));
    content.into()
}

fn action_description(target: &Target) -> Option<String> {
    let primary = target.result.actions.first()?;
    if matches!(primary.action, Action::Application { .. }) {
        return Some(format!("Launch {}", target.result.description));
    }
    action_hint(&primary.action).map(str::to_owned)
}

fn action_hint(action: &Action) -> Option<&'static str> {
    Some(match action {
        Action::TogglePin { shortcut } if shortcut.pinned => {
            "Remove this result from launcher suggestions"
        }
        Action::TogglePin { .. } => "Keep this result in launcher suggestions",
        Action::EditAlias { .. } => "Find this result with a custom search term",
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
        && let Some(start) = title.to_ascii_lowercase().find(query)
    {
        let end = start + query.len();
        let spans: [iced::widget::text::Span<'static, (), iced::Font>; 3] = [
            iced::widget::span(title[..start].to_owned()),
            iced::widget::span(title[start..end].to_owned()).font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..font
            }),
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
        Some(Action::FocusWindow { .. }) => WINDOWS_ICON,
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
