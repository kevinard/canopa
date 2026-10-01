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

impl crate::modules::render::ModuleRender for ButtonModule {
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
            crate::theme::bar_metrics::TEXT_SIZE
        }
    }
    fn fallback_view(
        &self,
        state: &crate::modules::ModuleState,
        layout: crate::modules::view::IconLabelLayout,
        style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        match layout {
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
