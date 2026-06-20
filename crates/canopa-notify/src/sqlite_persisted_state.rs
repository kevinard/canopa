use crate::{
    DeliveryOutcome, LifecycleDelivery, NewNotificationThread, NotificationAction,
    NotificationHistory, NotificationSnapshot, NotificationStatus, NotificationThread,
    ThreadRevision, Urgency, UserRule, now_millis, urgency_from_key, urgency_key,
};
use anyhow::Context;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteRow};
use sqlx::{Row, SqlitePool};
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

struct NotificationThreadRow {
    id: i64,
    app_identity: String,
    app_name: String,
    summary: String,
    body: String,
    raw_body: String,
    urgency: Urgency,
    status: NotificationStatus,
    delivery_outcome: DeliveryOutcome,
    rule_reference: Option<String>,
    category: Option<String>,
    progress: Option<i64>,
    actions: Vec<NotificationAction>,
    resident: bool,
    revision: i64,
    created_at: i64,
    updated_at: i64,
    closed_at: Option<i64>,
}

impl NotificationThreadRow {
    fn from_row(row: &SqliteRow) -> anyhow::Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            app_identity: row.try_get("app_identity")?,
            app_name: row.try_get("app_name")?,
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
            actions: serde_json::from_str(&row.try_get::<String, _>("actions_json")?)?,
            resident: row.try_get::<i64, _>("resident")? != 0,
            revision: row.try_get("revision")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
            closed_at: row.try_get("closed_at")?,
        })
    }

    fn into_thread(self) -> NotificationThread {
        NotificationThread {
            id: self.id,
            app_identity: self.app_identity,
            app_name: self.app_name,
            summary: self.summary,
            body: self.body,
            raw_body: self.raw_body,
            urgency: self.urgency,
            status: self.status,
            delivery_outcome: self.delivery_outcome,
            rule_reference: self.rule_reference,
            category: self.category,
            progress: self.progress,
            has_image_payload: false,
            actions: self.actions,
            resident: self.resident,
            revision: self.revision,
            created_at: self.created_at,
            updated_at: self.updated_at,
            closed_at: self.closed_at,
        }
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

    pub(crate) async fn snapshot(&self) -> anyhow::Result<NotificationSnapshot> {
        let rows = sqlx::query(
            r#"
            SELECT id, app_identity, app_name, summary, body, raw_body, urgency, status,
                   delivery_outcome, rule_reference, category, progress, resident,
                   actions_json, revision, created_at, updated_at, closed_at
            FROM notification_threads
            ORDER BY created_at DESC, id DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut active = Vec::new();
        let mut history = Vec::new();

        for row in rows {
            let thread = NotificationThreadRow::from_row(&row)?.into_thread();
            if thread.status == NotificationStatus::Active {
                active.push(thread.clone());
            }
            history.push(thread);
        }

        Ok(NotificationSnapshot { active, history })
    }

    pub(crate) async fn thread(&self, id: i64) -> anyhow::Result<NotificationThread> {
        let row = sqlx::query(
            r#"
            SELECT id, app_identity, app_name, summary, body, raw_body, urgency, status,
                   delivery_outcome, rule_reference, category, progress, resident,
                   actions_json, revision, created_at, updated_at, closed_at
            FROM notification_threads
            WHERE id = ?1
            "#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        Ok(NotificationThreadRow::from_row(&row)?.into_thread())
    }

    async fn active_ids(&self) -> anyhow::Result<Vec<i64>> {
        let ids = sqlx::query_scalar(
            "SELECT id FROM notification_threads WHERE status = 'active' ORDER BY created_at ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(ids)
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
}

impl NotificationHistory for SqlitePersistedState {
    async fn create_thread(
        &self,
        thread: NewNotificationThread,
    ) -> anyhow::Result<NotificationThread> {
        let actions_json = serde_json::to_string(&thread.actions)?;
        let id = sqlx::query(
            r#"
            INSERT INTO notification_threads (
                app_identity, app_name, summary, body, raw_body, urgency, status,
                delivery_outcome, rule_reference, category, progress, resident, created_at,
                updated_at, actions_json
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            "#,
        )
        .bind(&thread.app_identity)
        .bind(&thread.app_name)
        .bind(&thread.summary)
        .bind(&thread.body)
        .bind(&thread.raw_body)
        .bind(thread.urgency.as_str())
        .bind(thread.status.as_str())
        .bind(thread.delivery_outcome.as_str())
        .bind(&thread.rule_reference)
        .bind(&thread.category)
        .bind(thread.progress)
        .bind(thread.resident)
        .bind(thread.created_at)
        .bind(thread.updated_at)
        .bind(actions_json)
        .execute(&self.pool)
        .await?
        .last_insert_rowid();
        self.thread(id).await
    }

    async fn thread(&self, id: i64) -> anyhow::Result<NotificationThread> {
        SqlitePersistedState::thread(self, id).await
    }

    async fn revise_thread(
        &self,
        id: i64,
        revision: ThreadRevision,
    ) -> anyhow::Result<NotificationThread> {
        let actions_json = serde_json::to_string(&revision.actions)?;
        sqlx::query(
            r#"
            UPDATE notification_threads
            SET app_identity = ?2,
                app_name = ?3,
                summary = ?4,
                body = ?5,
                raw_body = ?6,
                urgency = ?7,
                category = ?8,
                progress = ?9,
                resident = ?10,
                actions_json = ?11,
                delivery_outcome = ?12,
                rule_reference = ?13,
                status = 'active',
                revision = revision + 1,
                updated_at = ?14,
                closed_at = NULL
            WHERE id = ?1
            "#,
        )
        .bind(id)
        .bind(revision.app_identity)
        .bind(revision.app_name)
        .bind(revision.summary)
        .bind(revision.body)
        .bind(revision.raw_body)
        .bind(revision.urgency.as_str())
        .bind(revision.category)
        .bind(revision.progress)
        .bind(revision.resident)
        .bind(actions_json)
        .bind(revision.delivery_outcome.as_str())
        .bind(revision.rule_reference)
        .bind(revision.updated_at)
        .execute(&self.pool)
        .await?;
        self.thread(id).await
    }

    async fn transition_thread(
        &self,
        id: i64,
        status: NotificationStatus,
        updated_at: i64,
    ) -> anyhow::Result<NotificationThread> {
        sqlx::query(
            r#"
            UPDATE notification_threads
            SET status = ?2, updated_at = ?3, closed_at = ?3
            WHERE id = ?1
            "#,
        )
        .bind(id)
        .bind(status.as_str())
        .bind(updated_at)
        .execute(&self.pool)
        .await?;
        self.prune_history().await?;
        self.thread(id).await
    }

    async fn active_threads(&self) -> anyhow::Result<Vec<NotificationThread>> {
        Ok(self.snapshot().await?.active)
    }

    async fn update_delivery(
        &self,
        id: i64,
        delivery: LifecycleDelivery,
    ) -> anyhow::Result<NotificationThread> {
        sqlx::query(
            r#"
            UPDATE notification_threads
            SET delivery_outcome = ?2,
                rule_reference = ?3
            WHERE id = ?1
            "#,
        )
        .bind(id)
        .bind(delivery.outcome.as_str())
        .bind(delivery.rule_reference)
        .execute(&self.pool)
        .await?;
        self.thread(id).await
    }

    async fn mark_active_threads_stale(
        &self,
        updated_at: i64,
    ) -> anyhow::Result<Vec<NotificationThread>> {
        let ids = self.active_ids().await?;
        sqlx::query(
            r#"
            UPDATE notification_threads
            SET status = 'stale', updated_at = ?1, closed_at = ?1
            WHERE status = 'active'
            "#,
        )
        .bind(updated_at)
        .execute(&self.pool)
        .await?;

        let mut stale = Vec::new();
        for id in ids {
            stale.push(self.thread(id).await?);
        }
        Ok(stale)
    }
}
