use crate::clock::now_millis;
use crate::domain::{urgency_from_key, urgency_key};
use crate::{
    AcceptedNotification, DeliveryOutcome, NewNotificationThread, NotificationHistory,
    NotificationSnapshot, NotificationStatus, NotificationThread, PersistPrecondition, Urgency,
    UserRule,
};
use anyhow::Context;
use sqlx::query::Query;
use sqlx::sqlite::{SqliteArguments, SqliteConnectOptions, SqlitePoolOptions, SqliteRow};
use sqlx::{Row, Sqlite, SqlitePool};
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Clone)]
pub(crate) struct SqlitePersistedState {
    pool: SqlitePool,
    retention: RetentionPolicy,
}

#[derive(Clone, Copy)]
struct RetentionPolicy {
    days: i64,
    history_limit: i64,
}

macro_rules! notification_thread_columns {
    () => {
        "id, app_identity, app_name, replaces_id, summary, body, raw_body, urgency, status,
         delivery_outcome, rule_reference, category, progress, resident, actions_json, revision,
         created_at, updated_at, closed_at"
    };
}

macro_rules! notification_thread_write_columns {
    () => {
        "app_identity, app_name, replaces_id, summary, body, raw_body, urgency, status,
         delivery_outcome, rule_reference, category, progress, resident, actions_json, revision,
         created_at, updated_at, closed_at"
    };
}

macro_rules! notification_thread_write_placeholders {
    () => {
        "?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18"
    };
}

macro_rules! notification_thread_update_assignments {
    () => {
        "app_identity = ?2,
         app_name = ?3,
         replaces_id = ?4,
         summary = ?5,
         body = ?6,
         raw_body = ?7,
         urgency = ?8,
         status = ?9,
         delivery_outcome = ?10,
         rule_reference = ?11,
         category = ?12,
         progress = ?13,
         resident = ?14,
         actions_json = ?15,
         revision = ?16,
         created_at = ?17,
         updated_at = ?18,
         closed_at = ?19"
    };
}

struct NotificationThreadRecord {
    id: Option<i64>,
    app_identity: String,
    app_name: String,
    replaces_id: Option<i64>,
    summary: String,
    body: String,
    raw_body: String,
    urgency: Urgency,
    status: NotificationStatus,
    delivery_outcome: DeliveryOutcome,
    rule_reference: Option<String>,
    category: Option<String>,
    progress: Option<i64>,
    actions_json: String,
    resident: bool,
    revision: i64,
    created_at: i64,
    updated_at: i64,
    closed_at: Option<i64>,
}

impl NotificationThreadRecord {
    fn from_new_thread(thread: NewNotificationThread) -> anyhow::Result<Self> {
        let notification = thread.accepted_notification;
        Ok(Self {
            id: None,
            app_identity: notification.app_identity,
            app_name: notification.app_name,
            replaces_id: notification.replaces_id,
            summary: notification.summary,
            body: notification.body,
            raw_body: notification.raw_body,
            urgency: notification.urgency,
            status: thread.status,
            delivery_outcome: thread.delivery_outcome,
            rule_reference: thread.rule_reference,
            category: notification.category,
            progress: notification.progress,
            actions_json: serde_json::to_string(&notification.actions)?,
            resident: notification.resident,
            revision: 1,
            created_at: thread.created_at,
            updated_at: thread.updated_at,
            closed_at: None,
        })
    }

    fn from_thread(thread: NotificationThread) -> anyhow::Result<Self> {
        let notification = thread.accepted_notification;
        Ok(Self {
            id: Some(thread.id),
            app_identity: notification.app_identity,
            app_name: notification.app_name,
            replaces_id: notification.replaces_id,
            summary: notification.summary,
            body: notification.body,
            raw_body: notification.raw_body,
            urgency: notification.urgency,
            status: thread.status,
            delivery_outcome: thread.delivery_outcome,
            rule_reference: thread.rule_reference,
            category: notification.category,
            progress: notification.progress,
            actions_json: serde_json::to_string(&notification.actions)?,
            resident: notification.resident,
            revision: thread.revision,
            created_at: thread.created_at,
            updated_at: thread.updated_at,
            closed_at: thread.closed_at,
        })
    }

