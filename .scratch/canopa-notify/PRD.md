# Canopa Notify PRD

Status: implemented (selected freedesktop static profile; see completion record)

## Problem Statement

Linux desktop notifications are useful but coarse: applications can interrupt the user freely, history is often ephemeral, and notification policy is usually limited to global presentation settings. A user running a Wayland/Niri-centered desktop wants a notification daemon that behaves more like modern mobile systems: applications can be allowed, silenced, or dropped; past notifications remain auditable; and the notification center is available from Niribar without making Niribar own notification policy.

Canopa Notify must provide the normal freedesktop notification daemon behavior expected by Linux applications while adding durable history, app permission controls, and a Niribar-hosted Notification Center. It must remain Wayland-only, Rust-based, and implemented with iced/layer-shell for daemon-owned Notification Banners.

## Solution

Build Canopa Notify as a Wayland-only Rust notification daemon. The Notification Daemon owns the Notification Server API, notification policy, Notification History, Notification Banners, the User Rule Store, and the Notify Control API. Niribar hosts the Notification Center through an External Panel Provider integration, but does not own notification receipt, rules, history, or lifecycle.

The user experience centers on three surfaces:

- Notification Banners: transient, non-focus-stealing, daemon-owned Wayland layer-shell surfaces for eligible Active Notifications.
- Notification Center: a Niribar-hosted panel provider view showing Active Notifications and recent Notification History grouped by App Identity, with App Permission Controls.
- Control Commands: minimal diagnostic and control commands for status, active notifications, Do Not Disturb, and configuration reload.

The first milestone is a tracer bullet: receive a freedesktop notification, apply default-allow policy, persist it, show a real minimal banner, expose it through control commands, and later render it through the Niribar provider path once the shared panel protocol crate is extracted.

## User Stories

