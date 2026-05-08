use iced::widget::{column, container, row};
use iced::{Alignment, Element, Length, Padding};

/// Edge padding applied on all sides of the bar content area (px).
pub const BAR_PADDING: f32 = 4.0;

/// Gap between modules within a zone (px).
pub const MODULE_GAP: f32 = 4.0;

/// Render three zones in a column (vertical bar) or row (horizontal bar).
///
/// **Vertical** (`horizontal = false`, left/right bars):
/// - Start zone stacks downward; center fills and centres; end stacks upward.
///
/// **Horizontal** (`horizontal = true`, top/bottom bars):
/// - Start zone aligns to the left; center fills and centres; end aligns to the right.
///
/// `BAR_PADDING` is applied as padding on all four edges of the outer container.
/// `MODULE_GAP` separates modules within each zone.
/// `clip(true)` prevents zones from overlapping on short outputs.
pub fn zone_layout<'a, Message: Clone + 'a>(
    start: Vec<Element<'a, Message>>,
    center: Vec<Element<'a, Message>>,
    end: Vec<Element<'a, Message>>,
    horizontal: bool,
) -> Element<'a, Message> {
    let zone = |items: Vec<Element<'a, Message>>| -> Element<'a, Message> {
        if horizontal {
            row(items).spacing(MODULE_GAP).into()
        } else {
            column(items).spacing(MODULE_GAP).into()
        }
    };

    let center_zone = zone(center);
    let center_fill: Element<'a, Message> = if horizontal {
        container(center_zone)
            .height(Length::Fill)
            .width(Length::Fill)
            .align_x(Alignment::Center)
            .into()
    } else {
        container(center_zone)
            .height(Length::Fill)
            .width(Length::Fill)
            .align_y(Alignment::Center)
            .into()
    };

    let parts = vec![zone(start), center_fill, zone(end)];
    let body: Element<'a, Message> = if horizontal {
        row(parts).width(Length::Fill).height(Length::Fill).into()
    } else {
        column(parts)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    };

    container(body)
        .padding(Padding::new(BAR_PADDING))
        .width(Length::Fill)
        .height(Length::Fill)
        .clip(true)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_padding_is_four() {
        assert_eq!(BAR_PADDING, 4.0);
    }

    #[test]
    fn module_gap_is_four() {
        assert_eq!(MODULE_GAP, 4.0);
    }
}
