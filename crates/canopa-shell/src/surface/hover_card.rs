//! Hover cards: interactive tooltips. A taskbar application's card lists its
//! windows with thumbnails; a workspace's card draws its columns to scale.
//! Both are drawn in the tooltip popup, from data the app gathers; clicking a
//! window focuses it. Window management stays in Niri's titlebars.
use iced::widget::{Space, button, column, container, image, row, text};
use iced::{Alignment, Element, Length};

use crate::focused_window_icons::ApplicationIcon;
use crate::modules::minimap::{MinimapState, MinimapTile, StripMetrics, TilePlace, draw_strip};

/// What a hover card asks of the app.
#[derive(Debug, Clone)]
pub enum Event {
    /// Focus a window, switching workspace and output as needed.
    FocusWindow(u64),
    /// Ask a window to close.
    CloseWindow(u64),
    /// Window thumbnails arrived from Niri: window ID, width, height, PNG.
    Previews(Vec<(u64, u32, u32, Vec<u8>)>),
    /// The module's bounds on its bar were measured: open its card there.
    Open {
        window: iced::window::Id,
        module: crate::modules::ModuleId,
        bounds: Option<iced::Rectangle>,
    },
}

/// A window image from Niri's preview service, at its pixel size.
#[derive(Debug, Clone)]
pub struct Thumbnail {
    pub handle: iced::widget::image::Handle,
    pub width: u32,
    pub height: u32,
}

/// How an application is drawn when there is no thumbnail.
#[derive(Debug, Clone, Default)]
pub struct AppFace {
    pub key: String,
    pub name: String,
    pub icon: Option<ApplicationIcon>,
}

#[derive(Debug, Clone)]
pub enum HoverCard {
    Windows(WindowList),
    Workspace(WorkspaceCard),
}

/// A taskbar application's windows, in screen order.
#[derive(Debug, Clone)]
pub struct WindowList {
    pub app: AppFace,
    /// The application's whole window count, shown in the header.
    pub count: usize,
    pub rows: Vec<WindowRow>,
}

#[derive(Debug, Clone)]
pub struct WindowRow {
    pub id: u64,
    pub title: String,
    /// Where the window is, such as its workspace.
    pub detail: Option<String>,
    pub focused: bool,
    pub urgent: bool,
    pub thumbnail: Option<Thumbnail>,
}

/// One workspace's columns, drawn like the minimap with window faces.
#[derive(Debug, Clone)]
pub struct WorkspaceCard {
    pub title: String,
    pub strip: MinimapState,
    pub faces: Vec<TileFace>,
}

#[derive(Debug, Clone)]
pub struct TileFace {
    pub id: u64,
    pub app: AppFace,
    pub thumbnail: Option<Thumbnail>,
}

/// At most this many windows are listed; the rest are counted.
pub const MAX_ROWS: usize = 8;
/// Card content width of a window list, in logical pixels.
const LIST_WIDTH: f32 = 340.0;
const PADDING: f32 = 8.0;
const HEADER_HEIGHT: f32 = 28.0;
const SECTION_GAP: f32 = 6.0;
const ROW_HEIGHT: f32 = 72.0;
const ROW_GAP: f32 = 2.0;
const ROW_PADDING: f32 = 6.0;
const THUMB_WIDTH: f32 = 108.0;
const THUMB_HEIGHT: f32 = 60.0;
const CLOSE_SIZE: f32 = 26.0;
const FOOTER_HEIGHT: f32 = 18.0;
const HEADER_ICON: f32 = 18.0;
/// The workspace strip's height, and the longest it may grow.
const STRIP_SIZE: f32 = 124.0;
const STRIP_MAX_LENGTH: f32 = 460.0;
const WORKSPACE_MIN_WIDTH: f32 = 220.0;

/// The workspace card's strip: the minimap's layout with room for faces.
pub(crate) const CARD_STRIP: StripMetrics = StripMetrics {
    column_gap: 6.0,
    pin_gap: 14.0,
    tile_gap: 4.0,
    min_extent: 12.0,
    frame_inset: 3.0,
    radius: 6.0,
};