1. As a desktop user, I want applications to send standard Linux notifications, so that existing applications work with Canopa Notify.
2. As a desktop user, I want Canopa Notify to own the freedesktop notification service, so that notification behavior is centralized.
3. As a desktop user, I want Canopa Notify to fail clearly if another notification daemon is already running, so that I understand why notifications are not handled.
4. As a desktop user, I want Notification Banners to appear without stealing keyboard focus, so that notifications do not interrupt typing.
5. As a desktop user, I want Notification Banners to appear on the focused output when possible, so that notifications appear where my attention is.
6. As a desktop user, I want banners to fall back to a configured or default output, so that notifications still appear when focused output detection is unavailable.
7. As a desktop user, I want at most three banners visible by default, so that noisy applications do not fill my screen.
8. As a desktop user, I want critical notifications to take precedence over normal banners, so that urgent events remain visible.
9. As a desktop user, I want old hidden notifications not to pop up later just because a banner slot opens, so that notifications do not surprise me after the fact.
10. As a desktop user, I want clicking a banner to invoke its Default Notification Action when available, so that normal notification activation works.
11. As a desktop user, I want clicking a banner without a default action to dismiss it, so that basic cleanup is quick.
12. As a desktop user, I want notification actions to remain available while a notification is active, so that I can respond from the banner or Notification Center.
13. As a desktop user, I want non-resident notifications to close after action invocation, so that completed interactions do not linger.
14. As a desktop user, I want Resident Notifications to remain active after action invocation, so that sender intent is respected.
15. As a desktop user, I want critical notifications not to expire automatically by default, so that urgent notifications require attention.
16. As a desktop user, I want low and normal notifications to expire according to configured timeouts, so that banners clear themselves.
17. As a desktop user, I want sender-requested timeouts respected within my configured bounds, so that applications can express intent without overriding my policy.
18. As a desktop user, I want notification progress rendered in banners when provided, so that downloads and transfers are understandable at a glance.
19. As a desktop user, I want notification body markup converted to readable plain text, so that raw tags are not shown.
20. As a desktop user, I want body-only notifications to be accepted, so that poorly formed but useful notifications still appear.
21. As a desktop user, I want empty no-op notifications dropped, so that blank records do not clutter history.
22. As a desktop user, I want Notification History to persist across daemon restarts, so that I can review missed notifications later.
23. As a desktop user, I want dismissed notifications to remain in history, so that dismissal does not destroy records.
24. As a desktop user, I want expired notifications to remain in history with a distinct state, so that I can tell timeout from dismissal.
25. As a desktop user, I want sender-closed notifications to remain in history with a distinct state, so that I can tell app closure from user action.
26. As a desktop user, I want stale notifications after daemon restart to be marked inactive, so that old action callbacks are not presented as live.
27. As a desktop user, I want historical action labels to remain as read-only metadata, so that history still explains what was possible.
28. As a desktop user, I want active notifications exempt from retention pruning, so that live notifications are not deleted by history limits.
29. As a desktop user, I want history bounded by default to 30 days and 1,000 Notification Threads, so that storage and privacy are controlled.
30. As a desktop user, I want retained history to avoid arbitrary image payloads by default, so that storage and privacy risks are reduced.
31. As a desktop user, I want notification history stored locally, so that it remains available without a network service.
32. As a desktop user, I want the Notification Center to show Active Notifications and recent Notification History, so that one panel covers triage and review.
33. As a desktop user, I want the Notification Center grouped by App Identity, so that noisy senders are easy to inspect.
34. As a desktop user, I want App Permission Controls in the Notification Center, so that I can allow, silence, or drop future notifications from a recent sender.
35. As a desktop user, I want App Permission Controls to show current policy, so that I know what will happen next.
36. As a desktop user, I want historical records to preserve their original Delivery Outcome, so that rule changes do not rewrite history.
37. As a desktop user, I want opening the Notification Center to be read-only by default, so that inspection does not dismiss notifications.
38. As a desktop user, I want the Notification Center to update live while open, so that replacements, dismissals, progress, and DND changes are reflected.
39. As a desktop user, I want the Notification Center to be keyboard navigable, so that it is usable without a pointer.
40. As a desktop user, I want Clear Active Notifications to be separate from Delete Notification History, so that I do not accidentally erase history.
41. As a desktop user, I want inactive history to be read-only except for deletion, so that old notifications do not trigger stale sender callbacks.
42. As a desktop user, I want full-text history search deferred, so that the first version focuses on reliable core notification behavior.
43. As a desktop user, I want unknown App Identities allowed by default, so that applications continue to work unless I restrict them.
44. As a desktop user, I want App Identity based primarily on desktop entry, so that rules follow real desktop applications rather than unstable labels.
45. As a desktop user, I want app-name fallback when desktop entry is unavailable, so that rules still work for imperfect senders.
46. As a desktop user, I want Notification Rules to match App Identity plus optional Notification Category and Notification Urgency, so that app permissions can treat notification types and critical messages differently.
47. As a desktop user, I want first decisive rule semantics, so that policy is predictable and auditable.
48. As a desktop user, I want explicit drop rules to apply even to critical notifications, so that my policy wins over sender urgency.
49. As a desktop user, I want Silenced Notifications retained in history, so that I can audit what was suppressed.
50. As a desktop user, I want Dropped Notifications not retained, so that drop means no record.
51. As a desktop user, I want dropped notifications closed immediately from the sender lifecycle perspective, so that applications do not think they remain active.
52. As a desktop user, I want silenced notifications to remain active without banners, so that actions can still be available in the Notification Center.
53. As a desktop user, I want replacement notifications to revise an existing Notification Thread, so that history does not fill with transient updates.
54. As a desktop user, I want silenced replacements to update history and close visible banners, so that current policy takes effect.
55. As a desktop user, I want dropped replacements ignored, so that drop remains stronger than silence.
56. As a desktop user, I want eligible replacements to be new presentation opportunities, so that a live updated notification can show when policy allows.
57. As a desktop user, I want Do Not Disturb to silence non-critical notifications while retaining them, so that I can pause interruptions without losing history.
58. As a desktop user, I want Do Not Disturb to persist across daemon restarts, so that a restart does not unexpectedly re-enable interruptions.
59. As a desktop user, I want Do Not Disturb to override app allow rules for non-critical notifications, so that global quiet mode is reliable.
60. As a desktop user, I want Notification Configuration reloads to keep the previous valid config on errors, so that a typo does not break current policy.
61. As a desktop user, I want rule changes to re-evaluate active notifications, so that currently visible banners respect current policy.
62. As a desktop user, I do not want rule re-evaluation to create late banners for older active notifications, so that policy changes do not create surprise interruptions.
63. As a desktop user, I want UI-managed rule changes stored separately from hand-written KDL config, so that the UI does not rewrite my configuration file.
64. As a desktop user, I want User Rule Store rules to take precedence over config rules, so that direct UI choices apply immediately.
65. As a desktop user, I want config rules to support optional Config Rule IDs, so that Delivery Outcomes can reference meaningful rule names.
66. As a desktop user, I want Delivery Decision Logs without Notification Content, so that policy debugging does not leak message bodies.
67. As a desktop user, I want daemon/config errors reported through logs and control surfaces, so that Canopa Notify does not notify through itself.
68. As a desktop user, I want `canopa-notify ctl status`, so that I can diagnose whether the daemon is running.
69. As a desktop user, I want `canopa-notify ctl reload`, so that I can reload configuration without restarting the daemon.
70. As a desktop user, I want `canopa-notify ctl dnd` controls, so that I can toggle Do Not Disturb from scripts or keybindings.
71. As a desktop user, I want diagnostic commands to support JSON where useful, so that tests and scripts can inspect daemon state.
72. As a Niribar user, I want `canopa-notify panel-provider` to render the Notification Center, so that I can open it from a Niribar action.
73. As a Niribar user, I want the panel provider to show an error if the daemon is unavailable, so that failure is visible instead of silent.
74. As a Niribar user, I want first-version docs with a Niribar configuration example, so that I can wire the provider into my bar.
75. As a developer, I want a dev/test D-Bus service name, so that I can test Canopa Notify without taking over my normal desktop notifications.
76. As a developer, I want Headless Mode for tests, so that policy, storage, D-Bus, and control behavior can run in CI.
77. As a developer, I want a vertical tracer bullet first, so that D-Bus, Wayland, SQLite, private IPC, and Niribar integration are proven early.
78. As a developer, I want a shared panel protocol crate extracted from Niribar, so that Notify does not depend on the Niribar application crate.
79. As a packager, I want a single `canopa-notify` binary with subcommands, so that install and versioning remain simple.
80. As a packager, I want a systemd user unit, so that startup is explicit and predictable.

