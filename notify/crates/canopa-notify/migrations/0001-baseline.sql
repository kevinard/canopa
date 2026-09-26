-- Legacy column normalization and body-content backfill run in the same transaction
-- before this baseline is recorded. Later migrations must be forward-only.
CREATE INDEX IF NOT EXISTS notification_threads_status_created
    ON notification_threads(status, created_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS notification_threads_retention
    ON notification_threads(status, updated_at DESC, id DESC);
PRAGMA user_version = 1;
