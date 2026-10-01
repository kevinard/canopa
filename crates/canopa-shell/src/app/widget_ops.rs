pub(crate) fn module_widget_id(
    window: iced::window::Id,
    id: &crate::modules::ModuleId,
) -> iced::widget::Id {
    iced::widget::Id::from(format!("module:{id:?}:{window:?}"))
}

pub(super) struct FindBounds {
    pub(super) target: iced::widget::Id,
    pub(super) result: Option<iced::Rectangle>,
}

impl iced::advanced::widget::Operation<Option<iced::Rectangle>> for FindBounds {
    fn traverse(
        &mut self,
        operate: &mut dyn FnMut(
            &mut dyn iced::advanced::widget::Operation<Option<iced::Rectangle>>,
        ),
    ) {
        operate(self);
    }

    fn container(&mut self, id: Option<&iced::widget::Id>, bounds: iced::Rectangle) {
        if id == Some(&self.target) {
            self.result = Some(bounds);
        }
    }

    fn finish(&self) -> iced::advanced::widget::operation::Outcome<Option<iced::Rectangle>> {
        iced::advanced::widget::operation::Outcome::Some(self.result)
    }
}
