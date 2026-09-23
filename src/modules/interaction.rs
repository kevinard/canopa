use super::InteractionEvent;

pub trait ModuleInteractionHandler: Send + Sync {
    fn on_interaction(&self, event: InteractionEvent) -> Option<crate::app::Message>;

    fn on_interaction_in_window(
        &self,
        event: InteractionEvent,
        _window: iced::window::Id,
    ) -> Option<crate::app::Message> {
        self.on_interaction(event)
    }
}