    fn from_row(row: &SqliteRow) -> anyhow::Result<Self> {
        Ok(Self {
            id: Some(row.try_get("id")?),
            app_identity: row.try_get("app_identity")?,
            app_name: row.try_get("app_name")?,
            replaces_id: row.try_get("replaces_id")?,
            summary: row.try_get("summary")?,
            body: row.try_get("body")?,
            raw_body: row.try_get("raw_body")?,
            urgency: Urgency::from_str(&row.try_get::<String, _>("urgency")?),
            status: NotificationStatus::from_str(&row.try_get::<String, _>("status")?),
            delivery_outcome: DeliveryOutcome::from_str(
                &row.try_get::<String, _>("delivery_outcome")?,
            ),
            rule_reference: row.try_get("rule_reference")?,
            category: row.try_get("category")?,
            progress: row.try_get("progress")?,
            actions_json: row.try_get("actions_json")?,
            resident: row.try_get::<i64, _>("resident")? != 0,
            revision: row.try_get("revision")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
            closed_at: row.try_get("closed_at")?,
        })
    }

    fn id(&self) -> anyhow::Result<i64> {
        self.id
            .ok_or_else(|| anyhow::anyhow!("notification thread record has no persisted id"))
    }

    fn bind_insert<'q>(
        &'q self,
        query: Query<'q, Sqlite, SqliteArguments<'q>>,
    ) -> Query<'q, Sqlite, SqliteArguments<'q>> {
        self.bind_write_values(query)
    }

    fn bind_update<'q>(
        &'q self,
        query: Query<'q, Sqlite, SqliteArguments<'q>>,
        expected_revision: i64,
        expected_status: Option<NotificationStatus>,
    ) -> anyhow::Result<Query<'q, Sqlite, SqliteArguments<'q>>> {
        Ok(self
            .bind_write_values(query.bind(self.id()?))
            .bind(expected_revision)
            .bind(expected_status.map(NotificationStatus::as_str)))
    }

    fn bind_write_values<'q>(
        &'q self,
        query: Query<'q, Sqlite, SqliteArguments<'q>>,
    ) -> Query<'q, Sqlite, SqliteArguments<'q>> {
        query
            .bind(&self.app_identity)
            .bind(&self.app_name)
            .bind(self.replaces_id)
            .bind(&self.summary)
            .bind(&self.body)
            .bind(&self.raw_body)
            .bind(self.urgency.as_str())
            .bind(self.status.as_str())
            .bind(self.delivery_outcome.as_str())
            .bind(self.rule_reference.as_deref())
            .bind(self.category.as_deref())
            .bind(self.progress)
            .bind(self.resident)
            .bind(&self.actions_json)
            .bind(self.revision)
            .bind(self.created_at)
            .bind(self.updated_at)
            .bind(self.closed_at)
    }

    fn into_thread(self) -> anyhow::Result<NotificationThread> {
        let actions = serde_json::from_str(&self.actions_json)?;
        Ok(NotificationThread {
            id: self
                .id
                .expect("persisted notification thread record must have id"),
            accepted_notification: AcceptedNotification {
                app_identity: self.app_identity,
                app_name: self.app_name,
                replaces_id: self.replaces_id,
                summary: self.summary,
                body: self.body,
                raw_body: self.raw_body,
                urgency: self.urgency,
                category: self.category,
                progress: self.progress,
                has_image_payload: false,
                actions,
                resident: self.resident,
            },
            status: self.status,
            delivery_outcome: self.delivery_outcome,
            rule_reference: self.rule_reference,
            revision: self.revision,
            created_at: self.created_at,
            updated_at: self.updated_at,
            closed_at: self.closed_at,
        })
    }
}

struct UserRuleRow {
    app_identity: String,
    urgency: Option<Urgency>,
    outcome: DeliveryOutcome,
    reference: String,
}

