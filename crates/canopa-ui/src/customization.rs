use canopa_theme::{Appearance, ColorValue, Insets, ResolvedStyle, UiKind};
use iced::{
    Color, Font, Padding,
    widget::{button, container},
};

pub fn resolve_color(appearance: &Appearance, value: ColorValue) -> Color {
    match value {
        ColorValue::Role(role) => crate::color(appearance.role(role)),
        ColorValue::Rgba(rgb, alpha) => {
            let mut c = crate::color(rgb);
            c.a = f32::from(alpha) / 255.0;
            c
        }
    }
}
pub fn padding(insets: Insets) -> Padding {
    Padding {
        top: insets.top.px(),
        right: insets.right.px(),
        bottom: insets.bottom.px(),
        left: insets.left.px(),
    }
}
/// Custom borders reserve space between their inner edge and content.
pub fn content_padding(style: ResolvedStyle) -> Padding {
    let mut value = padding(style.padding);
    if style.custom_border {
        let border = style.border_width.px();
        value.top += border;
        value.right += border;
        value.bottom += border;
        value.left += border;
    }
    value
}
pub fn frame(appearance: &Appearance, style: ResolvedStyle) -> container::Style {
    container::Style {
        background: Some(resolve_color(appearance, style.background).into()),
        text_color: Some(resolve_color(appearance, style.foreground)),
        border: iced::Border {
            color: resolve_color(appearance, style.border_color),
            width: style.border_width.px(),
            radius: iced::border::Radius {
                top_left: style.radius[0].px(),
                top_right: style.radius[1].px(),
                bottom_right: style.radius[2].px(),
                bottom_left: style.radius[3].px(),
            },
        },
        ..Default::default()
    }
}

/// Resolve installed families once. Only names from the finite system font database
/// are interned for Iced's static font API; configuration strings are never leaked.
pub fn font(appearance: &Appearance) -> Font {
    use iced::font::Family;
    static FAMILIES: std::sync::LazyLock<Vec<&'static str>> = std::sync::LazyLock::new(|| {
        let mut system = iced::advanced::graphics::text::font_system()
            .write()
            .expect("font system");
        let mut names = std::collections::BTreeSet::new();
        for face in system.raw().db().faces() {
            for (name, _) in &face.families {
                names.insert(name.clone());
            }
        }
        names
            .into_iter()
            .map(|n| &*Box::leak(n.into_boxed_str()))
            .collect()
    });
    for name in &appearance.typography().family {
        let family = match name.as_str() {
            "sans-serif" => Family::SansSerif,
            "serif" => Family::Serif,
            "monospace" => Family::Monospace,
            "cursive" => Family::Cursive,
            "fantasy" => Family::Fantasy,
            _ => match FAMILIES
                .iter()
                .find(|candidate| candidate.eq_ignore_ascii_case(name))
            {
                Some(name) => Family::Name(name),
                None => continue,
            },
        };
        return Font {
            family,
            ..Font::DEFAULT
        };
    }
    Font::DEFAULT
}

pub fn styled_action<'a, Message: Clone + 'a>(
    content: impl Into<iced::Element<'a, Message>>,
    on_press: Option<Message>,
    treatment: crate::Action,
    focused: bool,
    appearance: &Appearance,
) -> button::Button<'a, Message> {
    let appearance = appearance.clone();
    let style = appearance.style(UiKind::Button);
    button(content)
        .padding(content_padding(style))
        .on_press_maybe(on_press)
        .style(move |theme, status| {
            let mut base = crate::action_style(theme, status, treatment, focused);
            let selected = style.state(
                false,
                status == button::Status::Hovered,
                status == button::Status::Pressed,
                status == button::Status::Disabled,
                focused,
            );
            let overrides = appearance.customization.ui[UiKind::Button as usize];
            let frame = frame(&appearance, selected);
            if overrides.background.is_some() {
                base.background = frame.background;
            }
            if overrides.foreground.is_some() {
                base.text_color = frame.text_color.unwrap();
            }
            base.border.radius = frame.border.radius;
            if style.custom_border {
                base.border.width = frame.border.width;
                base.border.color = frame.border.color;
            }
            if focused && status != button::Status::Disabled {
                base.border.width = base.border.width.max(1.0);
                base.border.color = crate::color(appearance.role(canopa_theme::ColorRole::Focus));
            }
            for (enabled, s) in [
                false,
                status == button::Status::Hovered,
                status == button::Status::Pressed,
                status == button::Status::Disabled,
                focused && status != button::Status::Disabled,
            ]
            .into_iter()
            .zip(style.states)
            {
                if enabled {
                    if let Some(c) = s.background {
                        base.background = Some(resolve_color(&appearance, c).into());
                    }
                    if let Some(c) = s.foreground {
                        base.text_color = resolve_color(&appearance, c);
                    }
                    if let Some(c) = s.border_color {
                        base.border.color = resolve_color(&appearance, c);
                    }
                }
            }
            base
        })
}
