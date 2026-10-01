//! Bounded renderer intake. Reject the newest gesture on overload; preserve the
//! captured target of every accepted interaction. Slow token requests may run
//! concurrently, but neither the queue nor the worker set grows without limit.
use canopa_notify::{BannerInteraction, Daemon};
use tokio::sync::{mpsc, oneshot};
use tokio::task::{JoinHandle, JoinSet};

#[cfg(any(feature = "gui", test))]
pub(crate) const QUEUE_CAPACITY: usize = 32;
const WORKERS: usize = 4;

pub(crate) struct Forwarder {
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
}

impl Forwarder {
    pub(crate) fn spawn(
        daemon: Daemon,
        mut interactions: mpsc::Receiver<BannerInteraction>,
    ) -> Self {
        let (stop, mut stopping) = oneshot::channel();
        let task = tokio::spawn(async move {
            let mut jobs = JoinSet::new();
            let mut closed = false;
            let mut exhausted = false;
            loop {
                tokio::select! {
                    biased;
                    _ = &mut stopping, if !closed => {
                        closed = true;
                        interactions.close();
                    }
                    Some(result) = jobs.join_next(), if !jobs.is_empty() => {
                        if !matches!(result, Ok(Ok(()))) {
                            eprintln!("level=warn component=presentation event=banner_interaction_failed");
                        }
                    }
                    interaction = interactions.recv(), if !exhausted && jobs.len() < WORKERS => {
                        match interaction {
                            Some(interaction) => {
                                let daemon = daemon.clone();
                                jobs.spawn(async move { daemon.handle_banner_interaction(interaction).await });
                            }
                            None => exhausted = true,
                        }
                    }
                    else => break,
                }
                if exhausted && jobs.is_empty() {
                    break;
                }
            }
        });
        Self {
            stop: Some(stop),
            task: Some(task),
        }
    }

    /// Close admission, drain the finite accepted queue, and join all workers
    /// before the caller shuts down the daemon's adapters.
    pub(crate) async fn shutdown(mut self) -> anyhow::Result<()> {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(task) = self.task.as_mut() {
            task.await?;
        }
        self.task.take();
        Ok(())
    }
}

impl Drop for Forwarder {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
        // JoinSet drop cancels preparation/waiting calls. Admitted mutations have
        // already transferred completion ownership to Notification Runtime.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use canopa_notify::{
        BannerTarget, DaemonOptions, NotificationAction, NotificationInput, NotificationTimeout,
        SenderEvent, SenderEventPublication, SenderEventPublisher,
    };
    use std::sync::Arc;
    use std::time::Duration;

    #[derive(Clone)]
    struct PausedPublication {
        entered: Arc<tokio::sync::Notify>,
        release: Arc<tokio::sync::Semaphore>,
    }
    impl SenderEventPublisher for PausedPublication {
        fn publish(&self, _: SenderEvent) -> SenderEventPublication {
            let gate = self.clone();
            Box::pin(async move {
                gate.entered.notify_one();
                gate.release.acquire().await.unwrap().forget();
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn blocked_publication_bounds_intake_and_shutdown_drains_accepted_gestures() {
        let directory = tempfile::tempdir().unwrap();
        let daemon = Daemon::open(DaemonOptions::headless(directory.path().join("history.db")))
            .await
            .unwrap();
        let gate = PausedPublication {
            entered: Default::default(),
            release: Arc::new(tokio::sync::Semaphore::new(0)),
        };
        daemon.install_sender_event_publisher(gate.clone()).unwrap();
        let receipt = daemon
            .receive_notification(NotificationInput {
                summary: "Bounded work".into(),
                resident: true,
                expire_timeout: NotificationTimeout::Never,
                actions: vec![NotificationAction::new("reply", "Reply")],
                ..Default::default()
            })
            .await
            .unwrap();
        let interaction = || BannerInteraction::InvokeAction {
            target: BannerTarget {
                notification_id: receipt.notification_id,
                revision: 1,
            },
            action_key: "reply".into(),
            activation_token: None,
        };
        let (sender, receiver) = mpsc::channel(QUEUE_CAPACITY);
        let forwarder = Forwarder::spawn(daemon.clone(), receiver);
        sender.try_send(interaction()).unwrap();
        tokio::time::timeout(Duration::from_secs(1), gate.entered.notified())
            .await
            .unwrap();
        let accepted = (0..128)
            .filter(|_| sender.try_send(interaction()).is_ok())
            .count();
        assert_eq!(accepted, QUEUE_CAPACITY);
        let mut shutdown = tokio::spawn(forwarder.shutdown());
        assert!(
            tokio::time::timeout(Duration::from_millis(20), &mut shutdown)
                .await
                .is_err()
        );
        assert!(
            sender.is_closed(),
            "shutdown must close admission before waiting for publication"
        );
        assert!(sender.try_send(interaction()).is_err());
        gate.release.add_permits(accepted + 1);
        tokio::time::timeout(Duration::from_secs(2), shutdown)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(daemon.sender_events().await.len(), accepted + 1);
        daemon.shutdown().await.unwrap();
    }
    #[tokio::test]
    async fn cancelling_shutdown_keeps_dispatcher_and_waiting_workers_owned() {
        let directory = tempfile::tempdir().unwrap();
        let daemon = Daemon::open(DaemonOptions::headless(directory.path().join("history.db")))
            .await
            .unwrap();
        let gate = PausedPublication {
            entered: Default::default(),
            release: Arc::new(tokio::sync::Semaphore::new(0)),
        };
        daemon.install_sender_event_publisher(gate.clone()).unwrap();
        let receipt = daemon
            .receive_notification(NotificationInput {
                summary: "Cancellation".into(),
                resident: true,
                expire_timeout: NotificationTimeout::Never,
                actions: vec![NotificationAction::new("reply", "Reply")],
                ..Default::default()
            })
            .await
            .unwrap();
        let (sender, receiver) = mpsc::channel(QUEUE_CAPACITY);
        let forwarder = Forwarder::spawn(daemon.clone(), receiver);
        let dispatcher = forwarder.task.as_ref().unwrap().abort_handle();
        sender
            .send(BannerInteraction::InvokeAction {
                target: BannerTarget {
                    notification_id: receipt.notification_id,
                    revision: 1,
                },
                action_key: "reply".into(),
                activation_token: None,
            })
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(1), gate.entered.notified())
            .await
            .unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(20), forwarder.shutdown())
                .await
                .is_err()
        );
        let stopped = tokio::time::timeout(Duration::from_secs(1), async {
            while !dispatcher.is_finished() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .is_ok();
        // Always release admitted Runtime work, even if the ownership assertion fails.
        dispatcher.abort();
        gate.release.add_permits(1);
        daemon.shutdown().await.unwrap();
        assert!(
            stopped,
            "cancelled shutdown detached the forwarder's dispatcher"
        );
    }
}
