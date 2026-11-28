//! Shared Iced visual treatment and, with the `accessibility` feature, AT-SPI
//! trees. Callers retain events, focus, identity and action validation.
#[macro_use]
mod i18n;
#[cfg(feature = "accessibility")]
pub mod accessibility;
mod customization;
pub mod direction;
pub mod glass;
#[cfg(feature = "icons")]
pub mod icons;
pub mod panel;
mod scroll_speed;
pub use canopa_theme::metrics;
use canopa_theme::{Appearance, ColorScheme, Rgb};
pub use customization::*;
use iced::{
    Color, Theme,
    widget::{button, checkbox, container, text},
};
pub use scroll_speed::scroll_speed;

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
    // An empty box shows the surface it sits on, so it never reads as a hole.
    if let checkbox::Status::Active { is_checked: false }
    | checkbox::Status::Hovered { is_checked: false }
    | checkbox::Status::Disabled { is_checked: false } = status
    {
        style.background = Color::TRANSPARENT.into();
    }
    style.border.color = match status {
        checkbox::Status::Hovered { .. } => p.focus,
        checkbox::Status::Disabled { .. } => p.disabled,
        // An empty box's edge leans toward the text so it reads on any card.
        checkbox::Status::Active { is_checked: false } => blend(p.border, p.text, 0.25),
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
    Primary,
    Normal,
    Quiet,
    Destructive,
}

/// `from` moved `amount` of the way toward `to`, opaque.
fn blend(from: Color, to: Color, amount: f32) -> Color {
    Color {
        r: from.r + (to.r - from.r) * amount,
        g: from.g + (to.g - from.g) * amount,
        b: from.b + (to.b - from.b) * amount,
        a: 1.0,
    }
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
        Action::Primary => p.on_accent,
        Action::Destructive => p.danger,
        _ => p.text,
    };
    button::Style {
        background: if matches!(action, Action::Primary) {
            (!disabled).then_some(p.accent.into())
        } else if engaged {
            // Quiet controls can contain explicitly styled links and muted body text.
            // Their foreground roles need the same contrast as the resting surface.
            // Quiet controls sit on surfaces (menus, sidebars) of the surface
            // colour itself; their hover leans toward the text so it shows there.
            Some(
                if matches!(action, Action::Normal) {
                    p.raised
                } else {
                    // The lean stays small enough for muted text and links to keep
                    // their contrast on it.
                    let lean = if status == button::Status::Pressed {
                        0.07
                    } else {
                        0.06
                    };
                    blend(p.surface, p.text, lean)
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
            } else if engaged && matches!(action, Action::Primary) {
                p.on_accent
            } else {
                p.border
            },
            width: if focused && !disabled {
                2.0
            } else if matches!(action, Action::Normal)
                || engaged && matches!(action, Action::Primary)
            {
                1.0
            } else {
                0.0
            },
            radius: metrics::CONTROL_RADIUS.into(),
        },
        ..Default::default()
    }
}

/// An on/off switch: the accent track when on, a quiet outlined track when
/// off, with a light knob in both.
pub fn switch_style(
    theme: &Theme,
    status: iced::widget::toggler::Status,
) -> iced::widget::toggler::Style {
    use iced::widget::toggler::Status;
    let p = colors(theme);
    let mut style = iced::widget::toggler::default(theme, status);
    let (on, hovered, disabled) = match status {
        Status::Active { is_toggled } => (is_toggled, false, false),
        Status::Hovered { is_toggled } => (is_toggled, true, false),
        Status::Disabled { is_toggled } => (is_toggled, false, true),
    };
    let dark = theme.extended_palette().is_dark;
    let off_track = blend(p.surface, p.text, if hovered { 0.34 } else { 0.28 });
    style.background = if disabled {
        p.disabled.scale_alpha(0.4).into()
    } else if on {
        if hovered {
            blend(p.accent, p.text, 0.1).into()
        } else {
            p.accent.into()
        }
    } else {
        off_track.into()
    };
    style.background_border_width = 0.0;
    style.background_border_color = Color::TRANSPARENT;
    style.foreground = if on {
        p.on_accent.into()
    } else if dark {
        p.text.into()
    } else {
        Color::WHITE.into()
    };
    style.foreground_border_width = 0.0;
    style.foreground_border_color = Color::TRANSPARENT;
    style.padding_ratio = 0.12;
    style
}

/// A quiet scrollbar: no track, a slim rounded thumb in the text colour that
/// firms up under the pointer and while dragged.
pub fn scrollbar(
    theme: &Theme,
    status: iced::widget::scrollable::Status,
) -> iced::widget::scrollable::Style {
    use iced::widget::scrollable::{Rail, Scroller, Status};
    let mut style = iced::widget::scrollable::default(theme, status);
    let text = colors(theme).text;
    let rail = |alpha: f32| Rail {
        background: None,
        border: iced::Border::default(),
        scroller: Scroller {
            background: text.scale_alpha(alpha).into(),
            border: iced::border::rounded(u32::MAX),
        },
    };
    let (vertical, horizontal) = match status {
        Status::Active { .. } => (0.24, 0.24),
        Status::Hovered {
            is_horizontal_scrollbar_hovered,
            is_vertical_scrollbar_hovered,
            ..
        } => (
            if is_vertical_scrollbar_hovered {
                0.42
            } else {
                0.3
            },
            if is_horizontal_scrollbar_hovered {
                0.42
            } else {
                0.3
            },
        ),
        Status::Dragged {
            is_horizontal_scrollbar_dragged,
            is_vertical_scrollbar_dragged,
            ..
        } => (
            if is_vertical_scrollbar_dragged {
                0.52
            } else {
                0.3
            },
            if is_horizontal_scrollbar_dragged {
                0.52
            } else {
                0.3
            },
        ),
    };
    style.vertical_rail = rail(vertical);
    style.horizontal_rail = rail(horizontal);
    style.gap = None;
    style
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
    /// The language selection is process-wide; tests that change it take turns.
    pub(crate) static SELECTION: std::sync::Mutex<()> = std::sync::Mutex::new(());
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
            // A quiet control's hover shows on the surface menus are drawn on.
            let quiet = action_style(&theme, button::Status::Hovered, Action::Quiet, false);
            assert_ne!(quiet.background, Some(p.surface.into()));
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
