# Add lifecycle-owned guarded timeout expiration

Status: ready-for-agent
Type: AFK

## What to build

Add a **Notification Lifecycle** operation for timer expiry that expires an **Active Notification** only when the retained **Notification Thread** revision still matches the timer revision. This keeps revision-guarded expiration meaning inside **Notification Lifecycle** instead of having the **Notification Daemon** inspect **Notification History** directly.

## Acceptance criteria

- [x] **Notification Lifecycle** exposes a revision-guarded expiration operation for timer expiry.
- [x] The operation is a no-op when the **Notification Thread** is no longer active.
- [x] The operation is a no-op when the retained revision differs from the timer revision.
- [x] The operation produces the same **Lifecycle Effects** as normal expiration when the revision matches.
- [x] Lifecycle tests cover matching revision, stale revision, and inactive thread cases through the **Notification Lifecycle** interface.
- [x] Existing timeout and replacement tests still pass.

## Blocked by

None - can start immediately
