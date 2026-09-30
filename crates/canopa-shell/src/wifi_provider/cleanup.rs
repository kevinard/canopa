//! Cleanup survives cancellation of a UI operation and is awaited before exit.
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct Cleanup(Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>);

impl Cleanup {
    pub(super) fn spawn(&self, work: impl Future<Output = ()> + Send + 'static) {
        let mut tasks = self.0.lock().unwrap();
        tasks.retain(|task| !task.is_finished());
        tasks.push(tokio::spawn(work));
    }

    pub async fn finish(&self) {
        loop {
            let tasks = std::mem::take(&mut *self.0.lock().unwrap());
            if tasks.is_empty() {
                break;
            }
            for task in tasks {
                let _ = task.await;
            }
        }
    }
}
