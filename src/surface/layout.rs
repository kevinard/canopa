use iced::widget::{column, container};
use iced::{Alignment, Element, Length, Padding};

/// Edge padding applied on all sides of the bar content area (px).
pub const BAR_PADDING: f32 = 4.0;

/// Gap between modules within a zone (px).
pub const MODULE_GAP: f32 = 4.0;

/// Render three zones (top / center / bottom) in a full-height column.
///
/// - **Top zone:** modules stack downward from the top edge.
/// - **Center zone:** fills remaining space (`Length::Fill`); modules centred within it.
/// - **Bottom zone:** modules stack upward from the bottom edge.
///
/// `BAR_PADDING` is applied as padding on all four edges of the outer container.
/// `MODULE_GAP` separates modules within each zone.
/// `clip(true)` prevents zones from overlapping on short outputs.
pub fn zone_layout<'a, Message: Clone + 'a>(
    top: Vec<Element<'a, Message>>,
    center: Vec<Element<'a, Message>>,
    bottom: Vec<Element<'a, Message>>,
) -> Element<'a, Message> {
    let top_col: Element<'a, Message> = column(top).spacing(MODULE_GAP).into();
    let center_fill: Element<'a, Message> = container(column(center).spacing(MODULE_GAP))
        .height(Length::Fill)
        .width(Length::Fill)
        .align_y(Alignment::Center)
        .into();
    let bottom_col: Element<'a, Message> = column(bottom).spacing(MODULE_GAP).into();

    container(
        column(vec![top_col, center_fill, bottom_col])
            .width(Length::Fill)
            .height(Length::Fill),
    )
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