## Implementation Decisions

- Public identity is **Canopa Notify**. The installed binary and service identity are `canopa-notify`; `GetServerInformation` reports product name Canopa Notify and vendor Canopa.
- Canopa Notify is Wayland-only. Niri is the Primary Compositor, but other Wayland compositors with needed layer-shell/output capabilities are allowed.
- The first version supports one Graphical User Session per user and enforces one active Notification Daemon instance per session.
- Outside Headless Mode, failure to connect to a Wayland Session is startup failure.
- The Notification Daemon owns the freedesktop Notification Server API and fails fast when it cannot own it.
- The Notification Server API scope is freedesktop compatibility first, not Dunst compatibility. Dunst configuration import is out of scope.
- D-Bus implementation uses `zbus`.
- Development runs may use a dev/test D-Bus service name; service-mode runs use `org.freedesktop.Notifications`.
- Startup is through an explicit systemd user service before D-Bus activation. Version-one install artifacts include a systemd user unit and no autostart desktop file.
- Canopa Notify ships as one binary with `daemon`, `panel-provider`, and `ctl` modes.
- The first milestone is a tracer bullet that receives, stores, presents, controls, and eventually exposes one notification through the panel provider path.
- The tracer bullet includes a real minimal Notification Banner and does not need user-authored rule parsing.
- The tracer bullet's receive/store/banner/control parts can proceed before shared panel protocol extraction. The Niribar provider slice waits for the shared protocol crate.
- Notify and Niribar share panel protocol types through a small Canopa shared protocol crate extracted from Niribar.
- The Notification Center is daemon-owned state rendered through Niribar's external panel provider integration. Niribar does not own notification policy, history, or lifecycle.
- The first Notification Center targets Niribar's existing external panel provider primitives and does not require a dedicated Niribar notification module.
- Panel Provider Mode reports provider-visible error content if the Notification Daemon is unavailable and does not start the daemon implicitly.
- Control Commands report daemon unavailability instead of starting the daemon implicitly.
- The Notify Control API uses a per-user Unix domain socket under the runtime directory.
- The Notify Control API supports Notification State Subscription for the open Notification Center instead of polling.
- The daemon runtime is Tokio. Pure notification policy and storage logic should remain testable without Tokio where practical.
- D-Bus receipt, storage, policy, private control IPC, and iced/layer-shell banner presentation run in one daemon process for version one.
- Canopa Notify starts as a Cargo workspace, separating policy, storage, D-Bus receipt, Wayland presentation, control API, CLI/provider integration, and shared protocol concerns.
- iced and layer-shell dependencies should align with Niribar's source/version where practical.
- Notification History and the User Rule Store are stored in one per-user SQLite database accessed through `sqlx`.
- Database migrations exist from the start.
- Notification Configuration uses KDL and should align with Niribar's parser approach and diagnostic style where practical.
- Canopa Notify has one active Notification Configuration at a time.
- Invalid configuration reloads are rejected while the previous valid configuration stays active.
- Valid configuration reloads affect future requests, including replacements, under ADR 0027; existing notifications keep their committed delivery decision.
- Notification Daemon and configuration errors are reported through logs and control surfaces, not self-notifications.
- App Identity is primarily the desktop entry hint, with app name fallback.
- Unknown App Identities are allowed unless a Notification Rule says otherwise.
- Notification Rules use ordered first-decisive semantics for delivery policy.
- Rules match App Identity plus optional Notification Category and Notification Urgency.
- User-managed App Permission Controls write to the User Rule Store, not the KDL configuration file.
- User Rule Store rules are keyed by App Identity, optional Notification Category, and optional Notification Urgency.
- User Rule Store rules take precedence over Notification Configuration rules.
- Notification Configuration rules may define optional Config Rule IDs; unnamed config rules use generated references.
- Rule changes affect future requests without re-evaluating existing active or inactive notifications (ADR 0027).
- Policy changes alone do not close or create banners.
- Do Not Disturb persists across restarts, silences non-critical notifications, and takes precedence over app allow rules for non-critical notifications.
- Explicit drop rules apply even to critical notifications.
- Silenced Notifications remain retained and may remain active without a visible banner.
- Dropped Notifications are not retained and are immediately closed from the Sender Notification Lifecycle perspective.
- Replacement notifications revise existing Notification Threads.
- Silenced replacements update history and close visible banners. Dropped replacements do not update history.
- Eligible replacements may create banners even when prior revisions did not.
- Notification History persists across restarts, with default retention of 30 days and 1,000 Notification Threads.
- Retention pruning runs after writes and at daemon startup.
- Active Notifications and retained content requiring attention are exempt from archive pruning (ADR 0049).
- Active Notifications loaded after daemon restart become Stale Notifications, and old banners are not restored.
- Notification History does not persist arbitrary notification image payloads by default and is not app-level encrypted in version one.
- Notification Category is validated metadata and is available to category-specific Notification Rules and Notification Center controls.
- Notification Progress is stored as metadata and may be rendered by Notification Banners; first Niribar-hosted Notification Center does not require progress rendering.
- Raw Notification Body is retained; ordered safe markup, local body images and HTTP(S) links render with a plain fallback.
- Body-only notifications are valid. Empty no-op notifications are dropped.
- Notification Banners are daemon-owned, top-right on the focused output by default, capped at three visible banners, and never steal keyboard focus.
- Banner visibility prefers critical Active Notifications and then newest eligible Active Notifications.
- Hidden Active Notifications are not promoted later solely because a banner slot opens.
- First-version banner placement is global, not per-output.
- Notification Timeout follows ADR 0028: zero never expires, positive values apply after first display, and negative values use the daemon default; critical defaults do not expire.
- Critical Active Notifications do not expire automatically by default.
- Bounded local notification sound and sound-theme lookup are implemented when the audio adapter is available.
- Hiding notification body content for privacy-sensitive display states belongs to Compositor Privacy Mode, not Notify.
- Notification Actions are invokable only while a Notification Thread is active, including silenced Active Notifications shown in the Notification Center.
- Inactive Notification History supports acknowledgement and deletion, plus safe body links. Stale sender actions remain unavailable.
- Invoking an action closes the notification unless it is a Resident Notification.
- User dismissal has the same Sender Notification Lifecycle effect whether it starts from a banner or the Notification Center.
- The Notification Center shows Active Notifications and recent Notification History grouped by App Identity.
- The Notification Center includes App Permission Controls for recent notification senders.
- Opening the Notification Center does not change notification state.
- An open Notification Center updates live as notification state changes.
- The first Notification Center must be keyboard navigable.
- Clear Active Notifications and Delete Notification History are separate actions.
- Control Commands are diagnostic-first and do not promise stable output formats unless explicitly documented. Data-returning commands may support unstable JSON.

