//! Live-resource admission and revision-bound images share one lifetime owner.
//!
//! The runtime's mutation coordinator covers admission through committed effects.
//! Admission is read-only: a failed lifecycle write needs no reservation rollback.
use crate::domain::{
    AcceptedNotification, DeliveryOutcome, NotificationBodyImageSource, NotificationBodySegment,
    NotificationImageData, NotificationVisualReference, Urgency,
};
use crate::lifecycle::{LifecycleEffect, LifecycleEffects};
use crate::{InternalNotificationId, NotificationLimitExceeded};
use std::collections::HashMap;

const LIVE_NOTIFICATION_LIMIT: usize = 1024;
const LIVE_TEXT_LIMIT: usize = 16 * 1024 * 1024;
const LIVE_IMAGE_LIMIT: usize = 64 * 1024 * 1024;

struct Entry {
    revision: i64,
    text_bytes: usize,
    image: Option<NotificationImageData>,
    urgency: Urgency,
    /// Admission order of the current revision; the lowest is the oldest.
    sequence: u64,
}

impl Entry {
    fn image_bytes(&self) -> usize {
        self.image.as_ref().map_or(0, |image| image.data.len())
    }
}

#[derive(Default)]
pub(crate) struct RuntimeResources {
    live: HashMap<InternalNotificationId, Entry>,
    text_bytes: usize,
    image_bytes: usize,
    next_sequence: u64,
}

impl RuntimeResources {
    /// Returns the live thread that must be retired first when the live count is
    /// full: the oldest non-critical revision. The text and image budgets are
    /// checked as if it were already gone, so an admission that would still fail
    /// retires nothing. With only critical threads live, the count is refused.
    pub(crate) fn admit(
        &self,
        notification: &AcceptedNotification,
        outcome: DeliveryOutcome,
        image: Option<&NotificationImageData>,
    ) -> Result<Option<InternalNotificationId>, NotificationLimitExceeded> {
        if outcome == DeliveryOutcome::Dropped {
            return Ok(None);
        }
        let previous = notification.replaces_id.and_then(|id| self.live.get(&id));
        let evicted = if previous.is_none() && self.live.len() >= LIVE_NOTIFICATION_LIMIT {
            Some(
                self.oldest_non_critical()
                    .ok_or(NotificationLimitExceeded("live count"))?,
            )
        } else {
            None
        };
        let freed = previous
            .into_iter()
            .chain(evicted.map(|(_, entry)| entry))
            .collect::<Vec<_>>();
        let text_bytes =
            self.text_bytes - freed.iter().map(|entry| entry.text_bytes).sum::<usize>();
        if accepted_text_bytes(notification) > LIVE_TEXT_LIMIT - text_bytes {
            return Err(NotificationLimitExceeded("live text"));
        }
        if outcome == DeliveryOutcome::Shown {
            let image_bytes =
                self.image_bytes - freed.iter().map(|entry| entry.image_bytes()).sum::<usize>();
            if image.map_or(0, |image| image.data.len()) > LIVE_IMAGE_LIMIT - image_bytes {
                return Err(NotificationLimitExceeded("live image"));
            }
        }
        Ok(evicted.map(|(id, _)| id))
    }

    fn oldest_non_critical(&self) -> Option<(InternalNotificationId, &Entry)> {
        self.live
            .iter()
            .filter(|(_, entry)| entry.urgency != Urgency::Critical)
            .min_by_key(|(_, entry)| entry.sequence)
            .map(|(id, entry)| (*id, entry))
    }

    pub(crate) fn record_receipt(
        &mut self,
        effects: &LifecycleEffects,
        image: Option<NotificationImageData>,
    ) {
        // A receipt creates or revises at most one thread; drops leave it untouched.
        for effect in effects {
            if let LifecycleEffect::ThreadCreated(thread) | LifecycleEffect::ThreadRevised(thread) =
                effect
            {
                self.remove(thread.id);
                let text_bytes = accepted_text_bytes(&thread.accepted_notification);
                self.text_bytes += text_bytes;
                let image = if thread.delivery_outcome == DeliveryOutcome::Shown {
                    image
                } else {
                    None
                };
                self.image_bytes += image.as_ref().map_or(0, |image| image.data.len());
                self.next_sequence += 1;
                self.live.insert(
                    thread.id,
                    Entry {
                        revision: thread.revision,
                        text_bytes,
                        image,
                        urgency: thread.accepted_notification.urgency,
                        sequence: self.next_sequence,
                    },
                );
                break;
            }
        }
    }