impl UserRuleRow {
    fn from_row(row: SqliteRow) -> anyhow::Result<Self> {
        Ok(Self {
            app_identity: row.try_get("app_identity")?,
            urgency: urgency_from_key(&row.try_get::<String, _>("urgency")?),
            outcome: DeliveryOutcome::from_str(&row.try_get::<String, _>("outcome")?),
            reference: row.try_get("reference")?,
        })
    }

    fn into_user_rule(self) -> UserRule {
        UserRule {
            app_identity: self.app_identity,
            urgency: self.urgency,
            outcome: self.outcome,
            reference: self.reference,
        }
    }
}

impl SqlitePersistedState {
    pub(crate) async fn open(
        path: PathBuf,
        retention_days: i64,
        history_limit: i64,
    ) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))
            .with_context(|| format!("invalid SQLite database path {}", path.display()))?
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;

        Self::create_schema(&pool).await?;
        Self::migrate_schema(&pool).await?;

        Ok(Self {
            pool,
            retention: RetentionPolicy {
                days: retention_days,
                history_limit,
            },
        })
    }

    async fn create_schema(pool: &SqlitePool) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS daemon_metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )
            "#,
        )
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS user_rules (
                app_identity TEXT NOT NULL,
                urgency TEXT NOT NULL,
                outcome TEXT NOT NULL,
                reference TEXT NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (app_identity, urgency)
            )
            "#,
        )
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS notification_threads (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                app_identity TEXT NOT NULL,
                app_name TEXT NOT NULL,
                replaces_id INTEGER,
                summary TEXT NOT NULL,
                body TEXT NOT NULL,
                raw_body TEXT NOT NULL,
                urgency TEXT NOT NULL,
                status TEXT NOT NULL,
                delivery_outcome TEXT NOT NULL,
                rule_reference TEXT,
                category TEXT,
                progress INTEGER,
                resident INTEGER NOT NULL DEFAULT 0,
                actions_json TEXT NOT NULL DEFAULT '[]',
                revision INTEGER NOT NULL DEFAULT 1,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                closed_at INTEGER
            )
            "#,
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    async fn migrate_schema(pool: &SqlitePool) -> anyhow::Result<()> {
        let has_replaces_id: Option<String> = sqlx::query_scalar(
            "SELECT name FROM pragma_table_info('notification_threads') WHERE name = 'replaces_id'",
        )
        .fetch_optional(pool)
        .await?;

        if has_replaces_id.is_none() {
            sqlx::query("ALTER TABLE notification_threads ADD COLUMN replaces_id INTEGER")
                .execute(pool)
                .await?;
        }

        Ok(())
    }

    pub(crate) async fn snapshot(&self) -> anyhow::Result<NotificationSnapshot> {
        let rows = sqlx::query(concat!(
            "SELECT ",
            notification_thread_columns!(),
            "
            FROM notification_threads
            ORDER BY created_at DESC, id DESC"
        ))
        .fetch_all(&self.pool)
        .await?;

        let mut active = Vec::new();
        let mut history = Vec::new();

        for row in rows {
            let thread = NotificationThreadRecord::from_row(&row)?.into_thread()?;
            if thread.status == NotificationStatus::Active {
                active.push(thread.clone());
            }
            history.push(thread);
        }

        Ok(NotificationSnapshot { active, history })
    }

    pub(crate) async fn thread(&self, id: i64) -> anyhow::Result<NotificationThread> {
        let row = sqlx::query(concat!(
            "SELECT ",
            notification_thread_columns!(),
            "
            FROM notification_threads
            WHERE id = ?1"
        ))
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        NotificationThreadRecord::from_row(&row)?.into_thread()
    }

    pub(crate) async fn delete_inactive_history(&self) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM notification_threads WHERE status != 'active'")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub(crate) async fn set_do_not_disturb(&self, enabled: bool) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO daemon_metadata (key, value)
            VALUES ('do_not_disturb', ?1)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value
            "#,
        )
        .bind(if enabled { "true" } else { "false" })
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(crate) async fn do_not_disturb(&self) -> anyhow::Result<bool> {
        let value: Option<String> =
            sqlx::query_scalar("SELECT value FROM daemon_metadata WHERE key = 'do_not_disturb'")
                .fetch_optional(&self.pool)
                .await?;
        Ok(value.as_deref() == Some("true"))
    }

    pub(crate) async fn upsert_user_rule(&self, rule: &UserRule) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO user_rules (app_identity, urgency, outcome, reference, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(app_identity, urgency) DO UPDATE SET
                outcome = excluded.outcome,
                reference = excluded.reference,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&rule.app_identity)
        .bind(urgency_key(rule.urgency))
        .bind(rule.outcome.as_str())
        .bind(&rule.reference)
        .bind(now_millis())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(crate) async fn user_rules(&self) -> anyhow::Result<Vec<UserRule>> {
        let rows = sqlx::query(
            "SELECT app_identity, urgency, outcome, reference FROM user_rules ORDER BY app_identity, urgency",
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| UserRuleRow::from_row(row).map(UserRuleRow::into_user_rule))
            .collect()
    }

    pub(crate) async fn remove_user_rule(
        &self,
        app_identity: &str,
        urgency: Option<Urgency>,
    ) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM user_rules WHERE app_identity = ?1 AND urgency = ?2")
            .bind(app_identity)
            .bind(urgency_key(urgency))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub(crate) async fn prune_history(&self) -> anyhow::Result<()> {
        let cutoff = now_millis() - self.retention.days.max(0) * 24 * 60 * 60 * 1_000;

        sqlx::query(
            "DELETE FROM notification_threads WHERE status != 'active' AND updated_at < ?1",
        )
        .bind(cutoff)
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            DELETE FROM notification_threads
            WHERE status != 'active'
              AND id IN (
                  SELECT id
                  FROM notification_threads
                  WHERE status != 'active'
                  ORDER BY updated_at DESC, id DESC
                  LIMIT -1 OFFSET ?1
              )
            "#,
        )
        .bind(self.retention.history_limit.max(0))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // Call after fetching the write result so lifecycle effects do not depend on retention keeping the row.
    async fn finish_history_write<T>(&self, value: T) -> anyhow::Result<T> {
        self.prune_history().await?;
        Ok(value)
    }
}

