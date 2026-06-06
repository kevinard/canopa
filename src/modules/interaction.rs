use super::InteractionEvent;

pub trait ModuleInteractionHandler: Send + Sync {
    fn on_interaction(&self, event: InteractionEvent) -> Option<crate::app::Message>;
}
