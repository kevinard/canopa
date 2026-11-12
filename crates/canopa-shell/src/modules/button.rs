/// Runtime state for the button module. Button appearance is config-driven, not data-driven.
#[derive(Debug, Clone, PartialEq)]
pub struct ButtonState;

/// Events emitted by the button module.
#[derive(Debug, Clone, PartialEq)]
pub enum ButtonEvent {
    /// Emitted once at startup to transition Empty into Button(ButtonState).
    Ready,
}

pub(super) struct ButtonModule {
    id: crate::modules::ModuleId,
    label: String,
    icon: String,
    format: crate::config::FormatConfig,
    content_rotation: crate::config::ContentRotation,
}

impl ButtonModule {
    #[cfg(test)]
    pub(super) fn new(
        id: crate::modules::ModuleId,
        label: String,
        icon: String,
        format: crate::config::FormatConfig,
    ) -> Self {
        Self::new_with_content_rotation(
            id,
            label,
            icon,
            format,
            crate::config::ContentRotation::None,
        )
    }

    pub(super) fn new_with_content_rotation(
        id: crate::modules::ModuleId,
        label: String,
        icon: String,
        format: crate::config::FormatConfig,
        content_rotation: crate::config::ContentRotation,
    ) -> Self {
        ButtonModule {
            id,
            label,
            icon,
            format,
            content_rotation,
        }
    }

    fn styled_segments_with_default(
        &self,
        mode: crate::config::FormatMode,
        default_style: &crate::config::Style,
    ) -> Option<Vec<(String, crate::config::ResolvedStyle)>> {
        use std::borrow::Cow;

        crate::modules::view::render_format_segments(
            &self.format,
            mode,
            &[
                ("icon", Cow::Borrowed(self.icon.as_str())),
                ("value", Cow::Borrowed(self.label.as_str())),
                ("label", Cow::Borrowed(self.label.as_str())),
            ],
            None,
            default_style,
        )
    }
}

impl crate::modules::Module for ButtonModule {
    fn id(&self) -> crate::modules::ModuleId {
        self.id.clone()
    }

    fn kind(&self) -> crate::modules::ModuleKind {
        crate::modules::ModuleKind::Button
    }

    fn subscription(&self) -> iced::Subscription<crate::app::Message> {
        iced::Subscription::run_with(self.id.clone(), |id: &crate::modules::ModuleId| {
            let id = id.clone();
            iced::futures::stream::unfold(Some(id), |state| async move {
                match state {
                    Some(id) => {
                        let msg = crate::app::Message::Modules(
                            crate::app::messages::ModuleRuntimeEvent::Module(
                                id,
                                crate::modules::ModuleEvent::Button(ButtonEvent::Ready),
                            ),
                        );
                        Some((msg, None))
                    }
                    None => {
                        // Keep subscription alive but emit nothing further.
                        std::future::pending::<()>().await;
                        unreachable!()
                    }
                }
            })
        })
    }
}

/// What the Shell's own providers hold; other providers show only their name.
fn provider_description(provider: &str) -> String {
    match provider {
        "bluetooth" => fl!("shell-provider-bluetooth-description"),
        "power" => fl!("shell-provider-power-description"),
        "wifi" => fl!("shell-provider-wifi-description"),
        "audio" => fl!("shell-volume-devices"),
        _ => String::new(),
    }
}

