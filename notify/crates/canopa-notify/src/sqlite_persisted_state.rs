use crate::InternalNotificationId;
use crate::clock::now_millis;
use crate::domain::{urgency_from_key, urgency_key};
use crate::intake::interpreted_body_content;
use crate::{
    AcceptedNotification, DeliveryOutcome, NewNotificationThread, NotificationBodyContent,
    NotificationHistory, NotificationSnapshot, NotificationStatus, NotificationThread,
    PersistPrecondition, Urgency, UserRule, normalize_notification_category,
};
use sqlx::query::Query;
use sqlx::sqlite::{SqliteArguments, SqliteConnectOptions, SqlitePoolOptions, SqliteRow};
use sqlx::{Row, Sqlite, SqliteConnection, SqlitePool};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::PathBuf;
#[cfg(test)]
use std::str::FromStr;

const CURRENT_SCHEMA_VERSION: i64 = 2;

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

const NOTIFICATION_THREAD_ID_COLUMN: &str = "id";
const NOTIFICATION_THREAD_WRITE_COLUMNS: [&str; 23] = [
    "app_identity",
    "app_name",
    "replaces_id",
    "app_icon_json",
    "summary",
    "body",
    "body_content_json",
    "raw_body",
    "urgency",
    "status",
    "delivery_outcome",
    "rule_reference",
    "category",
    "progress",
    "image_reference_json",
    "resident",
    "actions_json",
    "revision",
    "created_at",
    "updated_at",
    "closed_at",
    "requires_attention",
    "retained_bytes",
];

fn notification_thread_columns() -> String {
    std::iter::once(NOTIFICATION_THREAD_ID_COLUMN)
        .chain(NOTIFICATION_THREAD_WRITE_COLUMNS)
        .collect::<Vec<_>>()
        .join(", ")
}

fn notification_thread_write_columns() -> String {
    NOTIFICATION_THREAD_WRITE_COLUMNS.join(", ")
}

