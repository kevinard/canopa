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
    pub(super) fn present_requested_preview(&mut self, id: window::Id) -> Task<Message> {
        let ready = self
            .windows
            .get(&id)
            .is_some_and(|window| window.size_confirmed)
            && self
                .active(id)
                .is_some_and(|tab| !tab.loading && tab.browser.is_some());
        if !ready {
            return Task::none();
        }
        let Some(path) = self
            .active_mut(id)
            .and_then(|tab| tab.preview_after_load.take())
        else {
            return Task::none();
        };
        if self
            .active(id)
            .and_then(|tab| tab.browser.as_ref())
            .and_then(|browser| browser.focused())
            != Some(path.as_path())
        {
            if let Some(tab) = self.active_mut(id) {
                tab.error = Some(fl!("files-quick-look-unavailable"));
            }
            return Task::none();
        }
        self.toggle_preview(id)
    }
    pub(super) fn open_requested(&mut self, opening: crate::activation::Opening) -> Task<Message> {
        let task = match opening.presentation.view.clone() {
            crate::presentation::View::QuickLook(path) => {
                let directory = path.parent().unwrap_or(&path).to_path_buf();
                self.new_target_window_with_preview(
                    vec![crate::presentation::BrowserTarget {
                        directory,
                        selected: vec![path],
                    }],
                    true,
                )
            }
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