impl NotificationHistory for SqlitePersistedState {
    async fn create_thread(
        &self,
        thread: NewNotificationThread,
    ) -> anyhow::Result<NotificationThread> {
        let record = NotificationThreadRecord::from_new_thread(thread)?;
        let id = record
            .bind_insert(sqlx::query(concat!(
                "INSERT INTO notification_threads (",
                notification_thread_write_columns!(),
                ")
                 VALUES (",
                notification_thread_write_placeholders!(),
                ")"
            )))
            .execute(&self.pool)
            .await?
            .last_insert_rowid();
        let thread = self.thread(id).await?;
        self.finish_history_write(thread).await
    }

    async fn thread(&self, id: i64) -> anyhow::Result<NotificationThread> {
        SqlitePersistedState::thread(self, id).await
    }

    async fn persist_thread(
        &self,
        thread: NotificationThread,
        precondition: PersistPrecondition,
    ) -> anyhow::Result<NotificationThread> {
        let record = NotificationThreadRecord::from_thread(thread)?;
        let thread_id = record.id()?;
        let expected_revision = match precondition {
            PersistPrecondition::ExpectedRevision(revision)
            | PersistPrecondition::ExpectedActiveRevision(revision) => revision,
        };
        let expected_status = match precondition {
            PersistPrecondition::ExpectedRevision(_) => None,
            PersistPrecondition::ExpectedActiveRevision(_) => Some(NotificationStatus::Active),
        };
        let result = record
            .bind_update(
                sqlx::query(concat!(
                    "UPDATE notification_threads
            SET ",
                    notification_thread_update_assignments!(),
                    "
            WHERE id = ?1
              AND revision = ?20
              AND (?21 IS NULL OR status = ?21)"
                )),
                expected_revision,
                expected_status,
            )?
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            anyhow::bail!(
                "failed to persist notification thread {}; persistence precondition was not met",
                thread_id
            );
        }

        let thread = self.thread(thread_id).await?;
        self.finish_history_write(thread).await
    }