/// A session action for the Quick settings footer: a quiet pill with its icon
/// and name. Rows of three keep everyday actions apart from ending the session.
fn session_tile<'a>(
    icon: &'static [u8],
    label: String,
    critical: bool,
    opens_menu: bool,
    node: canopa_ui::accessibility::Node<crate::app::Message>,
    primary: Option<crate::app::Message>,
    focused: bool,
) -> iced::Element<'a, crate::app::Message> {
    use iced::widget::{button, container, row};
    let palette = crate::theme::palette();
    let disabled = primary.is_none();
    let tint = if critical {
        palette.danger
    } else {
        palette.text
    };
    let mut content = row![
        crate::theme::icon(icon, 17.0, tint),
        crate::theme::text(label)
            .size(crate::theme::label_size() + 1.0)
            .color(if critical {
                palette.danger
            } else {
                palette.text
            })
            .wrapping(iced::widget::text::Wrapping::None),
    ]
    .spacing(7)
    .align_y(iced::Alignment::Center);
    if opens_menu {
        content = content.push(
            crate::theme::text(canopa_ui::direction::forward_chevron())
                .size(15)
                .style(canopa_ui::muted),
        );
    }
    canopa_ui::accessibility::semantic(
        node,
        button(
            container(content)
                .center_x(iced::Length::Fill)
                .center_y(iced::Length::Fill),
        )
        .padding([0, 6])
        .width(iced::Length::Fill)
        .height(38)
        .on_press_maybe(primary)
        .style(move |theme, status| {
            let colors = canopa_ui::colors(theme);
            let engaged = matches!(status, button::Status::Hovered | button::Status::Pressed);
            let wash = if critical { colors.danger } else { colors.text };
            button::Style {
                background: Some(wash.scale_alpha(if engaged { 0.14 } else { 0.06 }).into()),
                text_color: if disabled {
                    colors.disabled
                } else {
                    colors.text
                },
                border: iced::Border {
                    color: colors.focus,
                    width: if focused { 2.0 } else { 0.0 },
                    radius: 12.0.into(),
                },
                ..Default::default()
            }
        }),
    )
}

impl ButtonModule {
    /// A link row with a level slider beneath it, like the Sound card:
    /// Power & display shows the screen brightness.
    fn level_card(
        &self,
        icon: &'static [u8],
        provider: &str,
        level: u8,
        set: fn(u8) -> crate::app::Message,
        context: crate::modules::render::PanelModuleContext,
    ) -> iced::Element<'static, crate::app::Message> {
        use canopa_ui::accessibility::{Node, Range, semantic};
        let key = context.key.clone();
        let heading = crate::modules::view::panel_control(
            icon,
            self.label.clone(),
            context
                .provider_summary
                .clone()
                .unwrap_or_else(|| provider_description(provider)),
            crate::modules::view::PanelControlEnd::Navigate,
            crate::modules::render::PanelModuleContext {
                provider_switch: None,
                provider_level: None,
                ..context
            },
        );
        let level = level.clamp(1, 100);
        let text = canopa_i18n::format::percent(f64::from(level));
        let node = Node::slider(
            fl!("shell-osd-brightness"),
            Range {
                value: f64::from(level),
                min: 1.0,
                max: 100.0,
                step: Some(5.0),
            },
        )
        .key(format!("{key}/level"))
        .value(text.clone())
        .on_increment(Some(set(level.saturating_add(5).min(100))))
        .on_decrement(Some(set(level.saturating_sub(5).max(1))));
        let slider = crate::theme::level_slider(
            f32::from(level),
            Some(move |value: f32| set(value.round().clamp(1.0, 100.0) as u8)),
            false,
        );
        let glyph = iced::widget::container(
            iced::widget::svg(iced::widget::svg::Handle::from_memory(include_bytes!(
                "../../assets/brightness.svg"
            )))
            .width(18)
            .height(18)
            .style(|theme: &iced::Theme, _| iced::widget::svg::Style {
                color: Some(canopa_ui::colors(theme).muted),
            }),
        )
        .center_x(32)
        .center_y(32);
        iced::widget::column![
            heading,
            canopa_ui::direction::row([
                glyph.into(),
                semantic(node, slider),
                iced::widget::container(
                    crate::theme::strong_text(text)
                        .size(crate::theme::label_size())
                        .style(canopa_ui::muted),
                )
                .width(40)
                .align_x(canopa_ui::direction::end())
                .into(),
            ])
            .spacing(12)
            .padding([0, 6])
            .align_y(iced::Alignment::Center),
        ]
        .spacing(4)
        .into()
    }
}

