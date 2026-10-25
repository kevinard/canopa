//! The view subscribes to a catalog; dropping it retires only its inspection.
use super::*;
use iced::futures::{StreamExt, channel::mpsc};
use std::sync::Mutex;

#[derive(Clone)]
pub(super) struct Notices(Arc<Mutex<Option<mpsc::Receiver<()>>>>, window::Id);
impl PartialEq for Notices {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) && self.1 == other.1
    }
}
impl Eq for Notices {}
impl std::hash::Hash for Notices {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&Arc::as_ptr(&self.0), state);
        std::hash::Hash::hash(&self.1, state);
    }
}
impl Notices {
    pub fn new(receiver: Option<mpsc::Receiver<()>>, window: window::Id) -> Self {
        Self(Arc::new(Mutex::new(receiver)), window)
    }
    pub fn subscription(&self) -> Subscription<window::Id> {
        Subscription::run_with(self.clone(), |key| {
            let receiver = key.0.lock().unwrap_or_else(|e| e.into_inner()).take();
            let window = key.1;
            iced::futures::stream::unfold(receiver, move |receiver| async move {
                let mut receiver = receiver?;
                receiver.next().await?;
                Some((window, Some(receiver)))
            })
        })
    }
}