## Testing Decisions

- Test external behavior at the highest available seam. Pure domain behavior should be tested without D-Bus, Wayland, SQLite, or Tokio unless those integrations are the behavior under test.
- Policy tests should cover App Identity extraction, default allow, first-decisive rules, User Rule Store precedence, DND precedence, explicit critical drop, Delivery Outcome, Rule Reference, and content-redacted Delivery Decision Logs.
- Lifecycle tests should cover Active, Dismissed, Expired, Closed, Stale, Silenced, and Dropped states; sender-visible close behavior; action availability; Resident Notification behavior; and restart stale conversion.
- Replacement tests should cover thread revision, silenced replacements closing banners, dropped replacements not updating history, and eligible replacements creating new presentation opportunities.
- History storage tests should cover SQLite migrations, insert/update of Notification Threads, retention pruning, active retention exemption, stale startup transition, history deletion, and omission of arbitrary image payload persistence.
- Configuration tests should cover KDL parsing, invalid reload rejection, one active configuration, config rule IDs, generated references for unnamed rules, and future-request-only policy changes.
- Control API tests should cover Unix socket request/response behavior, daemon unavailable errors, state subscriptions, DND commands, reload commands, active notification listing, and unstable JSON output for diagnostics.
- D-Bus integration tests should run against the dev/test service name where possible and cover `Notify`, `CloseNotification`, server information, action invocation signals, and close signals.
- Presentation coordinator tests should cover banner eligibility, maximum visible banners, critical-first ordering, no delayed promotion, no late banners from policy changes, focused-output fallback, timeout decisions, and non-focus-stealing intent.
- Wayland/iced rendering should have smoke/integration coverage for a real minimal banner in a graphical test environment where available. Headless Mode should cover the same policy/storage/control behavior without graphical surfaces.
- Panel provider tests should cover daemon unavailable provider error content, initial snapshot rendering, live state updates through Notification State Subscription, App Permission Control events, clear/delete actions, and mapping Notify state into the shared panel protocol.
- CLI tests should cover exit codes and stable documented behavior for status, reload, DND, and active notification diagnostics. Non-documented human output should not be over-specified.
- Packaging/startup tests should cover single-instance enforcement and failure when the real Notification Server API name is already owned.