impl crate::modules::render::ModuleRender for ButtonModule {
    fn render_bar_panel_with_style(
        &self,
        state: &crate::modules::ModuleState,
        style: &crate::config::Style,
        context: crate::modules::render::PanelModuleContext,
    ) -> Option<iced::Element<'_, crate::app::Message>> {
        use iced::widget::container;
        let node = canopa_ui::accessibility::Node::button(self.label.clone())
            .key(context.key.clone())
            .disabled(context.primary.is_none())
            .focused(context.focused)
            .on_click(context.primary.clone());
        if let Some(icon) = crate::modules::view::embedded_icon(&self.icon) {
            if context.footer {
                return Some(session_tile(
                    icon,
                    self.label.clone(),
                    matches!(self.icon.as_str(), "canopa:poweroff"),
                    // Opening a menu, it says so with a chevron.
                    context.opens_menu,
                    node,
                    context.primary,
                    context.focused,
                ));
            }
            // Links to another panel are full-width rows, like Network and
            // Sound; tiles are reserved for switches.
            if let Some(provider) = context.opens_provider.clone() {
                let focused = context.focused;
                // A paired radio is a tile and draws its own frame.
                if context.paired_toggle
                    && let Some((on, toggle)) = context.provider_switch.clone()
                {
                    let description = context
                        .provider_summary
                        .clone()
                        .unwrap_or_else(|| provider_description(&provider));
                    return Some(crate::modules::view::panel_control(
                        icon,
                        self.label.clone(),
                        description,
                        crate::modules::view::PanelControlEnd::NavigateSwitch {
                            on,
                            toggle: Some(Box::new(toggle)),
                        },
                        context,
                    ));
                }
                if let Some((level, set)) = context.provider_level {
                    return Some(crate::surface::panel::panel_module_frame(
                        self.level_card(icon, &provider, level, set, context),
                        focused,
                    ));
                }
                let end = match context.provider_switch.clone() {
                    Some((on, toggle)) => crate::modules::view::PanelControlEnd::NavigateSwitch {
                        on,
                        toggle: Some(Box::new(toggle)),
                    },
                    None => crate::modules::view::PanelControlEnd::Navigate,
                };
                return Some(crate::surface::panel::panel_module_frame(
                    crate::modules::view::panel_control(
                        icon,
                        self.label.clone(),
                        context
                            .provider_summary
                            .clone()
                            .unwrap_or_else(|| provider_description(&provider)),
                        end,
                        context,
                    ),
                    focused,
                ));
            }
            return Some(crate::modules::view::panel_tile(
                icon,
                self.label.clone(),
                None,
                false,
                false,
                node,
                context.primary,
                context.focused,
            ));
        }
        let content = self.render_panel_with_style(state, style);
        let content = if context.opens_provider.is_some() {
            canopa_ui::direction::row([
                container(content).width(iced::Length::Fill).into(),
                crate::theme::text(canopa_ui::direction::forward_chevron())
                    .size(22)
                    .style(canopa_ui::muted)
                    .into(),
            ])
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .into()
        } else {
            content
        };
        let mut button = crate::theme::action(
            container(content)
                .height(iced::Length::Fill)
                .align_y(iced::Alignment::Center),
            context.primary,
            if context.footer {
                canopa_ui::Action::Normal
            } else {
                canopa_ui::Action::Quiet
            },
            context.focused,
        )
        .width(iced::Length::Fill)
        .height(44);
        if context.opens_provider.is_some()
            && !crate::theme::ui(canopa_theme::UiKind::Button).custom_padding
        {
            button = button.padding([8, 4]);
        }
        let button = canopa_ui::accessibility::semantic(node, button);
        Some(if context.footer {
            button
        } else {
            crate::surface::panel::panel_module_frame(button, false)
        })
    }

    fn presentation(&self) -> Option<&dyn crate::modules::view::IconLabel> {
        Some(self)
    }

    fn tooltip_values(&self, _state: &crate::modules::ModuleState) -> Vec<(&'static str, String)> {
        vec![
            ("icon", self.icon.clone()),
            ("value", self.label.clone()),
            ("label", self.label.clone()),
        ]
    }
}

