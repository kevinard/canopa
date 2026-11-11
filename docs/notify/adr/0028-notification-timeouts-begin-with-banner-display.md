# Notification timeouts begin with banner display

A finite **Notification Timeout** begins when the current **Notification Thread** revision first becomes visible in a **Notification Banner**, not when its request is accepted. This follows the freedesktop definition of expiration as time since display and preserves **Notification Center** actions for overflow notifications, **Silenced Notifications**, requests accepted under **Do Not Disturb**, and **Headless Mode** requests that were never displayed. Those no-display cases deliberately remain active indefinitely until the user or sender completes their lifecycle. [ADR 0054](0054-bounded-retention-of-undisplayed-and-pending-notifications.md) supersedes this for silenced, DND and overflow notifications: their timeout begins at acceptance or when they overflow. Headless Mode still expires nothing.

Once a timeout begins, its deadline continues even if the banner is displaced by a critical notification. Hidden notifications are not promoted merely because a slot opens. A replacement that revises the **Notification Thread** cancels the prior revision's timeout; the replacement revision gets a fresh timeout only if it becomes visible, while a replacement revision that remains overflowed or silenced remains active without automatic expiration. A dropped replacement does not revise the thread and preserves its existing timeout. A zero timeout never expires, and a server-default timeout resolves through the configured urgency default only when banner display begins.

A firing timeout retains its revision-scoped registration until expiration commits,
another mutation supersedes it, or shutdown cancels it. Its task handle is detached
before commitment so its own cancellation effect cannot abort sender publication.
Storage failures retry after 100, 200, 400, 800 and 1,600 milliseconds, for at most
six attempts. Retrying does not change the lifecycle precondition or start a fresh
display interval. Replacement and shutdown cancel pending retries under the same
timer lock used by display acknowledgements.

After retry exhaustion the thread remains active and the failed registration stays
observable until explicit expiration, dismissal, closure or replacement. Control
status exposes `timeout_retrying` and `timeout_failed` counts beside `timeout_records`;
content-free failure logs distinguish retrying and terminal failures. A transient
storage failure can therefore recover without another display acknowledgement,
while a persistent failure cannot silently remove its diagnostic record.
