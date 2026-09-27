-- Legacy inactive rows keep their archive meaning. Current live content becomes pending.
ALTER TABLE notification_threads ADD COLUMN requires_attention INTEGER NOT NULL DEFAULT 0 CHECK (requires_attention IN (0, 1));
ALTER TABLE notification_threads ADD COLUMN retained_bytes INTEGER NOT NULL DEFAULT 0;
UPDATE notification_threads SET requires_attention = (status = 'active'), retained_bytes =
    length(CAST(app_identity AS BLOB)) + length(CAST(app_name AS BLOB)) +
    length(CAST(summary AS BLOB)) + length(CAST(body AS BLOB)) + length(CAST(raw_body AS BLOB)) +
    length(CAST(actions_json AS BLOB)) + length(CAST(COALESCE(app_icon_json, '') AS BLOB)) +
    length(CAST(COALESCE(body_content_json, '') AS BLOB)) + length(CAST(COALESCE(rule_reference, '') AS BLOB)) +
    length(CAST(COALESCE(category, '') AS BLOB)) + length(CAST(COALESCE(image_reference_json, '') AS BLOB));
CREATE INDEX notification_threads_attention ON notification_threads(requires_attention, updated_at DESC, id DESC);
CREATE INDEX notification_threads_center ON notification_threads(requires_attention DESC, created_at DESC, id DESC);
PRAGMA user_version = 2;