impl crate::modules::view::IconLabel for ButtonModule {
    fn content_rotation(&self) -> crate::config::ContentRotation {
        self.content_rotation
    }
    fn accepts_state(&self, state: &crate::modules::ModuleState) -> bool {
        matches!(state, crate::modules::ModuleState::Button(_))
    }
    fn icon(&self, _state: &crate::modules::ModuleState) -> String {
        self.icon.clone()
    }
    fn label(&self, _state: &crate::modules::ModuleState) -> String {
        self.label.clone()
    }
    fn styled_segments_for_layout(
        &self,
        _state: &crate::modules::ModuleState,
        layout: crate::modules::view::IconLabelLayout,
        style: &crate::config::Style,
    ) -> Option<Vec<(String, crate::config::ResolvedStyle)>> {
        self.styled_segments_with_default(layout.into(), style)
    }
    fn formatted_color(
        &self,
        _state: &crate::modules::ModuleState,
        _layout: crate::modules::view::IconLabelLayout,
    ) -> iced::Color {
        crate::theme::palette().text
    }
    fn formatted_size(&self, layout: crate::modules::view::IconLabelLayout) -> u32 {
        if layout == crate::modules::view::IconLabelLayout::Expanded {
            13
        } else {
            crate::theme::label_size() as u32
        }
    }
    fn fallback_view(
        &self,
        state: &crate::modules::ModuleState,
        layout: crate::modules::view::IconLabelLayout,
        style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        match layout {
            crate::modules::view::IconLabelLayout::Collapsed
                if matches!(
                    self.icon.as_str(),
                    "canopa:search" | "canopa:overview" | "canopa:controls"
                ) =>
            {
                // Icon-only on the spine: the name is the tooltip and the
                // accessible label, so the rail stays calm and legible.
                iced::widget::container(crate::modules::view::styled_segment_text(
                    self.icon(state),
                    22,
                    crate::theme::palette().text,
                    crate::modules::view::fallback_resolved_style(style),
                ))
                .center_x(iced::Length::Fill)
                .into()
            }
            crate::modules::view::IconLabelLayout::Expanded => {
                crate::modules::view::spaced_icon_label(self.icon(state), self.label(state), style)
            }
            _ => crate::modules::view::default_icon_label_view(self, state, layout, style),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ColorValue, SegmentStyles, Style, ThemeToken, Weight};
    use crate::modules::Module;

    fn styled_format_config() -> crate::config::FormatConfig {
        let mut template = crate::config::FormatTemplate::parse(
            &["{label}"],
            &["icon", "value", "label"],
            "test.format",
        )
        .unwrap();
        template.set_segment_styles(
            0,
            SegmentStyles::Static(Style {
                color: Some(ColorValue::Token(ThemeToken::Accent)),
                weight: Some(Weight::Bold),
                ..Default::default()
            }),
        );
        crate::config::FormatConfig {
            collapsed: Some(template),
            ..Default::default()
        }
    }

    #[test]
    fn button_module_id_is_button() {
        let m = ButtonModule::new(
            crate::modules::ModuleId::new("button"),
            "Launch".into(),
            ">".into(),
            crate::config::FormatConfig::default(),
        );
        assert_eq!(m.id().display_name(), "button");
        assert_eq!(m.module_type(), "button");
    }

    #[test]
    fn button_state_is_debug() {
        let s = ButtonState;
        // Just verify Debug formatting doesn't panic.
        let _ = format!("{:?}", s);
    }

    #[test]
    fn button_event_ready_variant() {
        // Verify ButtonEvent::Ready constructs and is Debug-formattable.
        let e = ButtonEvent::Ready;
        let _ = format!("{:?}", e);
    }

    #[test]
    fn button_styled_segments_resolve_inline_style() {
        let m = ButtonModule::new(
            crate::modules::ModuleId::new("button"),
            "Launch".into(),
            ">".into(),
            styled_format_config(),
        );
        let segment = &m
            .styled_segments_with_default(
                crate::config::FormatMode::Collapsed,
                &crate::config::Style::default(),
            )
            .unwrap()[0];

        assert_eq!(segment.1.color, Some(crate::theme::palette().accent));
        assert_eq!(segment.1.weight, Some(Weight::Bold));
    }
}
