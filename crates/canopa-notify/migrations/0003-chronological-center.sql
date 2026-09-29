DROP INDEX notification_threads_center;
CREATE INDEX notification_threads_center ON notification_threads(created_at DESC, id DESC);
PRAGMA user_version = 3;