## Out of Scope

- X11 support or X11 fallback behavior.
- Dunst protocol compatibility, Dunst control compatibility, or Dunst configuration import.
- D-Bus activation for version one.
- A dedicated Niribar notification module in version one.
- Standalone settings UI.
- Standalone tray or status indicator.
- Full-text Notification History search.
- General unread/read workflows beyond retained-attention acknowledgement (ADR 0049).
- Remote audio resources and unbounded playback.
- Inline replies in version one.
- App-level body hiding or screenshot privacy mode; this belongs to Compositor Privacy Mode.
- Arbitrary notification image persistence in history by default.
- App-level encryption of Notification History in version one.
- Multi-seat or multiple graphical sessions for the same user.
- Per-output banner placement in version one.
- Per-app timeout configuration in version one.
- Rich theming or deep Dunst-style styling in version one.
- Explicit localization infrastructure in version one.
- Stable machine-readable CLI output except where explicitly documented.
- User-authored rule parsing in the first tracer bullet.
- Progress rendering in the first Niribar-hosted Notification Center.

## Further Notes

- The first implementation should resist building a large backend foundation before proving the vertical path. The tracer bullet is expected to drive crate boundaries, IPC shape, and presentation constraints.
- The shared panel protocol crate is a prerequisite for the final provider portion of the tracer bullet. The rest of the daemon can proceed independently.
- Canopa Notify should include first-version documentation showing how to configure Niribar to open `canopa-notify panel-provider` through its external panel provider mechanism.
- The repository uses local markdown tracking for now. Follow-up implementation issues should live under this feature directory and carry `Status: ready-for-agent` when fully specified.

## Completion alignment (2026-09-16)

The current contract is the [selected completion scope](../freedesktop-completion-review/PLAN.md), its ADRs, and the [release record](../../docs/notification-release-verification.md). Safe body links, all actions, local body images, retained attention and native accessibility are implemented together with Canopa-v4 Niribar support. Native portal v2, replies, grouping, snoozing and authenticated app permissions remain separate work.