fn notification_thread_placeholders(start: usize, count: usize) -> String {
    (start..start + count)
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn notification_thread_update_assignments() -> String {
    NOTIFICATION_THREAD_WRITE_COLUMNS
        .iter()
        .enumerate()
        .map(|(index, column)| format!("{column} = ?{}", index + 2))
        .collect::<Vec<_>>()
        .join(",\n                ")
}

fn notification_thread_insert_sql() -> String {
    format!(
        "INSERT INTO notification_threads ({}) VALUES ({})",
        notification_thread_write_columns(),
        notification_thread_placeholders(1, NOTIFICATION_THREAD_WRITE_COLUMNS.len())
    )
}

fn notification_thread_select_sql(tail: &str) -> String {
    format!(
        "SELECT {} FROM notification_threads {tail}",
        notification_thread_columns()
    )
}

fn notification_thread_update_sql() -> String {
    let expected_revision_placeholder = NOTIFICATION_THREAD_WRITE_COLUMNS.len() + 2;
    let expected_status_placeholder = NOTIFICATION_THREAD_WRITE_COLUMNS.len() + 3;
    format!(
        "UPDATE notification_threads
         SET {}
         WHERE id = ?1
           AND revision = ?{expected_revision_placeholder}
           AND (?{expected_status_placeholder} IS NULL OR status = ?{expected_status_placeholder})",
        notification_thread_update_assignments()
    )
}

struct NotificationThreadRecord {
    id: Option<i64>,
    app_identity: String,
    app_name: String,
    replaces_id: Option<i64>,
    app_icon_json: Option<String>,
    summary: String,
    body: String,
    body_content_json: Option<String>,
    raw_body: String,
    urgency: Urgency,
    status: NotificationStatus,
    requires_attention: bool,
    delivery_outcome: DeliveryOutcome,
    rule_reference: Option<String>,
    category: Option<String>,
    progress: Option<i64>,
    image_reference_json: Option<String>,
    actions_json: String,
    resident: bool,
    revision: i64,
    created_at: i64,
    updated_at: i64,
    closed_at: Option<i64>,
}

impl NotificationThreadRecord {
    fn from_new_thread(thread: NewNotificationThread) -> anyhow::Result<Self> {
        Self::from_thread_parts(
            None,
            thread.accepted_notification,
            thread.status,
            thread.requires_attention,
            thread.delivery_outcome,
            thread.rule_reference,
            1,
            thread.created_at,
            thread.updated_at,
            None,
        )
    }

    fn from_thread(thread: NotificationThread) -> anyhow::Result<Self> {
        Self::from_thread_parts(
            Some(thread.id.get()),
            thread.accepted_notification,
            thread.status,
            thread.requires_attention,
            thread.delivery_outcome,
            thread.rule_reference,
            thread.revision,
            thread.created_at,
            thread.updated_at,
            thread.closed_at,
        )
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "Private serialization boundary mirrors the persisted thread columns"
    )]
    fn from_thread_parts(
        id: Option<i64>,
        notification: AcceptedNotification,
        status: NotificationStatus,
        requires_attention: bool,
        delivery_outcome: DeliveryOutcome,
        rule_reference: Option<String>,
        revision: i64,
        created_at: i64,
        updated_at: i64,
        closed_at: Option<i64>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            id,
            app_identity: notification.app_identity,
            app_name: notification.app_name,
            replaces_id: notification.replaces_id.map(InternalNotificationId::get),
            app_icon_json: notification
                .app_icon
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?,
            summary: notification.summary,
            body: notification.body,
            body_content_json: Some(serde_json::to_string(&notification.body_content)?),
            raw_body: notification.raw_body,
            urgency: notification.urgency,
            status,
            requires_attention,
            delivery_outcome,
            rule_reference,
            category: notification.category,
            progress: notification.progress,
            image_reference_json: notification
                .image_reference
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?,
            actions_json: serde_json::to_string(&notification.actions)?,
            resident: notification.resident,
            revision,
            created_at,
            updated_at,
            closed_at,
        })
    }

    fn from_row(row: &SqliteRow) -> anyhow::Result<Self> {
        Ok(Self {
            id: Some(row.try_get(NOTIFICATION_THREAD_ID_COLUMN)?),
            app_identity: row.try_get("app_identity")?,
            app_name: row.try_get("app_name")?,
            replaces_id: row.try_get("replaces_id")?,
            app_icon_json: row.try_get("app_icon_json")?,
            summary: row.try_get("summary")?,
            body: row.try_get("body")?,
            body_content_json: row.try_get("body_content_json")?,
            raw_body: row.try_get("raw_body")?,
            urgency: Urgency::from_persisted_str(&row.try_get::<String, _>("urgency")?),
            status: NotificationStatus::from_persisted_str(&row.try_get::<String, _>("status")?),
            requires_attention: row.try_get("requires_attention")?,
            delivery_outcome: DeliveryOutcome::from_persisted_str(
                &row.try_get::<String, _>("delivery_outcome")?,
            ),
            rule_reference: row.try_get("rule_reference")?,
            category: row.try_get("category")?,
            progress: row.try_get("progress")?,
            image_reference_json: row.try_get("image_reference_json")?,
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
            .bind(self.app_icon_json.as_deref())
            .bind(&self.summary)
            .bind(&self.body)
            .bind(self.body_content_json.as_deref())
            .bind(&self.raw_body)
            .bind(self.urgency.as_str())
            .bind(self.status.as_str())
            .bind(self.delivery_outcome.as_str())
            .bind(self.rule_reference.as_deref())
            .bind(self.category.as_deref())
            .bind(self.progress)
            .bind(self.image_reference_json.as_deref())
            .bind(self.resident)
            .bind(&self.actions_json)
            .bind(self.revision)
            .bind(self.created_at)
            .bind(self.updated_at)
            .bind(self.closed_at)
            .bind(self.requires_attention)
            .bind(self.storage_bytes())
    }

    fn storage_bytes(&self) -> i64 {
        [
            &self.app_identity,
            &self.app_name,
            &self.summary,
            &self.body,
            &self.raw_body,
            &self.actions_json,
        ]
        .into_iter()
        .map(|value| value.len() as i64)
        .sum::<i64>()
            + [
                &self.app_icon_json,
                &self.body_content_json,
                &self.rule_reference,
                &self.category,
                &self.image_reference_json,
            ]
            .into_iter()
            .map(|value| value.as_ref().map_or(0, |value| value.len() as i64))
            .sum::<i64>()
    }

    fn into_thread(self) -> anyhow::Result<NotificationThread> {
        let actions = serde_json::from_str(&self.actions_json)?;
        let app_icon = self
            .app_icon_json
            .as_deref()
            .map(serde_json::from_str)
            .transpose()?;
        let image_reference = self
            .image_reference_json
            .as_deref()
            .map(serde_json::from_str)
            .transpose()?;
        let body_content = self
            .body_content_json
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(serde_json::from_str)
            .transpose()?
            .unwrap_or_else(|| NotificationBodyContent::plain(self.body.clone()));
        Ok(NotificationThread {
            id: InternalNotificationId::try_from(self.id()?)?,
            accepted_notification: AcceptedNotification {
                app_identity: self.app_identity,
                app_name: self.app_name,
                // Legacy library inputs could retain Some(0), meaning no replacement.
                replaces_id: self.replaces_id.and_then(InternalNotificationId::new),
                app_icon,
                summary: self.summary,
                body: self.body,
                body_content,
                raw_body: self.raw_body,
                urgency: self.urgency,
                category: self
                    .category
                    .as_deref()
                    .and_then(normalize_notification_category),
                progress: self.progress,
                image_reference,
                has_image_payload: false,
                actions,
                resident: self.resident,
                transient: false,
            },
            status: self.status,
            requires_attention: self.requires_attention,
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
    category: Option<String>,
    urgency: Option<Urgency>,
    outcome: DeliveryOutcome,
    reference: String,
}

impl UserRuleRow {
    fn from_row(row: SqliteRow) -> anyhow::Result<Self> {
        Ok(Self {
            app_identity: row.try_get("app_identity")?,
            category: category_from_key(&row.try_get::<String, _>("category")?),
            urgency: urgency_from_key(&row.try_get::<String, _>("urgency")?),
            outcome: DeliveryOutcome::from_persisted_str(&row.try_get::<String, _>("outcome")?),
            reference: row.try_get("reference")?,
        })
    }

    fn into_user_rule(self) -> UserRule {
        UserRule {
            app_identity: self.app_identity,
            category: self.category,
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

        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
            .open(&path)?;
        let metadata = file.metadata()?;
        anyhow::ensure!(
            metadata.is_file() && metadata.uid() == rustix::process::geteuid().as_raw(),
            "notification database must be an owned regular file"
        );
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        let options = SqliteConnectOptions::new()
            .filename(&path)
            .busy_timeout(std::time::Duration::from_secs(5));

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;

        let mut transaction = pool.begin().await?;
        let version: i64 = sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(&mut *transaction)
            .await?;
        anyhow::ensure!(
            version <= CURRENT_SCHEMA_VERSION,
            "notification database schema {version} is newer than supported schema {CURRENT_SCHEMA_VERSION}"
        );
        if version == 0 {
            // Version 1 adopts every supported pre-release schema atomically.
            Self::create_schema(&mut transaction).await?;
            Self::migrate_schema(&mut transaction).await?;
            sqlx::raw_sql(include_str!("../migrations/0001-baseline.sql"))
                .execute(&mut *transaction)
                .await?;
        }
        if version < 2 {
            sqlx::raw_sql(include_str!("../migrations/0002-retained-attention.sql"))
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;

        Ok(Self {
            pool,
            retention: RetentionPolicy {
                days: retention_days,
                history_limit,
            },
        })
    }

    async fn create_schema(connection: &mut SqliteConnection) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS daemon_metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )
            "#,
        )
        .execute(&mut *connection)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS user_rules (
                app_identity TEXT NOT NULL,
                category TEXT NOT NULL DEFAULT '',
                urgency TEXT NOT NULL,
                outcome TEXT NOT NULL,
                reference TEXT NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (app_identity, category, urgency)
            )
            "#,
        )
        .execute(&mut *connection)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS notification_threads (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                app_identity TEXT NOT NULL,
                app_name TEXT NOT NULL,
                replaces_id INTEGER,
                app_icon_json TEXT,
                summary TEXT NOT NULL,
                body TEXT NOT NULL,
                body_content_json TEXT,
                raw_body TEXT NOT NULL,
                urgency TEXT NOT NULL,
                status TEXT NOT NULL,
                delivery_outcome TEXT NOT NULL,
                rule_reference TEXT,
                category TEXT,
                progress INTEGER,
                image_reference_json TEXT,
                resident INTEGER NOT NULL DEFAULT 0,
                actions_json TEXT NOT NULL DEFAULT '[]',
                revision INTEGER NOT NULL DEFAULT 1,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                closed_at INTEGER
            )
            "#,
        )
        .execute(&mut *connection)
        .await?;

        Ok(())
    }

    async fn migrate_schema(connection: &mut SqliteConnection) -> anyhow::Result<()> {
        Self::migrate_user_rule_categories(&mut *connection).await?;
        Self::add_nullable_column_if_missing(&mut *connection, "replaces_id", "INTEGER").await?;
        Self::add_nullable_column_if_missing(&mut *connection, "app_icon_json", "TEXT").await?;
        Self::add_nullable_column_if_missing(&mut *connection, "image_reference_json", "TEXT")
            .await?;
        Self::add_nullable_column_if_missing(&mut *connection, "body_content_json", "TEXT").await?;
        Self::backfill_body_content(&mut *connection).await?;

        Ok(())
    }

    async fn migrate_user_rule_categories(connection: &mut SqliteConnection) -> anyhow::Result<()> {
        let category_column: Option<String> = sqlx::query_scalar(
            "SELECT name FROM pragma_table_info('user_rules') WHERE name = 'category'",
        )
        .fetch_optional(&mut *connection)
        .await?;
        if category_column.is_some() {
            return Ok(());
        }

        sqlx::query("ALTER TABLE user_rules RENAME TO legacy_user_rules")
            .execute(&mut *connection)
            .await?;
        sqlx::query(
            r#"
            CREATE TABLE user_rules (
                app_identity TEXT NOT NULL,
                category TEXT NOT NULL DEFAULT '',
                urgency TEXT NOT NULL,
                outcome TEXT NOT NULL,
                reference TEXT NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (app_identity, category, urgency)
            )
            "#,
        )
        .execute(&mut *connection)
        .await?;
        sqlx::query(
            r#"
            INSERT INTO user_rules (
                app_identity, category, urgency, outcome, reference, updated_at
            )
            SELECT app_identity, '', urgency, outcome, reference, updated_at
            FROM legacy_user_rules
            "#,
        )
        .execute(&mut *connection)
        .await?;
        sqlx::query("DROP TABLE legacy_user_rules")
            .execute(&mut *connection)
            .await?;
        Ok(())
    }

    async fn backfill_body_content(connection: &mut SqliteConnection) -> anyhow::Result<()> {
        let rows = sqlx::query(
            "SELECT id, raw_body FROM notification_threads WHERE body_content_json IS NULL OR body_content_json = ''",
        )
        .fetch_all(&mut *connection)
        .await?;

        for row in rows {
            let id: i64 = row.try_get("id")?;
            let raw_body: String = row.try_get("raw_body")?;
            let body_content_json = serde_json::to_string(&interpreted_body_content(&raw_body))?;
            sqlx::query("UPDATE notification_threads SET body_content_json = ?1 WHERE id = ?2")
                .bind(body_content_json)
                .bind(id)
                .execute(&mut *connection)
                .await?;
        }

        Ok(())
    }

    async fn add_nullable_column_if_missing(
        connection: &mut SqliteConnection,
        column: &str,
        sql_type: &str,
    ) -> anyhow::Result<()> {
        let existing: Option<String> = sqlx::query_scalar(
            "SELECT name FROM pragma_table_info('notification_threads') WHERE name = ?1",
        )
        .bind(column)
        .fetch_optional(&mut *connection)
        .await?;

        if existing.is_none() {
            sqlx::query(&format!(
                "ALTER TABLE notification_threads ADD COLUMN {column} {sql_type}"
            ))
            .execute(&mut *connection)
            .await?;
        }

        Ok(())
    }

    pub(crate) async fn snapshot(&self) -> anyhow::Result<NotificationSnapshot> {
        let sql = notification_thread_select_sql("ORDER BY created_at DESC, id DESC");
        let rows = sqlx::query(&sql).fetch_all(&self.pool).await?;

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

    pub(crate) async fn thread(
        &self,
        id: InternalNotificationId,
    ) -> anyhow::Result<NotificationThread> {
        let sql = notification_thread_select_sql("WHERE id = ?1");
        let row = sqlx::query(&sql)
            .bind(id.get())
            .fetch_one(&self.pool)
            .await?;

        NotificationThreadRecord::from_row(&row)?.into_thread()
    }

    pub(crate) async fn center_threads_after(
        &self,
        after: Option<crate::NotificationCenterCursor>,
        limit: u8,
    ) -> anyhow::Result<Vec<NotificationThread>> {
        let sql = notification_thread_select_sql(
            "WHERE ?1 IS NULL OR (requires_attention, created_at, id) < (?1, ?2, ?3)
             ORDER BY requires_attention DESC, created_at DESC, id DESC LIMIT ?4",
        );
        sqlx::query(&sql)
            .bind(after.map(|cursor| i64::from(cursor.requires_attention)))
            .bind(after.map(|cursor| cursor.created_at))
            .bind(after.map(|cursor| cursor.id.get()))
            .bind(i64::from(limit))
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(|row| NotificationThreadRecord::from_row(row)?.into_thread())
            .collect()
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
        rule.validate()?;
        sqlx::query(
            r#"
            INSERT INTO user_rules (app_identity, category, urgency, outcome, reference, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(app_identity, category, urgency) DO UPDATE SET
                outcome = excluded.outcome,
                reference = excluded.reference,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&rule.app_identity)
        .bind(category_key(rule.category.as_deref()))
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
            "SELECT app_identity, category, urgency, outcome, reference FROM user_rules ORDER BY app_identity, category, urgency",
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
        category: Option<&str>,
        urgency: Option<Urgency>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "DELETE FROM user_rules WHERE app_identity = ?1 AND category = ?2 AND urgency = ?3",
        )
        .bind(app_identity)
        .bind(category_key(category))
        .bind(urgency_key(urgency))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(crate) async fn prune_history(&self) -> anyhow::Result<()> {
        let mut transaction = self.pool.begin().await?;
        self.prune_history_in(&mut transaction).await?;
        transaction.commit().await?;
        Ok(())
    }

    async fn prune_history_in(&self, connection: &mut SqliteConnection) -> anyhow::Result<()> {
        let cutoff =
            now_millis().saturating_sub(self.retention.days.max(0).saturating_mul(86_400_000));

        sqlx::query(
            "DELETE FROM notification_threads WHERE status != 'active' AND requires_attention = 0 AND updated_at < ?1",
        )
        .bind(cutoff)
        .execute(&mut *connection)
        .await?;

        sqlx::query(
            r#"
            DELETE FROM notification_threads
            WHERE status != 'active' AND requires_attention = 0
              AND id IN (
                  SELECT id
                  FROM notification_threads
                  WHERE status != 'active' AND requires_attention = 0
                  ORDER BY updated_at DESC, id DESC
                  LIMIT -1 OFFSET ?1
              )
            "#,
        )
        .bind(self.retention.history_limit.max(0))
        .execute(&mut *connection)
        .await?;

        Ok(())
    }
}

// Pending content is never evicted to make room. Refuse new work atomically when
// count or logical stored UTF-8 reaches the fixed budget; recovery remains possible.
async fn check_attention_budget(connection: &mut SqliteConnection) -> anyhow::Result<()> {
    let (count, bytes): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(retained_bytes), 0) FROM notification_threads WHERE requires_attention = 1")
        .fetch_one(connection).await?;
    if count > 4096 {
        return Err(crate::NotificationLimitExceeded("retained count").into());
    }
    if bytes > 64 * 1024 * 1024 {
        return Err(crate::NotificationLimitExceeded("retained text").into());
    }
    Ok(())
}

fn category_key(category: Option<&str>) -> &str {
    category.unwrap_or("")
}

fn category_from_key(category: &str) -> Option<String> {
    (!category.is_empty()).then(|| category.to_string())
}

impl NotificationHistory for SqlitePersistedState {
    async fn create_thread(
        &self,
        thread: NewNotificationThread,
    ) -> anyhow::Result<NotificationThread> {
        if thread.accepted_notification.transient {
            anyhow::bail!("transient notifications cannot be written to persisted history");
        }
        let record = NotificationThreadRecord::from_new_thread(thread)?;
        let sql = format!(
            "{} RETURNING {}",
            notification_thread_insert_sql(),
            notification_thread_columns()
        );
        let mut transaction = self.pool.begin().await?;
        let row = record
            .bind_insert(sqlx::query(&sql))
            .fetch_one(&mut *transaction)
            .await?;
        let thread = NotificationThreadRecord::from_row(&row)?.into_thread()?;
        if thread.status == NotificationStatus::Active && thread.requires_attention {
            check_attention_budget(&mut transaction).await?;
        }
        self.prune_history_in(&mut transaction).await?;
        transaction.commit().await?;
        Ok(thread)
    }

    async fn thread(&self, id: InternalNotificationId) -> anyhow::Result<NotificationThread> {
        SqlitePersistedState::thread(self, id).await
    }

    async fn persist_thread(
        &self,
        thread: NotificationThread,
        precondition: PersistPrecondition,
    ) -> anyhow::Result<NotificationThread> {
        if thread.accepted_notification.transient {
            anyhow::bail!("transient notifications cannot be written to persisted history");
        }
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
        let sql = format!(
            "{} RETURNING {}",
            notification_thread_update_sql(),
            notification_thread_columns()
        );
        let mut transaction = self.pool.begin().await?;
        let row = record
            .bind_update(sqlx::query(&sql), expected_revision, expected_status)?
            .fetch_optional(&mut *transaction)
            .await?;

        let Some(row) = row else {
            anyhow::bail!(
                "failed to persist notification thread {}; persistence precondition was not met",
                thread_id
            );
        };

        let thread = NotificationThreadRecord::from_row(&row)?.into_thread()?;
        if thread.status == NotificationStatus::Active && thread.requires_attention {
            check_attention_budget(&mut transaction).await?;
        }
        self.prune_history_in(&mut transaction).await?;
        transaction.commit().await?;
        Ok(thread)
    }

    async fn active_threads(&self) -> anyhow::Result<Vec<NotificationThread>> {
        let sql = notification_thread_select_sql(
            "WHERE status = 'active' ORDER BY created_at DESC, id DESC",
        );
        sqlx::query(&sql)
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(|row| NotificationThreadRecord::from_row(row)?.into_thread())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NotificationBodyImage, NotificationBodySegment, NotificationBodyStyle};

    #[tokio::test]
    async fn persisted_history_rejects_transient_threads() {
        let (_temp, storage) = open_history(10).await;
        let mut thread = new_thread("runtime only", NotificationStatus::Active, now_millis());
        thread.accepted_notification.transient = true;

        assert!(storage.create_thread(thread).await.is_err());
        assert!(storage.snapshot().await.unwrap().history.is_empty());
    }

    #[tokio::test]
    async fn user_rule_schema_migrates_app_rules_and_keys_category_rules_separately() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("history.db");
        let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", database.display()))
            .unwrap()
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        sqlx::query(
            r#"
            CREATE TABLE user_rules (
                app_identity TEXT NOT NULL,
                urgency TEXT NOT NULL,
                outcome TEXT NOT NULL,
                reference TEXT NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (app_identity, urgency)
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO user_rules VALUES ('Chat', '*', 'silenced', 'user:Chat:*', 1)")
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;

        let storage = SqlitePersistedState::open(database, 30, 1_000)
            .await
            .unwrap();
        assert_eq!(storage.user_rules().await.unwrap()[0].category, None);
        storage
            .upsert_user_rule(
                &UserRule::for_app_category("Chat", "im.received", DeliveryOutcome::Dropped)
                    .unwrap(),
            )
            .await
            .unwrap();
        storage
            .upsert_user_rule(
                &UserRule::for_app_category("Chat", "transfer.complete", DeliveryOutcome::Shown)
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(storage.user_rules().await.unwrap().len(), 3);
        storage
            .remove_user_rule("Chat", Some("im.received"), None)
            .await
            .unwrap();
        assert_eq!(storage.user_rules().await.unwrap().len(), 2);
    }

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
    async fn retention_failure_rolls_back_the_notification_write() {
        let (_temp, storage) = open_history(10).await;
        storage
            .create_thread(new_thread(
                "Retained",
                NotificationStatus::Dismissed,
                now_millis(),
            ))
            .await
            .unwrap();
        sqlx::query("CREATE TRIGGER fail_retention BEFORE DELETE ON notification_threads BEGIN SELECT RAISE(ABORT, 'injected retention failure'); END")
            .execute(&storage.pool).await.unwrap();
        let storage = storage.with_history_limit(0);
        assert!(
            storage
                .create_thread(new_thread(
                    "Must not commit",
                    NotificationStatus::Active,
                    now_millis()
                ))
                .await
                .is_err()
        );
        assert!(
            storage.active_threads().await.unwrap().is_empty(),
            "write committed despite a failed result"
        );
        assert_eq!(
            summaries(&storage.snapshot().await.unwrap().history),
            vec!["Retained"]
        );
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
    async fn open_backfills_body_content_for_legacy_rows() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("history.db");
        let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", database.display()))
            .unwrap()
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        sqlx::query(
            r#"
            CREATE TABLE notification_threads (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                app_identity TEXT NOT NULL,
                app_name TEXT NOT NULL,
                replaces_id INTEGER,
                app_icon_json TEXT,
                summary TEXT NOT NULL,
                body TEXT NOT NULL,
                raw_body TEXT NOT NULL,
                urgency TEXT NOT NULL,
                status TEXT NOT NULL,
                delivery_outcome TEXT NOT NULL,
                rule_reference TEXT,
                category TEXT,
                progress INTEGER,
                image_reference_json TEXT,
                resident INTEGER NOT NULL DEFAULT 0,
                actions_json TEXT NOT NULL DEFAULT '[]',
                revision INTEGER NOT NULL DEFAULT 1,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                closed_at INTEGER
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"
            INSERT INTO notification_threads (
                app_identity,
                app_name,
                summary,
                body,
                raw_body,
                urgency,
                status,
                delivery_outcome,
                revision,
                created_at,
                updated_at
            )
            VALUES ('Legacy', 'Legacy', 'Inbox', 'One chart', '<b>One</b> <img src="/tmp/legacy-chart.png" alt="chart"/>', 'normal', 'active', 'shown', 1, 1, 1)
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();
        drop(pool);

        let storage = SqlitePersistedState::open(database, 30, 1_000)
            .await
            .unwrap();
        let snapshot = storage.snapshot().await.unwrap();
        let body_content = &snapshot.active[0].accepted_notification.body_content;

        assert_eq!(body_content.plain_text(), "One chart");
        assert_eq!(
            body_content,
            &NotificationBodyContent::from_segments(vec![
                NotificationBodySegment::styled_text(
                    "One",
                    NotificationBodyStyle {
                        bold: true,
                        ..NotificationBodyStyle::default()
                    },
                ),
                NotificationBodySegment::plain_text(" "),
                NotificationBodySegment::body_image(NotificationBodyImage::local_path(
                    "/tmp/legacy-chart.png",
                    Some("chart".to_string()),
                )),
            ])
        );
    }

    #[tokio::test]
    async fn notification_history_persists_body_image_references_across_reopen() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("history.db");
        let storage = SqlitePersistedState::open(database.clone(), 30, 1_000)
            .await
            .unwrap();
        let body_content = NotificationBodyContent::from_segments(vec![
            NotificationBodySegment::plain_text("Chart "),
            NotificationBodySegment::body_image(NotificationBodyImage::local_path(
                "/tmp/canopa-chart.png",
                Some("preview".to_string()),
            )),
        ]);
        let mut notification = accepted_notification("Image");
        notification.body = body_content.plain_text();
        notification.body_content = body_content.clone();
        notification.raw_body =
            r#"Chart <img src="/tmp/canopa-chart.png" alt="preview"/>"#.to_string();

        storage
            .create_thread(NewNotificationThread {
                requires_attention: true,
                accepted_notification: notification,
                status: NotificationStatus::Active,
                delivery_outcome: DeliveryOutcome::Shown,
                rule_reference: None,
                created_at: 1,
                updated_at: 1,
            })
            .await
            .unwrap();

        let stored_json: String =
            sqlx::query_scalar("SELECT body_content_json FROM notification_threads")
                .fetch_one(&storage.pool)
                .await
                .unwrap();
        assert_eq!(stored_json, serde_json::to_string(&body_content).unwrap());
        assert!(stored_json.contains("/tmp/canopa-chart.png"));
        assert!(!stored_json.contains("rgba"));
        storage.pool.close().await;

        let reopened = SqlitePersistedState::open(database, 30, 1_000)
            .await
            .unwrap();
        let snapshot = reopened.snapshot().await.unwrap();

        assert_eq!(
            snapshot.active[0].accepted_notification.body,
            "Chart preview"
        );
        assert_eq!(
            snapshot.active[0].accepted_notification.body_content,
            body_content
        );
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
        dismissed.requires_attention = false;
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
    async fn persist_stale_threads_preserves_pending_attention() {
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
        assert_eq!(
            summaries(&snapshot.history),
            vec!["new active", "old active"]
        );
        assert!(
            snapshot
                .history
                .iter()
                .all(|thread| thread.requires_attention)
        );
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
            requires_attention: status == NotificationStatus::Active,
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
            app_icon: None,
            summary: summary.to_string(),
            body: String::new(),
            body_content: NotificationBodyContent::default(),
            raw_body: String::new(),
            urgency: Urgency::Normal,
            category: None,
            progress: None,
            image_reference: None,
            has_image_payload: false,
            actions: Vec::new(),
            resident: false,
            transient: false,
        }
    }

    fn summaries(threads: &[NotificationThread]) -> Vec<&str> {
        threads
            .iter()
            .map(|thread| thread.accepted_notification.summary.as_str())
            .collect()
    }
}