impl HoverCard {
    /// The content size inside the card's padding and border.
    pub fn content_size(&self) -> (f32, f32) {
        match self {
            HoverCard::Windows(list) => {
                let rows = list.rows.len() as f32;
                let mut height = HEADER_HEIGHT;
                if rows > 0.0 {
                    height += SECTION_GAP + rows * ROW_HEIGHT + (rows - 1.0) * ROW_GAP;
                }
                if list.count > list.rows.len() {
                    height += SECTION_GAP + FOOTER_HEIGHT;
                }
                (LIST_WIDTH, height)
            }
            HoverCard::Workspace(card) => {
                let mut height = HEADER_HEIGHT;
                if card.strip.columns.is_empty() {
                    return (WORKSPACE_MIN_WIDTH, height + FOOTER_HEIGHT);
                }
                let lengths = CARD_STRIP.column_lengths(&card.strip, STRIP_SIZE, STRIP_MAX_LENGTH);
                let width =
                    CARD_STRIP.strip_length(&card.strip, &lengths) + 2.0 * CARD_STRIP.frame_inset;
                height += SECTION_GAP + STRIP_SIZE + 2.0 * CARD_STRIP.frame_inset;
                if card.strip.floating > 0 {
                    height += SECTION_GAP + FOOTER_HEIGHT;
                }
                (width.max(WORKSPACE_MIN_WIDTH), height)
            }
        }
    }

    /// The card's padding on every side.
    pub fn padding() -> f32 {
        PADDING
    }

    /// Windows whose thumbnails the card would show.
    pub fn windows(&self) -> Vec<u64> {
        match self {
            HoverCard::Windows(list) => list.rows.iter().map(|row| row.id).collect(),
            HoverCard::Workspace(card) => card.faces.iter().map(|face| face.id).collect(),
        }
    }

    pub fn view<'a>(&'a self) -> Element<'a, crate::app::Message> {
        match self {
            HoverCard::Windows(list) => window_list(list),
            HoverCard::Workspace(card) => workspace(card),
        }
    }
}

fn message(event: Event) -> crate::app::Message {
    crate::app::Message::HoverCard(event)
}

fn muted() -> iced::Color {
    let colors = crate::theme::palette();
    iced::Color {
        a: 0.68,
        ..colors.text
    }
}

fn header<'a>(
    leading: Option<Element<'a, crate::app::Message>>,
    title: &str,
    trailing: Option<String>,
) -> Element<'a, crate::app::Message> {
    let size = crate::theme::body_size();
    let trailing_width = trailing.as_deref().map_or(0.0, |value| {
        crate::theme::text_width(value, crate::theme::label_size()) + 12.0
    });
    let leading_width = if leading.is_some() {
        HEADER_ICON + 8.0
    } else {
        0.0
    };
    let title = crate::theme::ellipsize(
        title,
        (LIST_WIDTH - trailing_width - leading_width).max(40.0),
        size,
    );
    let mut items: Vec<Element<'a, crate::app::Message>> = Vec::new();
    if let Some(leading) = leading {
        items.push(leading);
    }
    items.push(
        crate::theme::strong_text(title)
            .wrapping(text::Wrapping::None)
            .into(),
    );
    items.push(Space::new().width(Length::Fill).into());
    if let Some(trailing) = trailing {
        items.push(
            text(trailing)
                .font(crate::theme::font())
                .size(crate::theme::label_size())
                .color(muted())
                .into(),
        );
    }
    container(
        row(items)
            .spacing(8)
            .align_y(Alignment::Center)
            .padding([0, 4]),
    )
    .height(HEADER_HEIGHT)
    .align_y(Alignment::Center)
    .into()
}

fn window_list(list: &WindowList) -> Element<'_, crate::app::Message> {
    let mut content = column![header(
        Some(crate::modules::taskbar::application_icon(
            list.app.icon.as_ref(),
            &list.app.key,
            &list.app.name,
            HEADER_ICON,
        )),
        &list.app.name,
        Some(fl!("shell-taskbar-windows", count = list.count)),
    )]
    .spacing(SECTION_GAP)
    .width(LIST_WIDTH);
    if !list.rows.is_empty() {
        content = content.push(
            column(list.rows.iter().map(|row| window_row(row, &list.app)))
                .spacing(ROW_GAP)
                .width(Length::Fill),
        );
    }
    if list.count > list.rows.len() {
        content = content.push(
            container(
                text(fl!(
                    "shell-hover-card-more-windows",
                    count = list.count.saturating_sub(list.rows.len())
                ))
                .font(crate::theme::font())
                .size(crate::theme::label_size())
                .color(muted()),
            )
            .height(FOOTER_HEIGHT)
            .padding([0, 4])
            .align_y(Alignment::Center),
        );
    }
    content.into()
}

