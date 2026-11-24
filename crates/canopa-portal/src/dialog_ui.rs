//! Common window chrome, controls, and typography for native portal dialogs.
use canopa_ui::Action;
use iced::{
    Element, Length, Theme,
    widget::{button, column, container, rule, scrollable, space, text, text_input},
};

pub(crate) fn heading<'a>(label: impl Into<String>) -> text::Text<'a> {
    text(label.into()).size(22)
}

pub(crate) fn section<'a>(label: impl Into<String>) -> text::Text<'a> {
    text(label.into()).size(14)
}

pub(crate) fn detail<'a>(label: impl Into<String>) -> text::Text<'a> {
    text(label.into()).size(14).style(canopa_ui::muted)
}

pub(crate) fn surface(theme: &Theme) -> container::Style {
    let colors = canopa_ui::colors(theme);
    container::Style::default()
        .background(if theme.extended_palette().is_dark {
            colors.background
        } else {
            colors.surface
        })
        .color(colors.text)
}

pub(crate) fn card(theme: &Theme) -> container::Style {
    let colors = canopa_ui::colors(theme);
    container::Style::default()
        .background(if theme.extended_palette().is_dark {
            colors.surface
        } else {
            colors.raised
        })
        .border(
            iced::Border::default()
                .rounded(10)
                .width(1)
                .color(colors.border.scale_alpha(0.55)),
        )
}

pub(crate) fn field(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let mut style = text_input::default(theme, status);
    let colors = canopa_ui::colors(theme);
    style.background = colors.background.into();
    style.border.radius = 8.0.into();
    style.border.color = match status {
        text_input::Status::Focused { .. } => colors.focus,
        text_input::Status::Hovered => colors.border,
        _ => colors.border.scale_alpha(0.65),
    };
    style.placeholder = colors.muted;
    if status == text_input::Status::Disabled {
        style.background = colors.surface.into();
        style.value = colors.muted;
    }
    style
}

pub(crate) fn action<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
    on_press: Option<Message>,
    treatment: Action,
    focused: bool,
) -> button::Button<'a, Message> {
    button(content)
        .on_press_maybe(on_press)
        .padding([8, 15])
        .style(move |theme, status| {
            let mut style = canopa_ui::action_style(theme, status, treatment, focused);
            if matches!(treatment, Action::Primary) && status == button::Status::Disabled {
                let colors = canopa_ui::colors(theme);
                style.background = Some(colors.text.scale_alpha(0.07).into());
            }
            style
        })
}

pub(crate) fn danger<'a, Message: Clone + 'a>(
    label: String,
    message: Message,
) -> button::Button<'a, Message> {
    button(text(label))
        .on_press(message)
        .padding([8, 15])
        .style(|theme, status| {
            let mut style = canopa_ui::action_style(theme, status, Action::Normal, false);
            let colors = canopa_ui::colors(theme);
            style.text_color = colors.danger;
            style.border.color = colors.danger.scale_alpha(0.7);
            style.background = Some(
                colors
                    .danger
                    .scale_alpha(if status == button::Status::Active {
                        0.12
                    } else {
                        0.22
                    })
                    .into(),
            );
            style
        })
}

/// Adjacent actions select views, with the current view indicated by its edge and fill.
pub(crate) fn tab<'a, Message: Clone + 'a>(
    label: String,
    selected: bool,
    message: Option<Message>,
) -> Element<'a, Message> {
    button(
        text(label)
            .size(15)
            .width(Length::Fill)
            .align_x(iced::Center),
    )
    .padding([9, 12])
    .width(Length::Fill)
    .on_press_maybe(message)
    .style(move |theme, status| {
        let mut style = selection(theme, status, selected, false);
        let colors = canopa_ui::colors(theme);
        if selected {
            style.border.width = 1.5;
            style.border.color = colors.accent;
        } else if status == button::Status::Active {
            style.background = Some(colors.surface.into());
            style.border.width = 1.0;
            style.border.color = colors.border.scale_alpha(0.55);
        }
        style
    })
    .into()
}

pub(crate) fn footer<'a, Message: 'a>(
    actions: impl IntoIterator<Item = Element<'a, Message>>,
) -> Element<'a, Message> {
    canopa_ui::direction::row(std::iter::once(space().width(Length::Fill).into()).chain(actions))
        .align_y(iced::Center)
        .spacing(8)
        .width(Length::Fill)
        .into()
}

/// Full-width footer stays in reach when request text or options need to scroll.
pub(crate) fn layout<'a, Message: 'a>(
    body: impl Into<Element<'a, Message>>,
    footer: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    frame(
        scrollable(container(body).padding([20, 24]).width(Length::Fill))
            .style(canopa_ui::scrollbar)
            .height(Length::Fill)
            .width(Length::Fill),
        footer,
    )
}

pub(crate) fn frame<'a, Message: 'a>(
    body: impl Into<Element<'a, Message>>,
    footer: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    container(column![
        body.into(),
        rule::horizontal(1).style(canopa_files::browser_ui::divider),
        container(footer).padding([12, 24]).width(Length::Fill),
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .style(surface)
    .into()
}

pub(crate) fn selection(
    theme: &Theme,
    status: button::Status,
    selected: bool,
    focused: bool,
) -> button::Style {
    let mut style = canopa_ui::action_style(theme, status, Action::Quiet, focused);
    style.border.radius = 7.0.into();
    if !selected && matches!(status, button::Status::Hovered | button::Status::Pressed) {
        style.background = None;
        style.border.width = 1.0;
        style.border.color = canopa_ui::colors(theme).border.scale_alpha(0.52);
    }
    if selected {
        let colors = canopa_ui::colors(theme);
        style.background = Some(colors.accent.scale_alpha(0.18).into());
        style.text_color = colors.text;
        style.border.color = colors.accent.scale_alpha(0.56);
        style.border.width = 1.0;
    }
    style
}

pub(crate) fn symbol(
    icon: canopa_files::browser_ui::Icon,
    canopa_theme::Rgb(r, g, b): canopa_theme::Rgb,
) -> Option<iced::widget::image::Handle> {
    let raster = canopa_assets::svg::rasterize(
        icon.svg().as_bytes(),
        canopa_assets::svg::Policy {
            max_encoded_bytes: 4096,
            max_dimension: 48,
            max_pixels: 48 * 48,
            output_side: 48,
            foreground: [r, g, b],
        },
    )?;
    Some(iced::widget::image::Handle::from_rgba(
        raster.width,
        raster.height,
        raster.pixels,
    ))
}

/// The desktop entry preserves the caller's real name; unknown IDs stay visible.
pub(crate) fn requester(app_id: &str) -> String {
    if app_id.is_empty() {
        return fl!("portal-requesting-app");
    }
    canopa_apps::desktop_entry_name(app_id)
        .unwrap_or_else(|| app_id.to_owned())
        .chars()
        .filter(|ch| !ch.is_control())
        .take(120)
        .collect()
}
