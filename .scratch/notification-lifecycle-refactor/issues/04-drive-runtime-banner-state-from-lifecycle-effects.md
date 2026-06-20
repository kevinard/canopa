# Drive runtime banner state from Lifecycle Effects

Status: ready-for-agent
Type: AFK

## Parent

[Notification Lifecycle Refactor](../PRD.md)

## What to build

Introduce **Notification Banner State** as runtime-only state that consumes all Lifecycle Effects and decides banner eligibility itself. Banner visibility should no longer be inferred from persisted Notification History flags, and Notification Lifecycle must not emit banner-specific effects.

## Acceptance criteria

- [ ] Notification Banner State consumes Lifecycle Effects for create, replacement, dismissal, expiration, close, stale, silence, and drop outcomes.
- [ ] Banner eligibility still enforces max-three visibility, critical-first preference, newest eligible preference, and no late promotion.
- [ ] Silenced and dropped lifecycle outcomes hide or avoid banners through runtime banner state.
- [ ] Presentation updates are derived from runtime banner state, not persisted history fields.
- [ ] Banner tests cover new notification, replacement, dismissal, silencing, dropping, cap, critical displacement, and no late promotion.

## Blocked by

- [02-handle-replacements-in-notification-lifecycle.md](02-handle-replacements-in-notification-lifecycle.md)
- [03-move-active-transitions-to-lifecycle-effects.md](03-move-active-transitions-to-lifecycle-effects.md)

