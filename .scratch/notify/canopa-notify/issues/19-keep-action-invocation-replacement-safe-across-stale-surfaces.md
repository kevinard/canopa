# Keep action invocation replacement-safe across stale surfaces

Status: implemented

## Parent

.scratch/notify/canopa-notify/PRD.md

## What to build

Ensure **Notification Action** invocation stays tied to the current revision of an active **Notification Thread** even when users interact with stale banner or Notification Center surfaces. Replacing a notification should replace its available action set. If a stale UI tries to invoke an action key that no longer exists on the current revision, Notify should fail strictly and wait for the refreshed projection rather than matching labels or previous revision actions.

## Acceptance criteria

- [ ] Replacement notifications fully replace the available **Notification Action** set for the retained **Notification Thread**.
- [ ] Stale action keys from previous revisions cannot be invoked after replacement.
- [ ] Action labels are never used as fallback invocation identity.
- [ ] Updated banners and Notification Center projections expose only the current revision's active action controls.
- [ ] A stale action invocation through the private control path returns an error while user-facing panel behavior stays quiet apart from logging.
- [ ] Tests cover replacement removing an action, replacement changing labels, and stale surface invocation failure.

## Blocked by

- .scratch/notify/canopa-notify/issues/16-publish-correct-sender-lifecycle-for-invoked-actions.md
- .scratch/notify/canopa-notify/issues/18-invoke-active-notification-actions-from-notification-center.md
