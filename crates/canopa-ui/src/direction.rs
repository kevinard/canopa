//! Right-to-left mirroring for Canopa's Iced layouts.
//!
//! Iced lays rows out left to right and shapes each text run with its own
//! bidi direction, but it does not mirror a layout. Views build rows, place
//! leading and trailing content, and pick chevrons and directional icons
//! through these helpers, which follow the selected language's direction
//! (`canopa_i18n::direction`; the pseudo-locale is right to left).
pub use canopa_i18n::Direction;
use iced::{
    Element, Padding,
    alignment::Horizontal,
    widget::{Row, text},
};
use std::borrow::Cow;

/// The writing direction of the current language.
pub fn direction() -> Direction {
    canopa_i18n::direction()
}

pub fn is_rtl() -> bool {
    direction().is_rtl()
}

/// `items` in visual order: reading order left to right, reversed for RTL.
pub fn ordered<T>(items: impl IntoIterator<Item = T>) -> Vec<T> {
    ordered_for(direction(), items)
}

pub fn ordered_for<T>(direction: Direction, items: impl IntoIterator<Item = T>) -> Vec<T> {
    let mut items: Vec<T> = items.into_iter().collect();
    if direction.is_rtl() {
        items.reverse();
    }
    items
}

/// A row whose children are given in reading order (leading first).
pub fn row<'a, Message: 'a>(
    children: impl IntoIterator<Item = Element<'a, Message>>,
) -> Row<'a, Message> {
    Row::with_children(ordered(children))
}

/// The leading edge: left for LTR, right for RTL.
pub fn start() -> Horizontal {
    start_for(direction())
}

/// The trailing edge: right for LTR, left for RTL.
pub fn end() -> Horizontal {
    end_for(direction())
}

pub fn start_for(direction: Direction) -> Horizontal {
    if direction.is_rtl() {
        Horizontal::Right
    } else {
        Horizontal::Left
    }
}

pub fn end_for(direction: Direction) -> Horizontal {
    if direction.is_rtl() {
        Horizontal::Left
    } else {
        Horizontal::Right
    }
}

/// Text alignment at the leading edge.
pub fn text_start() -> text::Alignment {
    if is_rtl() {
        text::Alignment::Right
    } else {
        text::Alignment::Left
    }
}

/// Text alignment at the trailing edge.
pub fn text_end() -> text::Alignment {
    if is_rtl() {
        text::Alignment::Left
    } else {
        text::Alignment::Right
    }
}

/// Padding given by leading and trailing edges instead of left and right.
pub fn padding(top: f32, end: f32, bottom: f32, start: f32) -> Padding {
    padding_for(direction(), top, end, bottom, start)
}

pub fn padding_for(direction: Direction, top: f32, end: f32, bottom: f32, start: f32) -> Padding {
    let (left, right) = if direction.is_rtl() {
        (end, start)
    } else {
        (start, end)
    };
    Padding {
        top,
        right,
        bottom,
        left,
    }
}

/// The chevron that points forward, into a page or submenu (`›`, `‹` in RTL).
pub fn forward_chevron() -> &'static str {
    if is_rtl() { "‹" } else { "›" }
}

/// The chevron that points back (`‹`, `›` in RTL).
pub fn back_chevron() -> &'static str {
    if is_rtl() { "›" } else { "‹" }
}

/// The arrow that points back (`←`, `→` in RTL).
pub fn back_arrow() -> &'static str {
    if is_rtl() { "→" } else { "←" }
}

/// A Back button caption with its chevron: `‹ Back`.
pub fn back_label() -> String {
    fl!("back-with-chevron", chevron = back_chevron())
}

/// The icon to draw for a directional freedesktop icon name: in RTL, `next`
/// and `previous`, `start` and `end`, and `-ltr`/`-rtl` variants swap.
pub fn icon(name: &str) -> Cow<'_, str> {
    icon_for(direction(), name)
}

pub fn icon_for(direction: Direction, name: &str) -> Cow<'_, str> {
    if !direction.is_rtl() {
        return Cow::Borrowed(name);
    }
    const PAIRS: [(&str, &str); 6] = [
        ("go-next", "go-previous"),
        ("go-first", "go-last"),
        ("pan-end", "pan-start"),
        ("media-seek-forward", "media-seek-backward"),
        ("media-skip-forward", "media-skip-backward"),
        ("-ltr", "-rtl"),
    ];
    for (a, b) in PAIRS {
        if let Some(swapped) = swap(name, a, b).or_else(|| swap(name, b, a)) {
            return Cow::Owned(swapped);
        }
    }
    Cow::Borrowed(name)
}

fn swap(name: &str, from: &str, to: &str) -> Option<String> {
    name.contains(from).then(|| name.replacen(from, to, 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_and_edges_mirror_for_rtl() {
        assert_eq!(ordered_for(Direction::Ltr, [1, 2, 3]), [1, 2, 3]);
        assert_eq!(ordered_for(Direction::Rtl, [1, 2, 3]), [3, 2, 1]);
        assert_eq!(start_for(Direction::Ltr), Horizontal::Left);
        assert_eq!(start_for(Direction::Rtl), Horizontal::Right);
        assert_eq!(end_for(Direction::Rtl), Horizontal::Left);
        let padding = padding_for(Direction::Rtl, 1., 2., 3., 4.);
        assert_eq!((padding.left, padding.right), (2., 4.));
        let padding = padding_for(Direction::Ltr, 1., 2., 3., 4.);
        assert_eq!((padding.left, padding.right), (4., 2.));
    }

    #[test]
    fn directional_icons_swap_for_rtl() {
        assert_eq!(icon_for(Direction::Ltr, "go-next-symbolic"), "go-next-symbolic");
        assert_eq!(icon_for(Direction::Rtl, "go-next-symbolic"), "go-previous-symbolic");
        assert_eq!(icon_for(Direction::Rtl, "go-previous-symbolic"), "go-next-symbolic");
        assert_eq!(icon_for(Direction::Rtl, "pan-end-symbolic"), "pan-start-symbolic");
        assert_eq!(icon_for(Direction::Rtl, "edit-undo-symbolic-ltr"), "edit-undo-symbolic-rtl");
        assert_eq!(icon_for(Direction::Rtl, "folder-symbolic"), "folder-symbolic");
    }

    #[test]
    fn the_pseudo_locale_mirrors_chevrons_and_labels() {
        let _guard = crate::tests::SELECTION.lock().unwrap_or_else(|p| p.into_inner());
        canopa_i18n::select(canopa_i18n::Selection::default());
        assert_eq!((forward_chevron(), back_chevron(), back_arrow()), ("›", "‹", "←"));
        assert_eq!(back_label(), "‹ Back");
        assert_eq!(start(), Horizontal::Left);
        canopa_i18n::select(canopa_i18n::Selection {
            pseudo: true,
            ..Default::default()
        });
        assert_eq!((forward_chevron(), back_chevron(), back_arrow()), ("‹", "›", "→"));
        assert_eq!(start(), Horizontal::Right);
        assert_eq!(text_start(), text::Alignment::Right);
        assert_eq!(icon("go-previous-symbolic"), "go-next-symbolic");
        let label = back_label();
        assert!(label.contains('›') && label.contains("Ɓáçķ"), "{label}");
        canopa_i18n::select(canopa_i18n::Selection::default());
    }

    #[test]
    fn catalog_is_complete() {
        canopa_i18n::check_catalog(&crate::i18n::LOADER).unwrap();
    }
}
