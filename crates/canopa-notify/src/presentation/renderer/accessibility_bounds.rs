//! Read the actual Iced layout, including nested scroll translations.
use super::BannerTarget;
use iced::advanced::widget::{Operation, operation};
use iced::{Rectangle, Vector};
use std::collections::BTreeMap;

pub(super) type Bounds = BTreeMap<(BannerTarget, usize), Rectangle>;

// Slots 0..4 are structural; named actions occupy 4..68 (input limit 64).
pub(super) fn link_slot(segment_index: usize) -> usize {
    256 + segment_index
}

pub(super) fn widget_id(target: BannerTarget, slot: usize) -> iced::widget::Id {
    iced::widget::Id::from(format!("canopa-banner/{target:?}/{slot}"))
}

pub(super) struct Measure {
    targets: Vec<(iced::widget::Id, (BannerTarget, usize))>,
    previous: Bounds,
    pub(super) bounds: Bounds,
    offset: Vector,
    clip: Option<Rectangle>,
    pending_scroll: Option<(Vector, Rectangle)>,
}

impl Measure {
    pub(super) fn new(
        targets: impl Iterator<Item = (BannerTarget, usize)>,
        previous: Bounds,
    ) -> Self {
        Self {
            targets: targets.map(|key| (widget_id(key.0, key.1), key)).collect(),
            previous,
            bounds: Bounds::new(),
            offset: Vector::ZERO,
            clip: None,
            pending_scroll: None,
        }
    }
}

impl Operation<Bounds> for Measure {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<Bounds>)) {
        let old = (self.offset, self.clip);
        if let Some((translation, clip)) = self.pending_scroll.take() {
            self.offset += translation;
            self.clip = Some(match self.clip {
                Some(previous) => previous.intersection(&clip).unwrap_or_default(),
                None => clip,
            });
        }
        operate(self);
        (self.offset, self.clip) = old;
    }

    fn scrollable(
        &mut self,
        _: Option<&iced::widget::Id>,
        bounds: Rectangle,
        _: Rectangle,
        translation: Vector,
        _: &mut dyn operation::Scrollable,
    ) {
        self.pending_scroll = Some((translation, bounds - self.offset));
    }

    fn container(&mut self, id: Option<&iced::widget::Id>, bounds: Rectangle) {
        let Some((_, key)) = self.targets.iter().find(|(target, _)| Some(target) == id) else {
            return;
        };
        let mut bounds = bounds - self.offset;
        // Cards and the body viewport clip descendants. Individual controls retain
        // their content coordinates, including when they need scrolling into view.
        if key.1 <= 1
            && let Some(clip) = self.clip
        {
            bounds = bounds.intersection(&clip).unwrap_or_default();
        }
        self.bounds.insert(*key, bounds);
    }

    fn finish(&self) -> operation::Outcome<Bounds> {
        if self.bounds.is_empty() || self.bounds == self.previous {
            operation::Outcome::None
        } else {
            operation::Outcome::Some(self.bounds.clone())
        }
    }
}
