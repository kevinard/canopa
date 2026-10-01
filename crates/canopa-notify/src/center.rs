//! Bounded Center reads share one permission snapshot and lightweight transient
//! index for a frame. Content is fetched only in bounded batches; no full-history
//! projection or long-lived database read transaction is retained.
use crate::lifecycle::NotificationLifecycle;
use crate::sqlite_persisted_state::SqlitePersistedState;
use crate::{
    AppPermissionControl, BannerIconSettings, CategoryPermissionControl, NotificationCenterBatch,
    NotificationCenterCursor, NotificationCenterGroup, NotificationCenterItem,
    NotificationCenterProjection, NotificationCenterTarget, NotificationStatus, NotificationThread,
    UserRule,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub struct NotificationCenterReader {
    storage: SqlitePersistedState,
    lifecycle: NotificationLifecycle<SqlitePersistedState>,
    projection: CenterProjection,
    transient: Vec<NotificationCenterCursor>,
}

/// One bounded read row carries its position and page-local permission controls.
/// The projection contains this row only, so an overflowing row cannot add another
/// category's controls to the preceding page.
pub struct NotificationCenterRow {
    pub cursor: NotificationCenterCursor,
    pub projection: NotificationCenterProjection,
}

impl NotificationCenterReader {
    pub(crate) fn new(
        storage: SqlitePersistedState,
        lifecycle: NotificationLifecycle<SqlitePersistedState>,
        projection: CenterProjection,
    ) -> Self {
        let transient = lifecycle.transient_center_cursors();
        Self {
            storage,
            lifecycle,
            projection,
            transient,
        }
    }

    async fn threads(
        &self,
        after: Option<NotificationCenterCursor>,
        limit: u8,
    ) -> anyhow::Result<Vec<NotificationThread>> {
        anyhow::ensure!(
            (1..=16).contains(&limit),
            "center batch limit must be between 1 and 16"
        );
        let mut threads = self.storage.center_threads_after(after, limit).await?;
        threads.extend(
            self.lifecycle
                .transient_center_threads(&self.transient, after, limit),
        );
        threads
            .sort_by_key(|thread| std::cmp::Reverse(NotificationCenterCursor::for_thread(thread)));
        threads.truncate(usize::from(limit));
        Ok(threads)
    }

    pub async fn read(
        &self,
        after: Option<NotificationCenterCursor>,
        limit: u8,
    ) -> anyhow::Result<Vec<NotificationCenterRow>> {
        Ok(self
            .threads(after, limit)
            .await?
            .into_iter()
            .map(|thread| NotificationCenterRow {
                cursor: NotificationCenterCursor::for_thread(&thread),
                projection: self.projection.project(vec![thread]),
            })
            .collect())
    }

    pub(crate) async fn batch(
        &self,
        after: Option<NotificationCenterCursor>,
        limit: u8,
    ) -> anyhow::Result<NotificationCenterBatch> {
        let threads = self.threads(after, limit).await?;
        Ok(NotificationCenterBatch {
            last: threads.last().map(NotificationCenterCursor::for_thread),
            projection: self.projection.project(threads),
        })
    }
}

pub(crate) struct CenterProjection {
    app_permissions: HashMap<String, AppPermissionControl>,
    category_permissions: HashMap<(String, String), AppPermissionControl>,
    instance: [u8; 16],
    icon_settings: BannerIconSettings,
}

impl CenterProjection {
    pub(crate) fn new(
        user_rules: &[UserRule],
        instance: [u8; 16],
        icon_settings: BannerIconSettings,
    ) -> Self {
        let app_permissions: HashMap<_, _> = user_rules
            .iter()
            .filter(|rule| rule.category.is_none() && rule.urgency.is_none())
            .map(|rule| {
                (
                    rule.app_identity.clone(),
                    AppPermissionControl::from_delivery_outcome(rule.outcome),
                )
            })
            .collect();
        let category_permissions: HashMap<_, _> = user_rules
            .iter()
            .filter_map(|rule| {
                if rule.urgency.is_some() {
                    return None;
                }
                let category = rule.category.as_ref()?;
                Some((
                    (rule.app_identity.clone(), category.clone()),
                    AppPermissionControl::from_delivery_outcome(rule.outcome),
                ))
            })
            .collect();
        Self {
            app_permissions,
            category_permissions,
            instance,
            icon_settings,
        }
    }

    pub(crate) fn project(&self, threads: Vec<NotificationThread>) -> NotificationCenterProjection {
        let Self {
            app_permissions,
            category_permissions,
            instance,
            icon_settings,
        } = self;
        let mut grouped: BTreeMap<String, Vec<NotificationCenterItem>> = BTreeMap::new();
        let mut categories: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

        for thread in threads {
            let notification = thread.accepted_notification;
            if let Some(category) = &notification.category {
                categories
                    .entry(notification.app_identity.clone())
                    .or_default()
                    .insert(category.clone());
            }
            let actions = if thread.status == NotificationStatus::Active {
                notification
                    .actions
                    .iter()
                    .filter(|action| action.key != "default")
                    .cloned()
                    .collect()
            } else {
                Vec::new()
            };
            grouped
                .entry(notification.app_identity.clone())
                .or_default()
                .push(NotificationCenterItem {
                    app_name: notification.app_name,
                    app_icon: notification.app_icon,
                    requires_attention: thread.requires_attention,
                    notification_id: thread.id,
                    target: NotificationCenterTarget {
                        instance: *instance,
                        notification_id: thread.id,
                        revision: thread.revision,
                    },
                    transient: notification.transient,
                    has_default_action: thread.status == NotificationStatus::Active
                        && notification
                            .actions
                            .iter()
                            .any(|action| action.key == "default"),
                    summary: notification.summary,
                    body: notification.body,
                    body_content: notification.body_content,
                    status: thread.status,
                    delivery_outcome: thread.delivery_outcome,
                    actions,
                });
        }

        NotificationCenterProjection {
            icon_settings: icon_settings.clone(),
            groups: grouped
                .into_iter()
                .map(|(app_identity, items)| NotificationCenterGroup {
                    permission: app_permissions
                        .get(app_identity.as_str())
                        .copied()
                        .unwrap_or(AppPermissionControl::Allow),
                    category_permissions: categories
                        .remove(&app_identity)
                        .unwrap_or_default()
                        .into_iter()
                        .map(|category| CategoryPermissionControl {
                            permission: category_permissions
                                .get(&(app_identity.clone(), category.clone()))
                                .copied()
                                .or_else(|| app_permissions.get(app_identity.as_str()).copied())
                                .unwrap_or(AppPermissionControl::Allow),
                            category,
                        })
                        .collect(),
                    app_identity,
                    items,
                })
                .collect(),
        }
    }
}
