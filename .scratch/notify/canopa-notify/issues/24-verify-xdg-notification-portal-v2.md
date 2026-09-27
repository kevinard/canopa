# Verify XDG Notification portal v2 delivery end to end

Status: implemented
Type: AFK

## Parent

[Canopa Notify](../PRD.md)

## What to build

Establish and test the supported path by which a sandboxed application sends, updates, removes, and activates a notification through XDG Notification portal v2 into Canopa Notify. The slice should identify which portal fields survive the installed portal implementation and provide a repeatable compatibility fixture before Notify adds portal-specific behavior.

## Acceptance criteria

- [x] A documented test environment routes `AddNotification` through a real XDG Desktop Portal frontend to Canopa Notify.
- [x] Title, body, application identity, priority, default action, named buttons, and icon behavior are verified end to end.
- [x] Reusing one application-scoped portal ID updates the same active notification without flicker or duplicate Notification History entries.
- [x] `RemoveNotification` withdraws the corresponding Active Notification and completes the application-visible lifecycle.
- [x] Action activation returns the correct application-scoped ID, action target, and Activation Token where the portal supports them.
- [x] The fixture records current behavior for portal v2 display hints, category, file-descriptor visuals, sound, and inline-reply purposes even when unsupported.
- [x] Automated coverage handles deterministic protocol behavior, with a clearly documented manual check only for environment-owned presentation behavior.

## Blocked by

None - can start immediately.

## Comments

- Implemented on 2026-09-03. `scripts/manual/verify-xdg-notification-portal.sh` creates an isolated D-Bus session and a Bubblewrap caller with Flatpak-compatible identity metadata, then exercises the installed real `xdg-desktop-portal` frontend, GTK implementation backend, and a headless Canopa instance owning the canonical freedesktop notification name only inside that session. It asserts trusted App Identity, stable field translation, revision-preserving replacement, one history item, removal, and default/named action round trips.
- The installed `xdg-desktop-portal` 1.22.0 frontend publishes Notification version 1 because the selected `xdg-desktop-portal-gtk` 1.15.3 backend reports implementation version 0; both expose empty `SupportedOptions`. `docs/notify/xdg-notification-portal-v2.md` records the normative v2 contract and the observed v1 degradation for markup body, display hints, category, FD visuals, sound, button purpose, inline reply, and activation-token platform data. These fields are filtered before Canopa and are not Canopa feature gaps.
- The real GTK route exposed one Canopa compatibility defect: portal default actions become a freedesktop `default` action with an empty label. Intake now preserves that special pair while continuing to reject empty labels for named actions, with a public D-Bus regression test. The fixture also passes a real sealable FD icon, observes GTK's `image-data` translation, and proves Canopa accepts the resulting image-only notification. The compatibility fixture passes, and `cargo test --workspace --all-targets` passes all 216 tests.