/// How a thumbnail fills its frame: whole and slightly cropped when the
/// shapes are close; a tall window shows its top, where titles and toolbars
/// are; a very wide one is shown whole on the plate.
fn fit(
    thumbnail: &Thumbnail,
    width: f32,
    height: f32,
) -> (iced::ContentFit, Option<iced::Rectangle<u32>>) {
    let frame = width / height.max(1.0);
    let window = thumbnail.width as f32 / thumbnail.height.max(1) as f32;
    let ratio = window / frame;
    if (ratio - 1.0).abs() <= 0.3 {
        (iced::ContentFit::Cover, None)
    } else if ratio < 1.0 {
        let visible = ((thumbnail.width as f32 / frame).round() as u32).clamp(1, thumbnail.height);
        (
            iced::ContentFit::Cover,
            Some(iced::Rectangle {
                x: 0,
                y: 0,
                width: thumbnail.width,
                height: visible,
            }),
        )
    } else {
        (iced::ContentFit::Contain, None)
    }
}

/// A window thumbnail fitted to `width` × `height`, or the application's icon
/// on a quiet plate while there is none.
fn face<'a>(
    thumbnail: Option<&Thumbnail>,
    app: &AppFace,
    width: f32,
    height: f32,
    radius: f32,
    dim: bool,
) -> Element<'a, crate::app::Message> {
    let colors = crate::theme::palette();
    let opacity = if dim { 0.55 } else { 1.0 };
    let plate = move |_: &iced::Theme| container::Style {
        background: Some(
            iced::Color {
                a: 0.07,
                ..colors.text
            }
            .into(),
        ),
        border: iced::Border {
            radius: radius.into(),
            width: 1.0,
            color: iced::Color {
                a: 0.08,
                ..colors.text
            },
        },
        ..Default::default()
    };
    match thumbnail {
        Some(thumbnail) => {
            let (content_fit, crop) = fit(thumbnail, width, height);
            let mut picture = image(thumbnail.handle.clone())
                .width(width)
                .height(height)
                .content_fit(content_fit)
                .border_radius(radius)
                .opacity(opacity);
            if let Some(crop) = crop {
                picture = picture.crop(crop);
            }
            container(picture)
                .width(width)
                .height(height)
                .style(plate)
                .into()
        }
        None => {
            let icon = (width.min(height) * 0.42).clamp(12.0, 40.0);
            let mut inner = column![crate::modules::taskbar::application_icon(
                app.icon.as_ref(),
                &app.key,
                &app.name,
                icon,
            )]
            .spacing(4)
            .align_x(Alignment::Center);
            let size = crate::theme::label_size();
            if height >= icon + size * 1.6 + 12.0 && width >= 64.0 {
                inner = inner.push(
                    text(crate::theme::ellipsize(&app.name, width - 12.0, size))
                        .font(crate::theme::font())
                        .size(size)
                        .color(muted())
                        .wrapping(text::Wrapping::None),
                );
            }
            container(inner)
                .center_x(width)
                .center_y(height)
                .style(plate)
                .into()
        }
    }
}

