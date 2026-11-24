//! Presentation shared by Files and its file picker.
pub mod breadcrumb;
pub mod entry;
pub mod file_icon;
mod icons;
pub mod text_layout;
pub use icons::Icon;

use canopa_theme::{Appearance, ColorRole, UiKind};
use iced::{
    Color, Theme,
    widget::{container, rule, text_input},
};

fn ink(appearance: &Appearance, role: ColorRole) -> Color {
    canopa_ui::color(appearance.role(role))
}

fn mix(a: Color, b: Color, amount: f32) -> Color {
    Color {
        r: a.r + (b.r - a.r) * amount,
        g: a.g + (b.g - a.g) * amount,
        b: a.b + (b.b - a.b) * amount,
        a: a.a,
    }
}

pub fn divider(theme: &Theme) -> rule::Style {
    let mut color = canopa_ui::colors(theme).border;
    color.a = 0.64;
    rule::Style {
        color,
        radius: 0.0.into(),
        fill_mode: rule::FillMode::Full,
        snap: true,
    }
}

/// The toolbar's and status bar's tint over the window's surface; the tab
/// strip takes it too.
pub fn bar_tint(appearance: &Appearance) -> Color {
    let mut tint = ink(appearance, ColorRole::Raised);
    tint.a = 0.56;
    tint
}

pub fn bar(appearance: &Appearance, top: bool) -> container::Style {
    let tint = bar_tint(appearance);
    let mut frame = window_surface(appearance, false);
    let radius = frame.border.radius;
    frame.border = iced::Border::default().rounded(if top {
        iced::border::Radius {
            bottom_left: 0.0,
            bottom_right: 0.0,
            ..radius
        }
    } else {
        iced::border::Radius {
            top_left: 0.0,
            top_right: 0.0,
            ..radius
        }
    });
    frame.background = Some(tint.into());
    frame
}

/// The surface that fills a Files window. The compositor owns the window's
/// shape, corner radius, outline and shadow, so the default surface runs edge
/// to edge instead of drawing rounded corners over an opaque rectangle. In
/// high contrast, floating dialogs keep their own full-strength outline.
/// Customized radii and borders still apply.
pub fn window_surface(appearance: &Appearance, floating: bool) -> container::Style {
    let style = appearance.style(UiKind::Panel);
    let mut frame = canopa_ui::frame(appearance, style);
    if !style.custom_radius {
        frame.border.radius = 0.0.into();
    }
    if !style.custom_border {
        frame.border.width = if floating && appearance.high_contrast() {
            1.0
        } else {
            0.0
        };
    }
    // Floating dialogs sit one step above the browser surface they cover.
    let custom_background = appearance.customization.ui[UiKind::Panel as usize]
        .background
        .is_some();
    if floating && !custom_background && !appearance.high_contrast() {
        let lift = match appearance.color_scheme {
            canopa_theme::ColorScheme::Dark => 0.55,
            canopa_theme::ColorScheme::Light => 0.0,
        };
        frame.background = Some(
            mix(
                ink(appearance, ColorRole::Surface),
                ink(appearance, ColorRole::Raised),
                lift,
            )
            .into(),
        );
    }
    frame
}

/// The current place in the sidebar: a neutral fill that stands out from the
/// sidebar's own tint in both appearances, leaving the accent to the selection.
pub fn sidebar_current(appearance: &Appearance) -> container::Style {
    let fill = match appearance.color_scheme {
        canopa_theme::ColorScheme::Dark => mix(
            ink(appearance, ColorRole::Raised),
            ink(appearance, ColorRole::Text),
            0.1,
        ),
        canopa_theme::ColorScheme::Light => mix(
            ink(appearance, ColorRole::Background),
            ink(appearance, ColorRole::Text),
            0.12,
        ),
    };
    container::Style::default()
        .background(fill)
        .border(iced::Border::default().rounded(8))
}

pub fn input_style(
    appearance: &canopa_theme::Appearance,
    theme: &Theme,
    status: text_input::Status,
) -> text_input::Style {
    let hovered = matches!(
        status,
        text_input::Status::Hovered | text_input::Status::Focused { is_hovered: true }
    );
    let focused = matches!(status, text_input::Status::Focused { .. });
    let style = appearance.style(UiKind::Input).state(
        false,
        hovered,
        false,
        status == text_input::Status::Disabled,
        focused,
    );
    let frame = canopa_ui::frame(appearance, style);
    let mut result = text_input::default(theme, status);
    if let Some(background) = frame.background {
        result.background = background;
    }
    if let Some(color) = frame.text_color {
        result.value = color;
    }
    // Selected text reads clearly against the field, as in a pre-selected name.
    result.selection = if appearance.high_contrast() {
        ink(appearance, ColorRole::AccentSurface)
    } else {
        // Light enough that the selected characters keep their contrast.
        ink(appearance, ColorRole::Accent).scale_alpha(
            if appearance.color_scheme == canopa_theme::ColorScheme::Dark {
                0.36
            } else {
                0.42
            },
        )
    };
    result.border = frame.border;
    if focused && style.states[4].border_color.is_none() {
        result.border.width = result.border.width.max(1.0);
        result.border.color = canopa_ui::color(appearance.role(ColorRole::Focus));
    }
    result
}
