use super::{ModuleDetailMessage, ModuleDetailSession, ModuleDetailUpdate, ModuleState};

pub trait ModuleDetailLifecycle: Send + Sync {
    fn create_module_detail_session(&self, state: &ModuleState) -> Option<ModuleDetailSession>;

    fn update_module_detail_session(
        &mut self,
        session: &mut ModuleDetailSession,
        message: ModuleDetailMessage,
    ) -> ModuleDetailUpdate;
}
