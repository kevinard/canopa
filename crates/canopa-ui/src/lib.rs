//! Shared Iced visual treatment. Callers retain events, focus, identity and accessibility trees.
mod customization;
pub mod panel;
pub use canopa_theme::metrics;
use canopa_theme::{Appearance, ColorScheme, Rgb};
pub use customization::*;
use iced::{
    Color, Theme,
    widget::{button, checkbox, container, text},
};

pub fn color(Rgb(r, g, b): Rgb) -> Color {
    Color::from_rgb8(r, g, b)
}

/// Resolve one immutable snapshot into Iced's complete widget palette.
pub fn theme(appearance: Appearance) -> Theme {
    use iced::theme::{
        Palette,
        palette::{Extended, Pair},
    };
    let p = appearance.palette();
    let pair = |background, foreground| Pair {
        color: color(background),
        text: color(foreground),
    };
    Theme::custom_with_fn(
        "Canopa",
        Palette {
            background: color(p.background),
            text: color(p.text),
            primary: color(p.accent),
            success: color(p.success),
            warning: color(p.warning),
            danger: color(p.danger),
        },
        |palette| {
            let mut extended = Extended::generate(palette);
            extended.is_dark = appearance.color_scheme == ColorScheme::Dark;
            extended.background.base = pair(p.background, p.text);
            extended.background.weakest = pair(p.surface, p.text);
            extended.background.weaker = pair(p.surface, p.text);
            extended.background.weak = pair(p.surface, p.text);
            extended.background.neutral = pair(p.raised, p.text);
            extended.background.strong = pair(p.raised, p.text);
            extended.background.stronger = pair(p.border, p.text);
            extended.background.strongest = pair(p.disabled, p.text);
            extended.secondary.base = pair(p.muted, p.background);
            extended.primary.base =
                pair(p.accent, appearance.role(canopa_theme::ColorRole::OnAccent));
            extended.danger.base =
                pair(p.danger, appearance.role(canopa_theme::ColorRole::OnDanger));
            extended.secondary.strong = pair(
                appearance.role(canopa_theme::ColorRole::Focus),
                p.background,
            );
            extended
        },
    )
}

#[derive(Clone, Copy, Debug)]
pub struct Colors {
    pub background: Color,
    pub surface: Color,
    pub raised: Color,
    pub text: Color,
    pub muted: Color,
    pub disabled: Color,
    pub border: Color,
    pub accent: Color,
    pub focus: Color,
    pub on_accent: Color,
    pub danger: Color,
    pub on_danger: Color,
    pub warning: Color,
    pub success: Color,
}

/// Semantic roles, mapped centrally onto the palette produced by `theme`.
pub fn colors(theme: &Theme) -> Colors {
    let p = theme.extended_palette();
    Colors {
        background: p.background.base.color,
        surface: p.background.weak.color,
        raised: p.background.strong.color,
        text: p.background.base.text,
        muted: p.secondary.base.color,
        disabled: p.background.strongest.color,
        border: p.background.stronger.color,
        accent: p.primary.base.color,
        focus: p.secondary.strong.color,
        on_accent: p.primary.base.text,
        danger: p.danger.base.color,
        on_danger: p.danger.base.text,
        warning: theme.palette().warning,
        success: theme.palette().success,
    }
}

/// Keep unfilled checkbox boundaries visible on both shared surface palettes.
pub fn checkbox_style(theme: &Theme, status: checkbox::Status) -> checkbox::Style {
    let p = colors(theme);
    let mut style = checkbox::primary(theme, status);
    style.border.color = match status {
        checkbox::Status::Hovered { .. } => p.focus,
        checkbox::Status::Disabled { .. } => p.disabled,
        checkbox::Status::Active { .. } => p.border,
    };
    style
}

pub fn surface(theme: &Theme) -> container::Style {
    let p = colors(theme);
    container::Style {
        background: Some(p.background.into()),
        text_color: Some(p.text),
        border: iced::Border {
            color: p.border,
            width: 1.0,
            radius: metrics::SURFACE_RADIUS.into(),
        },
        ..Default::default()
    }
}

pub fn muted(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(colors(theme).muted),
    }
}

/// Links retain readable text even when an explicit accent override has poor contrast.
pub fn link(theme: &Theme) -> text::Style {
    let p = colors(theme);
    let rgb = |c: Color| {
        Rgb(
            (c.r * 255.0).round() as u8,
            (c.g * 255.0).round() as u8,
            (c.b * 255.0).round() as u8,
        )
    };
    text::Style {
        color: Some(
            if rgb(p.accent).contrast(rgb(p.background)) >= 4.5
                && rgb(p.accent).contrast(rgb(p.surface)) >= 4.5
            {
                p.accent
            } else {
                p.text
            },
        ),
    }
}