    async fn active_threads(&self) -> anyhow::Result<Vec<NotificationThread>> {
        Ok(self.snapshot().await?.active)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn create_thread_prunes_inactive_history_after_write() {
        let (_temp, storage) = open_history(1).await;
        let now = now_millis();

        storage
            .create_thread(new_thread("old", NotificationStatus::Dismissed, now))
            .await
            .unwrap();
        storage
            .create_thread(new_thread("new", NotificationStatus::Dismissed, now + 1))
            .await
            .unwrap();

        let snapshot = storage.snapshot().await.unwrap();
        assert_eq!(summaries(&snapshot.history), vec!["new"]);
    }

    #[tokio::test]
    async fn create_thread_prunes_inactive_history_by_age_after_write() {
        let (_temp, storage) = open_history_with_retention(1, 10).await;
        let now = now_millis();
        let old = now - 2 * 24 * 60 * 60 * 1_000;

        storage
            .create_thread(new_thread("old", NotificationStatus::Dismissed, old))
            .await
            .unwrap();
        storage
            .create_thread(new_thread("new", NotificationStatus::Dismissed, now))
            .await
            .unwrap();

        let snapshot = storage.snapshot().await.unwrap();
        assert_eq!(summaries(&snapshot.history), vec!["new"]);
    }

    #[tokio::test]
    async fn persist_thread_prunes_existing_inactive_history_after_write() {
        let (_temp, storage) = open_history(10).await;
        let now = now_millis();
        storage
            .create_thread(new_thread("old", NotificationStatus::Dismissed, now))
            .await
            .unwrap();
        storage
            .create_thread(new_thread("newer", NotificationStatus::Dismissed, now + 1))
            .await
            .unwrap();
        let active = storage
            .create_thread(new_thread("active", NotificationStatus::Active, now + 2))
            .await
            .unwrap();
        let storage = storage.with_history_limit(1);

        let mut revised = active.clone();
        revised.accepted_notification = accepted_notification("revised");
        revised.revision += 1;
        revised.updated_at = now + 3;
        storage
            .persist_thread(
                revised,
                PersistPrecondition::ExpectedRevision(active.revision),
            )
            .await
            .unwrap();

        let snapshot = storage.snapshot().await.unwrap();
        assert_eq!(summaries(&snapshot.history), vec!["revised", "newer"]);
    }

    #[tokio::test]
    async fn persist_delivery_change_prunes_existing_inactive_history_after_write() {
        let (_temp, storage) = open_history(10).await;
        let now = now_millis();
        storage
            .create_thread(new_thread("old", NotificationStatus::Dismissed, now))
            .await
            .unwrap();
        storage
            .create_thread(new_thread("newer", NotificationStatus::Dismissed, now + 1))
            .await
            .unwrap();
        let active = storage
            .create_thread(new_thread("active", NotificationStatus::Active, now + 2))
            .await
            .unwrap();
        let storage = storage.with_history_limit(1);

        let mut silenced = active.clone();
        silenced.delivery_outcome = DeliveryOutcome::Silenced;
        silenced.rule_reference = Some("user:active:*".to_string());
        storage
            .persist_thread(
                silenced,
                PersistPrecondition::ExpectedActiveRevision(active.revision),
            )
            .await
            .unwrap();

        let snapshot = storage.snapshot().await.unwrap();
        assert_eq!(summaries(&snapshot.history), vec!["active", "newer"]);
        assert_eq!(
            snapshot.active[0].delivery_outcome,
            DeliveryOutcome::Silenced
        );
    }

    #[tokio::test]
    async fn persist_transition_returns_updated_thread_before_retention_deletes_it() {
        let (_temp, storage) = open_history(0).await;
        let now = now_millis();
        let active = storage
            .create_thread(new_thread("active", NotificationStatus::Active, now))
            .await
            .unwrap();

        let mut dismissed = active.clone();
        dismissed.status = NotificationStatus::Dismissed;
        dismissed.updated_at = now + 1;
        dismissed.closed_at = Some(now + 1);
        let dismissed = storage
            .persist_thread(
                dismissed,
                PersistPrecondition::ExpectedActiveRevision(active.revision),
            )
            .await
            .unwrap();

        assert_eq!(dismissed.status, NotificationStatus::Dismissed);
        assert!(storage.snapshot().await.unwrap().history.is_empty());
    }

    #[tokio::test]
    async fn persist_stale_threads_prunes_after_returning_stale_threads() {
        let (_temp, storage) = open_history(1).await;
        let now = now_millis();
        storage
            .create_thread(new_thread("old active", NotificationStatus::Active, now))
            .await
            .unwrap();
        storage
            .create_thread(new_thread(
                "new active",
                NotificationStatus::Active,
                now + 1,
            ))
            .await
            .unwrap();

        let mut stale = Vec::new();
        for thread in storage.active_threads().await.unwrap() {
            let mut stale_thread = thread.clone();
            stale_thread.status = NotificationStatus::Stale;
            stale_thread.updated_at = now + 2;
            stale_thread.closed_at = Some(now + 2);
            stale.push(
                storage
                    .persist_thread(
                        stale_thread,
                        PersistPrecondition::ExpectedActiveRevision(thread.revision),
                    )
                    .await
                    .unwrap(),
            );
        }

        assert_eq!(stale.len(), 2);
        assert!(
            stale
                .iter()
                .all(|thread| thread.status == NotificationStatus::Stale)
        );
        let snapshot = storage.snapshot().await.unwrap();
        assert_eq!(summaries(&snapshot.history), vec!["new active"]);
    }

    #[tokio::test]
    async fn persist_thread_rejects_stale_revision() {
        let (_temp, storage) = open_history(10).await;
        let now = now_millis();
        let active = storage
            .create_thread(new_thread("active", NotificationStatus::Active, now))
            .await
            .unwrap();
        let mut changed = active.clone();
        changed.accepted_notification = accepted_notification("changed");

        let error = storage
            .persist_thread(
                changed,
                PersistPrecondition::ExpectedRevision(active.revision + 1),
            )
            .await
            .unwrap_err();

        assert!(error.to_string().contains("precondition"));
        assert_eq!(
            storage
                .thread(active.id)
                .await
                .unwrap()
                .accepted_notification
                .summary,
            "active"
        );
    }

    async fn open_history(history_limit: i64) -> (tempfile::TempDir, SqlitePersistedState) {
        open_history_with_retention(30, history_limit).await
    }

    async fn open_history_with_retention(
        retention_days: i64,
        history_limit: i64,
    ) -> (tempfile::TempDir, SqlitePersistedState) {
        let temp = tempfile::tempdir().unwrap();
        let storage = SqlitePersistedState::open(
            temp.path().join("history.db"),
            retention_days,
            history_limit,
        )
        .await
        .unwrap();
        (temp, storage)
    }

    impl SqlitePersistedState {
        fn with_history_limit(&self, history_limit: i64) -> Self {
            Self {
                pool: self.pool.clone(),
                retention: RetentionPolicy {
                    days: self.retention.days,
                    history_limit,
                },
            }
        }
    }

    fn new_thread(
        summary: &str,
        status: NotificationStatus,
        updated_at: i64,
    ) -> NewNotificationThread {
        NewNotificationThread {
            accepted_notification: accepted_notification(summary),
            status,
            delivery_outcome: DeliveryOutcome::Shown,
            rule_reference: None,
            created_at: updated_at,
            updated_at,
        }
    }

    fn accepted_notification(summary: &str) -> AcceptedNotification {
        AcceptedNotification {
            app_identity: "App".to_string(),
            app_name: "App".to_string(),
            replaces_id: None,
            summary: summary.to_string(),
            body: String::new(),
            raw_body: String::new(),
            urgency: Urgency::Normal,
            category: None,
            progress: None,
            has_image_payload: false,
            actions: Vec::new(),
            resident: false,
        }
    }

    fn summaries(threads: &[NotificationThread]) -> Vec<&str> {
        threads
            .iter()
            .map(|thread| thread.accepted_notification.summary.as_str())
            .collect()
    }
}
