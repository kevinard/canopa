# Normalize notification content and metadata

Status: implemented
Type: AFK

## Parent

[Canopa Notify PRD](../PRD.md)

## What to build

Normalize sender-provided notification content into safe user-visible metadata. Canopa Notify stores Raw Notification Body for protocol fidelity, displays a conservative plain-text Notification Body, accepts body-only notifications, drops empty no-op notifications, stores Notification Category and Notification Progress metadata, avoids arbitrary image payload persistence, and redacts Notification Content from normal Delivery Decision Logs.

## Acceptance criteria

- [ ] Markup-like notification bodies are converted into readable plain text for display.
- [ ] Raw Notification Body is retained separately from user-visible Notification Body.
- [ ] Body-only notifications are accepted even when Notification Summary is empty.
- [ ] Notifications with no summary, body, action, or meaningful metadata are dropped as no-op notifications.
- [ ] Notification Category and Notification Progress are stored when provided.
- [ ] Arbitrary notification image payloads are not persisted in Notification History by default.
- [ ] Normal Delivery Decision Logs include policy metadata but not Notification Content.
- [ ] Tests cover content normalization, no-op dropping, metadata persistence, image persistence omission, and log redaction.

## Blocked by

- [02-receive-store-default-allowed-notification.md](02-receive-store-default-allowed-notification.md)
