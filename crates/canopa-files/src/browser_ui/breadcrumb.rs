//! The Files location bar for both browser hosts.
use super::{Icon, ink, input_style, text_layout};
use canopa_theme::{Appearance, ColorRole, UiKind};
use canopa_ui::accessibility::{Node, semantic};
use iced::{
    Element, Length, Padding,
    font::Weight,
    widget::{button, container, responsive, text, tooltip},
};
use std::{os::unix::ffi::OsStrExt, path::Path};

/// The host supplies the navigation messages and its icon loader.
pub struct Location<'a, M> {
    pub appearance: &'a Appearance,
    pub path: &'a Path,
    pub home: &'a Path,
    pub home_label: String,
    pub key_prefix: &'static str,
    pub edit_description: String,
    pub edit: Option<M>,
    pub compact: bool,
    pub show_tooltips: bool,
}

pub fn view<'a, M: Clone + Send + 'static>(
    location: Location<'a, M>,
    icon: impl Fn(Icon, f32, f32) -> Element<'a, M> + 'a,
    navigate: impl Fn(&Path) -> Option<M> + 'a,
    trailing: Option<Element<'a, M>>,
) -> Element<'a, M> {
    let Location {
        appearance,
        path,
        home,
        home_label,
        key_prefix,
        edit_description,
        edit,
        compact,
        show_tooltips,
    } = location;
    let font = canopa_ui::font(appearance);
    let mut current_font = font;
    current_font.weight = Weight::Semibold;
    let size = appearance.typography().body_size.px();
    let within_home = path.starts_with(home);
    let label = |path: &Path| {
        if path == home {
            home_label.clone()
        } else {
            path.file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned()
        }
    };
    let current = label(path);
    let current_width = text_layout::text_width(&current, current_font, size);
    let ancestors: Vec<_> = path
        .ancestors()
        .skip(1)
        .take_while(|ancestor| !within_home || ancestor.starts_with(home))
        .map(|path| {
            let label = label(path);
            let width = text_layout::text_width(&label, font, size);
            (path, label, width)
        })
        .collect();
    let field_style = appearance.style(UiKind::Input);
    let mut padding = canopa_ui::content_padding(field_style);
    let inset = if compact { 3.0 } else { 6.0 };
    if compact && !field_style.custom_padding {
        padding.left -= 6.0;
        padding.right -= 6.0;
    }
    let crumbs = responsive(move |bounds| {
        let origin_width = if compact { 0.0 } else { 34.0 };
        let mut available = bounds.width - current_width - origin_width - 2.0 * inset;
        let count = ancestors
            .iter()
            .take_while(|(_, _, width)| {
                available -= width + 20.0;
                available >= 0.0
            })
            .count();
        let mut trail = Vec::new();
        // In a compact toolbar, keep a clickable parent ahead of the icon.
        if !compact || count == 0 {
            let mark = icon(
                if within_home {
                    Icon::Home
                } else {
                    Icon::Folder
                },
                18.0,
                0.87,
            );
            let hidden_home = within_home && count < ancestors.len();
            let origin = if let Some(message) = hidden_home.then(|| navigate(home)).flatten() {
                let home_button = crumb_button(mark, Some(message.clone())).padding([4, 2]);
                let control: Element<'_, M> = if show_tooltips {
                    tooltip(
                        home_button,
                        text(home_label.clone()).font(font).size(size),
                        tooltip::Position::Bottom,
                    )
                    .padding(6)
                    .style(text_layout::tooltip_style)
                    .into()
                } else {
                    home_button.into()
                };
                semantic(
                    Node::button(&home_label)
                        .key(format!("{key_prefix}-home"))
                        .on_click(Some(message)),
                    control,
                )
            } else {
                container(mark).padding([0, 2]).into()
            };
            trail.push(origin);
            if hidden_home && count != 0 {
                trail.push(icon(Icon::Forward, 12.0, 0.82));
            }
        }
        for (path, label, _) in ancestors[..count].iter().rev() {
            let message = navigate(path);
            let content = text(label.clone())
                .font(font)
                .size(size)
                .shaping(text::Shaping::Advanced)
                .color(ink(appearance, ColorRole::Muted))
                .wrapping(text::Wrapping::None);
            trail.push(semantic(
                Node::button(label)
                    .key(format!(
                        "{key_prefix}-{}",
                        path.as_os_str().as_bytes().escape_ascii()
                    ))
                    .disabled(message.is_none())
                    .on_click(message.clone()),
                crumb_button(content, message).padding([4, 4]),
            ));
            trail.push(icon(Icon::Forward, 12.0, 0.82));
        }
        let name = text_layout::single_line(current.clone(), current_font, size, None, false);
        trail.push(semantic(
            Node::button(&current)
                .key(format!("{key_prefix}-current"))
                .description(&edit_description)
                .disabled(edit.is_none())
                .on_click(edit.clone()),
            crumb_button(name, edit.clone())
                .padding([4.0, inset])
                .width(Length::Fill),
        ));
        canopa_ui::direction::row(trail)
            .align_y(iced::Center)
            .into()
    })
    .height(Length::Shrink);
    let controls = canopa_ui::direction::row([Some(crumbs.into()), trailing].into_iter().flatten())
        .spacing(2)
        .align_y(iced::Center);
    container(controls)
        .width(Length::Fill)
        .height((size * 1.3).ceil() + padding.top + padding.bottom)
        .align_y(iced::Center)
        .padding(Padding {
            top: 0.0,
            bottom: 0.0,
            ..padding
        })
        .clip(true)
        .style(move |theme| {
            let input = input_style(appearance, theme, iced::widget::text_input::Status::Active);
            container::Style::default()
                .background(input.background)
                .border(input.border)
        })
        .into()
}

pub fn crumb_button<'a, M: Clone + 'a>(
    content: impl Into<Element<'a, M>>,
    message: Option<M>,
) -> button::Button<'a, M> {
    iced::widget::button(content)
        .on_press_maybe(message)
        .style(|theme, status| {
            let mut style = canopa_ui::action_style(theme, status, canopa_ui::Action::Quiet, false);
            style.border.radius = 6.0.into();
            style
        })
}
