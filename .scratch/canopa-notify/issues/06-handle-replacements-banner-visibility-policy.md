# Handle replacements and banner visibility policy

Status: implemented
Type: AFK

## Parent

[Canopa Notify PRD](../PRD.md)

## What to build

Implement replacement-aware Notification Threads and first-version banner visibility policy. Replacement notifications revise an existing thread instead of creating normal history duplicates, eligible replacements can create or update banners, progress metadata can update a visible banner, and banner visibility is capped at three with critical-first/newest-next ordering and no delayed promotion.

## Acceptance criteria

- [ ] A replacement notification revises the existing Notification Thread shown in normal history.
- [ ] An eligible replacement can create a Notification Banner even when the previous revision did not.
- [ ] Progress metadata updates are reflected in daemon-owned banner presentation when present.
- [ ] At most three Notification Banners are visible by default.
- [ ] Visible banner selection prefers critical Active Notifications, then newest eligible Active Notifications.
- [ ] Hidden Active Notifications are not promoted later solely because a banner slot opens.
- [ ] Tests cover replacement revision, eligible replacement presentation, progress update behavior, max-three visibility, critical-first ordering, and no delayed promotion.

## Blocked by

- [03-show-real-minimal-notification-banner.md](03-show-real-minimal-notification-banner.md)
- [04-implement-sender-lifecycle-states.md](04-implement-sender-lifecycle-states.md)
- [05-normalize-notification-content-metadata.md](05-normalize-notification-content-metadata.md)
