//! Map renderer-independent configuration publications to the Iced frontend.
use super::*;
pub(super) use crate::configuration::jobs::{Published, Reply};
use iced::futures::StreamExt;
use std::sync::Mutex;

#[derive(Clone)]
pub(super) struct Notices(Arc<Mutex<Option<iced::futures::channel::mpsc::Receiver<()>>>>);
impl PartialEq for Notices {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for Notices {}
impl std::hash::Hash for Notices {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&Arc::as_ptr(&self.0), state);
    }
}
impl Notices {
    pub fn new(receiver: iced::futures::channel::mpsc::Receiver<()>) -> Self {
        Self(Arc::new(Mutex::new(Some(receiver))))
    }
    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::run_with(self.clone(), |key| {
            let receiver = key.0.lock().unwrap_or_else(|e| e.into_inner()).take();
            iced::futures::stream::unfold(receiver, |receiver| async move {
                let mut receiver = receiver?;
                receiver.next().await?;
                Some((Message::ConfigurationPoll, Some(receiver)))
            })
        })
    }
}

impl Desktop {
    pub(super) fn configuration_poll(&mut self) -> Task<Message> {
        let event = self
            .configuration_worker
            .as_ref()
            .and_then(|worker| worker.snapshot().latest);
        let task = event.map_or_else(Task::none, |event| self.install_configuration(event));
        Task::batch([task, self.exit_if_empty()])
    }

    pub(super) fn install_configuration(&mut self, event: Published) -> Task<Message> {
        if event.revision() <= self.configuration_revision {
            return Task::none();
        }
        self.configuration_revision = event.revision();
        self.configuration_error = event.error;
        if event.current_revision <= self.configuration_applied {
            return Task::none();
        }
        self.configuration_applied = event.current_revision;
        let snapshot = event.current;
        self.theme = canopa_ui::theme(snapshot.appearance.clone());
        let _ = self
            .preferences
            .set_defaults(snapshot.settings.view.clone());
        self.configuration = (*snapshot).clone();
        let icons = self.icons.refresh(&self.configuration);
        let views = self.publish_folder_views(None);
        let ids: Vec<_> = self.windows.keys().copied().collect();
        Task::batch(
            [icons, views].into_iter().chain(
                ids.into_iter()
                    .flat_map(|id| [self.restore_scroll(id), self.load_columns(id)]),
            ),
        )
    }
}