    pub(crate) fn release(&mut self, effects: &LifecycleEffects) {
        for effect in effects {
            match effect {
                LifecycleEffect::ThreadSilenced(thread) => {
                    if let Some(entry) = self.live.get_mut(&thread.id) {
                        self.image_bytes -= entry.image_bytes();
                        entry.image = None;
                    }
                }
                LifecycleEffect::ThreadDismissed(thread)
                | LifecycleEffect::ThreadExpired(thread)
                | LifecycleEffect::ThreadClosed(thread)
                | LifecycleEffect::ThreadStale(thread) => {
                    self.remove(thread.id);
                }
                LifecycleEffect::ThreadAttentionChanged { .. }
                | LifecycleEffect::SenderReceipt(_)
                | LifecycleEffect::ThreadCreated(_)
                | LifecycleEffect::ThreadRevised(_)
                | LifecycleEffect::NotificationDropped { .. }
                | LifecycleEffect::NotificationTimeoutStartsOnDisplay { .. }
                | LifecycleEffect::NotificationTimeoutCancelledThroughRevision { .. }
                | LifecycleEffect::ActionInvoked { .. }
                | LifecycleEffect::SenderClosed { .. } => {}
            }
        }
    }

    fn remove(&mut self, notification_id: InternalNotificationId) {
        if let Some(entry) = self.live.remove(&notification_id) {
            self.text_bytes -= entry.text_bytes;
            self.image_bytes -= entry.image_bytes();
        }
    }

    pub(crate) fn image_for(
        &self,
        notification_id: InternalNotificationId,
        revision: i64,
    ) -> Option<NotificationImageData> {
        self.live
            .get(&notification_id)
            .filter(|entry| entry.revision == revision)
            .and_then(|entry| entry.image.clone())
    }
}

// Logical UTF-8 storage, including duplicated accepted representations. Intake's
// per-request bounds make this sum bounded; new accepted fields require a choice here.
fn accepted_text_bytes(notification: &AcceptedNotification) -> usize {
    let AcceptedNotification {
        app_identity,
        app_name,
        replaces_id: _,
        app_icon,
        summary,
        body,
        body_content,
        raw_body,
        urgency: _,
        category,
        progress: _,
        image_reference,
        has_image_payload: _,
        actions,
        resident: _,
        transient: _,
    } = notification;
    let mut bytes =
        app_identity.len() + app_name.len() + summary.len() + body.len() + raw_body.len();
    bytes += category.as_ref().map_or(0, String::len);
    bytes += app_icon.as_ref().map_or(0, visual_text_bytes);
    bytes += image_reference.as_ref().map_or(0, visual_text_bytes);
    for segment in &body_content.segments {
        bytes += match segment {
            NotificationBodySegment::Text { text, .. } => text.len(),
            NotificationBodySegment::Link { text, uri, .. } => text.len() + uri.as_str().len(),
            NotificationBodySegment::Image { image } => {
                let NotificationBodyImageSource::LocalPath { path } = &image.source;
                path.len() + image.alt.as_ref().map_or(0, String::len)
            }
        };
    }
    for action in actions {
        bytes += action.key.len()
            + action.label.len()
            + action.icon.as_ref().map_or(0, visual_text_bytes);
    }
    bytes
}

fn visual_text_bytes(visual: &NotificationVisualReference) -> usize {
    match visual {
        NotificationVisualReference::IconName(value)
        | NotificationVisualReference::LocalPath(value) => value.len(),
        NotificationVisualReference::IconNames(names) => names.iter().map(String::len).sum(),
    }
}