fn window_row<'a>(row_data: &'a WindowRow, app: &AppFace) -> Element<'a, crate::app::Message> {
    let colors = crate::theme::palette();
    let body = crate::theme::body_size();
    let label = crate::theme::label_size();
    let text_width = LIST_WIDTH - 2.0 * ROW_PADDING - THUMB_WIDTH - CLOSE_SIZE - 2.0 * 10.0;
    let title_color = if row_data.urgent {
        colors.warning
    } else {
        colors.text
    };
    let mut lines = column![
        text(crate::theme::ellipsize(&row_data.title, text_width, body))
            .font(if row_data.focused {
                iced::Font {
                    weight: iced::font::Weight::Semibold,
                    ..crate::theme::font()
                }
            } else {
                crate::theme::font()
            })
            .size(body)
            .color(title_color)
            .wrapping(text::Wrapping::None)
    ]
    .spacing(2)
    .width(text_width);
    if let Some(detail) = &row_data.detail {
        lines = lines.push(
            text(crate::theme::ellipsize(detail, text_width, label))
                .font(crate::theme::font())
                .size(label)
                .color(muted())
                .wrapping(text::Wrapping::None),
        );
    }
    let close = button(
        container(crate::theme::icon(
            include_bytes!("../../assets/close.svg"),
            16.0,
            muted(),
        ))
        .center_x(CLOSE_SIZE)
        .center_y(CLOSE_SIZE),
    )
    .padding(0)
    .width(CLOSE_SIZE)
    .height(CLOSE_SIZE)
    .on_press(message(Event::CloseWindow(row_data.id)))
    .style(move |_, status| button::Style {
        background: match status {
            button::Status::Hovered => Some(colors.text.scale_alpha(0.12).into()),
            button::Status::Pressed => Some(colors.text.scale_alpha(0.2).into()),
            _ => None,
        },
        border: iced::border::rounded(CLOSE_SIZE / 2.0),
        ..Default::default()
    });
    let content = row![
        face(
            row_data.thumbnail.as_ref(),
            app,
            THUMB_WIDTH,
            THUMB_HEIGHT,
            6.0,
            false
        ),
        lines,
        Space::new().width(Length::Fill),
        close,
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    let focused = row_data.focused;
    button(content)
        .padding(ROW_PADDING)
        .width(Length::Fill)
        .height(ROW_HEIGHT)
        .on_press(message(Event::FocusWindow(row_data.id)))
        .style(move |_, status| {
            let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
            let background = if focused {
                Some(crate::theme::accent_surface().into())
            } else if hovered {
                Some(colors.text.scale_alpha(0.07).into())
            } else {
                None
            };
            button::Style {
                background,
                text_color: colors.text,
                border: iced::Border {
                    radius: 10.0.into(),
                    width: if focused && hovered { 1.0 } else { 0.0 },
                    color: colors.accent.scale_alpha(0.5),
                },
                ..Default::default()
            }
        })
        .into()
}

fn workspace(card: &WorkspaceCard) -> Element<'_, crate::app::Message> {
    let colors = crate::theme::palette();
    let windows = card
        .strip
        .columns
        .iter()
        .map(|column| column.tiles.len())
        .sum::<usize>()
        + card.strip.floating;
    let trailing = (windows > 0).then(|| fl!("shell-taskbar-windows", count = windows));
    let mut content = column![header(None, &card.title, trailing)].spacing(SECTION_GAP);
    if card.strip.columns.is_empty() {
        let note = if card.strip.floating > 0 {
            fl!("shell-a11y-minimap-floating", count = card.strip.floating)
        } else {
            fl!("shell-workspace-empty")
        };
        return content
            .push(
                container(
                    text(note)
                        .font(crate::theme::font())
                        .size(crate::theme::label_size())
                        .color(muted()),
                )
                .height(FOOTER_HEIGHT)
                .padding([0, 4]),
            )
            .width(WORKSPACE_MIN_WIDTH)
            .into();
    }
    let frame = colors.text.scale_alpha(0.45);
    let strip = draw_strip(
        &card.strip,
        CARD_STRIP,
        STRIP_SIZE,
        STRIP_MAX_LENGTH,
        true,
        frame,
        |tile, place| workspace_tile(tile, place, card.faces.iter().find(|f| f.id == tile.id)),
    );
    content = content.push(container(strip).center_x(Length::Fill));
    if card.strip.floating > 0 {
        content = content.push(
            container(
                text(fl!(
                    "shell-a11y-minimap-floating",
                    count = card.strip.floating
                ))
                .font(crate::theme::font())
                .size(crate::theme::label_size())
                .color(muted()),
            )
            .height(FOOTER_HEIGHT)
            .padding([0, 4])
            .align_y(Alignment::Center),
        );
    }
    content.into()
}