#[derive(Clone, Copy)]
pub enum Action {
    Normal,
    Quiet,
    Destructive,
}

pub fn action_style(
    theme: &Theme,
    status: button::Status,
    action: Action,
    focused: bool,
) -> button::Style {
    let p = colors(theme);
    let disabled = status == button::Status::Disabled;
    let engaged = matches!(status, button::Status::Hovered | button::Status::Pressed);
    let foreground = match action {
        Action::Destructive => p.danger,
        _ => p.text,
    };
    button::Style {
        background: if engaged {
            // Quiet controls can contain explicitly styled links and muted body text.
            // Their foreground roles need the same contrast as the resting surface.
            Some(
                if matches!(action, Action::Normal) {
                    p.raised
                } else {
                    p.surface
                }
                .into(),
            )
        } else if matches!(action, Action::Normal) {
            Some(p.surface.into())
        } else {
            None
        },
        text_color: if disabled { p.disabled } else { foreground },
        border: iced::Border {
            color: if focused && !disabled {
                p.focus
            } else {
                p.border
            },
            width: if focused && !disabled {
                2.0
            } else if matches!(action, Action::Normal) {
                1.0
            } else {
                0.0
            },
            radius: metrics::CONTROL_RADIUS.into(),
        },
        ..Default::default()
    }
}

/// A standard Iced button: preserves toolkit input semantics and caller-owned messages.
pub fn action<'a, Message: Clone + 'a>(
    content: impl Into<iced::Element<'a, Message>>,
    on_press: Option<Message>,
    treatment: Action,
    focused: bool,
) -> button::Button<'a, Message> {
    button(content)
        .padding([4, 8])
        .on_press_maybe(on_press)
        .style(move |theme, status| action_style(theme, status, treatment, focused))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn consumers_share_surface_action_and_focus_roles() {
        for scheme in [ColorScheme::Dark, ColorScheme::Light] {
            let appearance = Appearance {
                color_scheme: scheme,
                ..Default::default()
            };
            let theme = theme(appearance.clone());
            let p = colors(&theme);
            assert_eq!(p.background, color(appearance.palette().background));
            assert_eq!(surface(&theme).text_color, Some(p.text));
            let active = action_style(&theme, button::Status::Active, Action::Normal, false);
            let hover = action_style(&theme, button::Status::Hovered, Action::Normal, false);
            assert_ne!(active.background, hover.background);
            let focus = action_style(&theme, button::Status::Active, Action::Normal, true);
            assert_eq!(focus.border.color, p.accent);
            assert_eq!(focus.border.width, 2.0);
            assert_eq!(
                action_style(&theme, button::Status::Disabled, Action::Normal, true).text_color,
                p.disabled
            );
            assert_eq!(link(&theme).color, Some(p.accent));
        }
    }
    #[test]
    fn custom_accent_does_not_hide_links() {
        let theme = theme(Appearance {
            accent: Some(Rgb(30, 30, 46)),
            ..Default::default()
        });
        assert_eq!(link(&theme).color, Some(colors(&theme).text));
    }

    #[test]
    fn hovered_and_pressed_controls_preserve_nested_text_contrast() {
        let rgb = |c: Color| {
            Rgb(
                (c.r * 255.0).round() as u8,
                (c.g * 255.0).round() as u8,
                (c.b * 255.0).round() as u8,
            )
        };
        for color_scheme in [ColorScheme::Dark, ColorScheme::Light] {
            let theme = theme(Appearance {
                color_scheme,
                ..Default::default()
            });
            for status in [button::Status::Hovered, button::Status::Pressed] {
                for (action, foregrounds) in [
                    (
                        Action::Quiet,
                        vec![muted(&theme).color.unwrap(), link(&theme).color.unwrap()],
                    ),
                    (Action::Destructive, vec![colors(&theme).danger]),
                    (Action::Normal, vec![colors(&theme).text]),
                ] {
                    let Some(iced::Background::Color(background)) =
                        action_style(&theme, status, action, false).background
                    else {
                        panic!("missing hover background")
                    };
                    for foreground in foregrounds {
                        assert!(
                            rgb(foreground).contrast(rgb(background)) >= 4.5,
                            "{color_scheme:?}"
                        );
                    }
                }
            }
        }
    }
}
