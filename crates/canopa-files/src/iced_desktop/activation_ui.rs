//! Translate accepted launch intent into frontend windows.
use super::*;
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
    pub fn subscription(&self) -> Subscription<()> {
        Subscription::run_with(self.clone(), |key| {
            let receiver = key.0.lock().unwrap_or_else(|e| e.into_inner()).take();
            iced::futures::stream::unfold(receiver, |receiver| async move {
                let mut receiver = receiver?;
                receiver.next().await?;
                Some(((), Some(receiver)))
            })
        })
    }
}

impl Desktop {
    pub(super) fn open_requested(&mut self, opening: crate::activation::Opening) -> Task<Message> {
        let task = match opening.presentation.view.clone() {
            crate::presentation::View::Browsers(mut targets) => {
                if targets.is_empty() {
                    targets.push(crate::presentation::BrowserTarget {
                        directory: self.home.clone(),
                        selected: Vec::new(),
                    });
                }
                self.new_target_window(targets)
            }
            crate::presentation::View::Properties(paths) => self.show_properties(paths),
        };
        if let Some(instance) = &self.instance {
            instance.presented(opening);
        }
        task
    }

    pub(super) fn activation_poll(&mut self) -> Task<Message> {
        let openings = self
            .instance
            .as_ref()
            .map_or_else(Vec::new, crate::activation::Instance::take_openings);
        let mut tasks = Vec::with_capacity(openings.len() + 1);
        for opening in openings {
            tasks.push(self.open_requested(opening));
        }
        tasks.push(self.exit_if_empty());
        Task::batch(tasks)
    }
}