fn workspace_tile<'a>(
    tile: &MinimapTile,
    place: TilePlace,
    face_data: Option<&'a TileFace>,
) -> Element<'a, crate::app::Message> {
    let colors = crate::theme::palette();
    let (width, height) = (place.size.width, place.size.height);
    let fallback = AppFace {
        key: String::new(),
        name: tile.title.clone().unwrap_or_default(),
        icon: None,
    };
    let app = face_data.map_or(&fallback, |face| &face.app);
    let inner = face(
        face_data.and_then(|face| face.thumbnail.as_ref()),
        app,
        width,
        height,
        CARD_STRIP.radius,
        !place.on_screen,
    );
    let (focused, urgent) = (tile.focused, tile.urgent);
    // The ring is a layer over the face, so a thumbnail never hides it and
    // the tile keeps its size.
    let ring = |hovered: bool| {
        let (width, color) = if urgent {
            (2.0, colors.warning)
        } else if focused {
            (2.0, colors.accent)
        } else if hovered {
            (2.0, colors.text.scale_alpha(0.55))
        } else {
            (1.0, colors.text.scale_alpha(0.08))
        };
        container(Space::new().width(Length::Fill).height(Length::Fill)).style(move |_| {
            container::Style {
                border: iced::Border {
                    radius: CARD_STRIP.radius.into(),
                    width,
                    color,
                },
                ..Default::default()
            }
        })
    };
    let face = iced::widget::hover(iced::widget::stack![inner, ring(false)], ring(true));
    button(face)
        .padding(0)
        .width(width)
        .height(height)
        .on_press(message(Event::FocusWindow(tile.id)))
        .style(move |_, _| button::Style {
            background: None,
            text_color: colors.text,
            ..Default::default()
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::minimap::{MinimapColumn, MinimapTile};

    fn row(id: u64) -> WindowRow {
        WindowRow {
            id,
            title: format!("Window {id}"),
            detail: None,
            focused: false,
            urgent: false,
            thumbnail: None,
        }
    }

    #[test]
    fn thumbnails_fill_similar_frames_and_fit_inside_others() {
        let thumbnail = |width, height| Thumbnail {
            handle: iced::widget::image::Handle::from_bytes(vec![0]),
            width,
            height,
        };
        assert_eq!(
            fit(&thumbnail(320, 180), 108.0, 60.0),
            (iced::ContentFit::Cover, None)
        );
        // A tall window shows its top at the frame's shape.
        let (content_fit, crop) = fit(&thumbnail(180, 320), 108.0, 60.0);
        assert_eq!(content_fit, iced::ContentFit::Cover);
        assert_eq!(
            crop,
            Some(iced::Rectangle {
                x: 0,
                y: 0,
                width: 180,
                height: 100
            })
        );
        assert_eq!(
            fit(&thumbnail(320, 60), 108.0, 60.0),
            (iced::ContentFit::Contain, None)
        );
    }

    #[test]
    fn window_lists_grow_by_row_and_count_the_rest() {
        let list = |rows: usize, count: usize| {
            HoverCard::Windows(WindowList {
                app: AppFace::default(),
                count,
                rows: (0..rows as u64).map(row).collect(),
            })
        };
        let (width, one) = list(1, 1).content_size();
        let (_, two) = list(2, 2).content_size();
        assert_eq!(width, LIST_WIDTH);
        assert_eq!(two - one, ROW_HEIGHT + ROW_GAP);
        let (_, more) = list(2, 5).content_size();
        assert_eq!(more - two, SECTION_GAP + FOOTER_HEIGHT);
        assert_eq!(list(3, 3).windows(), vec![0, 1, 2]);
    }

    #[test]
    fn workspace_cards_fit_their_strip_and_say_when_empty() {
        let column = |width: f64| MinimapColumn {
            width,
            tiles: vec![MinimapTile {
                id: width as u64,
                height: 1000.0,
                ..Default::default()
            }],
            ..Default::default()
        };
        let card = |columns: Vec<MinimapColumn>, floating| {
            HoverCard::Workspace(WorkspaceCard {
                title: "Workspace 1".into(),
                strip: MinimapState {
                    columns,
                    workspace_height: 1000.0,
                    floating,
                    view_width: None,
                },
                faces: Vec::new(),
            })
        };
        let empty = card(Vec::new(), 0).content_size();
        assert_eq!(empty, (WORKSPACE_MIN_WIDTH, HEADER_HEIGHT + FOOTER_HEIGHT));

        // Two columns at the workspace's full width and half of it.
        let (width, height) = card(vec![column(1000.0), column(2000.0)], 0).content_size();
        let strip = STRIP_SIZE * 3.0 + CARD_STRIP.column_gap + 2.0 * CARD_STRIP.frame_inset;
        assert_eq!(width, strip);
        assert_eq!(
            height,
            HEADER_HEIGHT + SECTION_GAP + STRIP_SIZE + 2.0 * CARD_STRIP.frame_inset
        );
        // Wide workspaces shrink to the card's limit; floating windows add a line.
        let (width, floating) = card(vec![column(9000.0), column(9000.0)], 2).content_size();
        assert!(width <= STRIP_MAX_LENGTH + 2.0 * CARD_STRIP.frame_inset + 0.01);
        assert_eq!(floating - height, SECTION_GAP + FOOTER_HEIGHT);
    }
}
