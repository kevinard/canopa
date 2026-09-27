# XDG Notification portal v2 and the Canopa delivery path

Research snapshot: 2026-09-03. This note distinguishes the version 2 protocol from the behavior of the portal stack installed on the development machine. The protocol and implementation findings below come from the official XDG Desktop Portal documentation and the matching first-party source releases.

## Bottom line

The public Notification portal v2 is not a second notification-server API for Canopa to implement. A sandboxed client calls `org.freedesktop.portal.Notification`; the `xdg-desktop-portal` frontend derives the caller's application ID and calls a selected `org.freedesktop.impl.portal.Notification` desktop backend. The GTK backend selected by Niri then translates that request into the ordinary `org.freedesktop.Notifications.Notify` call that Canopa already implements.

The installed frontend is `xdg-desktop-portal` 1.22.0, but the selected `xdg-desktop-portal-gtk` 1.15.3 backend reports Notification backend version `0`. The frontend treats that as version 1 and therefore publishes public Notification version `1`, not `2`. Both live `SupportedOptions` dictionaries are empty. Consequently the installed route can verify the stable v1 fields and record how v2 inputs degrade, but it cannot demonstrate true v2 delivery of `markup-body`, `sound`, `display-hint`, `category`, or button `purpose`. A fixture must assert the discovered version instead of assuming that installing a frontend with the v2 XML makes v2 active.

## D-Bus protocol surface

The application-facing service is `org.freedesktop.portal.Desktop`, object `/org/freedesktop/portal/desktop`, interface `org.freedesktop.portal.Notification`:

| Member | D-Bus signature | Meaning |
| --- | --- | --- |
| `AddNotification` | `s id, a{sv} notification -> ()` | Add or update an application-scoped notification. The method supports an accompanying Unix FD list. |
| `RemoveNotification` | `s id -> ()` | Withdraw that application's notification ID. |
| `ActionInvoked` | signal `s id, s action, av parameter` | Report a non-exported action using the application's original string ID. |
| `version` | read-only `u` | Negotiated frontend version, capped by the selected backend version. |
| `SupportedOptions` | read-only `a{sv}` | Backend-advertised `category: as` and `button-purpose: as` values when available. |

These signatures and replacement rules are normative in the [official Notification API](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Notification.html). The backend contract adds the trusted application identity: `AddNotification(s app_id, s id, a{sv})`, `RemoveNotification(s app_id, s id)`, and `ActionInvoked(s app_id, s id, s action, av parameter)`, as documented in the [official backend API](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.impl.portal.Notification.html).

All `AddNotification` keys are optional. Their exact v2 forms and behavioral intent are:

| Key | Type inside `a{sv}` | Contract |
| --- | --- | --- |
| `title` | `s` | Short title; a UI may truncate it to one line. |
| `body` | `s` | Potentially long body; a UI may wrap or truncate it. |
| `markup-body` | `s` | V2 formatted body. Only XML-style `b`, `i`, and `a href=...` markup is retained; unsupported markup and line breaks are removed. |
| `icon` | `v` containing `s` or `(sv)` | A legacy string means one themed icon name. `(sv)` supports `themed: as`, deprecated `bytes: ay`, or v2 `file-descriptor: h`. |
| `sound` | `v` containing `s` or `(sv)` | `"default"`, `"silent"`, or `(file-descriptor, h)` containing Ogg/Opus, Ogg/Vorbis, or WAV/PCM. |
| `priority` | `s` | Exactly `low`, `normal`, `high`, or `urgent`. |
| `default-action` | `s` | Action activated when the notification itself is selected. |
| `default-action-target` | `v` | Optional target passed to the default action. |
| `buttons` | `aa{sv}` | Each button requires `action: s` and either `label: s` or `purpose: s`; it may carry an arbitrary `target: v`. |
| `display-hint` | `as` | Any of `transient`, `tray`, `persistent`, `hide-on-lock-screen`, `hide-content-on-lock-screen`, and `show-as-new`. |
| `category` | `s` | A standardized content category or an extension beginning `x-`. |

The [official v2 field documentation](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Notification.html#org-freedesktop-portal-notification-addnotification) defines the types, allowed values, and UI latitude. FD icons must be sealable memfds containing PNG, JPEG, or SVG. Portal icon validation additionally requires a square icon, at most 512 pixels per raster edge, at most 4 MiB of raster data, or at most 4096 bytes of SVG data; see the [official icon validation requirements](https://flatpak.github.io/xdg-desktop-portal/docs/icons.html).

`display-hint` expresses policy rather than an unconditional rendering command. `transient` requests banner-only handling, `tray` requests tray-only handling, `persistent` prevents ordinary user dismissal while the app has a window, the lock-screen hints constrain disclosure, and `show-as-new` deliberately asks for remove/add presentation when reusing an ID. Without `show-as-new`, reusing an ID must update in place without flicker. `transient` and `tray` together are a programmer error.

The standardized `im.received` category defines button purpose `im.reply-with-text`. For an exported action the reply string is the second action parameter, after an optional target; for a non-exported action it is the third `ActionInvoked.parameter` value, after target and platform data. Other standardized purposes cover incoming/ongoing call controls, while `system.custom-alert` permits a policy-controlled application alert callback. Servers may advertise understood category and purpose values through `SupportedOptions`; an unrecognized purpose should degrade to a normal labeled button. The official API contains the [category and purpose rules](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Notification.html#org-freedesktop-portal-notification-addnotification).

There is a spelling discrepancy in the installed 1.22.0 release: the installed XML comments say `hide-on-lockscreen` and `hide-content-on-lockscreen`, while the implementation accepts `hide-on-lock-screen` and `hide-content-on-lock-screen`. Current official generated documentation uses the implementation spellings. The compatibility fixture should use the hyphenated `lock-screen` forms and separately record this 1.22.0 documentation mismatch. See the [1.22.0 parser](https://github.com/flatpak/xdg-desktop-portal/blob/1.22.0/src/notification.c#L851-L900) and [1.22.0 interface XML](https://github.com/flatpak/xdg-desktop-portal/blob/1.22.0/data/org.freedesktop.portal.Notification.xml#L235-L280).

## IDs, replacement, and removal

The portal ID is an opaque application-provided `s`, not the numeric freedesktop server ID. Its namespace is the pair `(derived app_id, portal id)`. Reusing that pair without removal updates the notification; another application may independently use the same string. The frontend and GTK backend both index their active state by that pair. The GTK backend retains the numeric `u` returned by `org.freedesktop.Notifications.Notify` and supplies it as `replaces_id` on the next call for the pair. This is the mechanism that should update one Canopa Active Notification and one history thread. See the GTK backend's [pair lookup and replacement call](https://github.com/flatpak/xdg-desktop-portal-gtk/blob/1.15.3/src/fdonotification.c#L31-L90) and [add/remove implementation](https://github.com/flatpak/xdg-desktop-portal-gtk/blob/1.15.3/src/fdonotification.c#L381-L451).

`RemoveNotification(id)` becomes backend `RemoveNotification(app_id, id)`. GTK resolves the pair to the last numeric server ID, calls `org.freedesktop.Notifications.CloseNotification(u)`, and forgets its mapping. Calling remove for an unknown pair is effectively a successful no-op: the portal methods have no result payload and the GTK backend does not propagate its internal Boolean lookup result. The public portal has no `NotificationClosed` signal, so application-visible completion is the return from `RemoveNotification`; Canopa's freedesktop `NotificationClosed` signal is consumed only by the backend. The frontend currently returns from `RemoveNotification` before its asynchronous backend call completes, so the fixture must observe Canopa state rather than treating the method return as proof that presentation has already disappeared. See the frontend [remove forwarding](https://github.com/flatpak/xdg-desktop-portal/blob/1.22.0/src/notification.c#L1140-L1191) and GTK [close conversion](https://github.com/flatpak/xdg-desktop-portal-gtk/blob/1.15.3/src/fdonotification.c#L381-L415).

One implementation edge deserves a deterministic regression test: GTK stores the returned numeric server ID asynchronously. A second add for the same portal pair that arrives before the first `Notify` reply could still send `replaces_id = 0`. Normal sequential D-Bus delivery is likely to make the first reply win the race, but the source does not establish that as an invariant. The end-to-end replacement fixture should send updates back-to-back and assert one active/history thread rather than testing only a delayed replacement.

## Actions and activation data

V2 distinguishes exported actions from non-exported actions:

- An action name beginning `app.` is D-Bus activated on the application. The prefix is stripped for `org.freedesktop.Application.ActivateAction`; the method's `av` holds the optional target and, for inline reply, the response. Its separate platform-data vardict carries the XDG `activation-token`. No portal `ActionInvoked` signal is emitted for this path.
- Other action names cause the public `ActionInvoked(id, action, av)` signal. In v2 the ordered `av` contains an optional target, a platform-data `a{sv}` containing `activation-token: s`, then a purpose-specific response when applicable.

That contract is specified in the official [action overview and signal documentation](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Notification.html#org-freedesktop-portal-notification-actioninvoked). The backend signal includes `app_id` so the frontend can validate the `(app_id, id)` pair, then the frontend removes `app_id` when relaying to the application; see the [1.22.0 relay](https://github.com/flatpak/xdg-desktop-portal/blob/1.22.0/src/notification.c#L1194-L1219).

The installed GTK v1 route has narrower behavior. GTK subscribes to the freedesktop `ActivationToken(u, s)` extension and saves the token until the following `ActionInvoked(u, s)`. It maps the numeric ID back to `(app_id, portal id)` and decodes named-button targets from GLib detailed action names. For `app.*`, it passes both `activation-token` and legacy `desktop-startup-id` in `ActivateAction` platform data. For a non-exported action it first calls application `Activate` with that platform data, then emits backend `ActionInvoked` with only the optional target in `av`. The public v1 signal therefore returns the correct portal ID, action, and target, but does **not** contain the activation-token platform-data slot promised by v2. See the GTK [freedesktop signal mapping](https://github.com/flatpak/xdg-desktop-portal-gtk/blob/1.15.3/src/fdonotification.c#L92-L171) and [application activation bridge](https://github.com/flatpak/xdg-desktop-portal-gtk/blob/1.15.3/src/notification.c#L44-L112).

## Real installed delivery path

Niri's portal configuration explicitly selects GTK for Notification ([upstream `niri-portals.conf`](https://github.com/niri-wm/niri/blob/main/resources/niri-portals.conf)):

```text
sandboxed application
  -> org.freedesktop.portal.Desktop
     /org/freedesktop/portal/desktop
     org.freedesktop.portal.Notification.AddNotification(s, a{sv})
  -> org.freedesktop.impl.portal.desktop.gtk
     /org/freedesktop/portal/desktop
     org.freedesktop.impl.portal.Notification.AddNotification(app_id, s, a{sv})
  -> org.freedesktop.Notifications
     /org/freedesktop/Notifications
     org.freedesktop.Notifications.Notify(susssasa{sv}i) -> u
  -> Canopa Notify when Canopa owns org.freedesktop.Notifications
```

`xdg-desktop-portal` derives `app_id` from the authenticated calling peer and forwards that value; the application cannot supply an `app_id` argument to the public method. It also validates and filters the vardict according to the selected backend version before making the backend call. See the [frontend validation and forwarding](https://github.com/flatpak/xdg-desktop-portal/blob/1.22.0/src/notification.c#L947-L1108). The frontend sets its public version to `min(max(backend_version, 1), 2)` and binds `SupportedOptions` directly from the backend; see [version negotiation](https://github.com/flatpak/xdg-desktop-portal/blob/1.22.0/src/notification.c#L1289-L1307).

The exact GTK-to-freedesktop mapping is:

| `Notify` argument | Value produced by GTK 1.15.3 |
| --- | --- |
| `app_name: s` | Empty string. |
| `replaces_id: u` | `0` for a new `(app_id, portal id)`; otherwise the numeric ID returned by the previous `Notify`. |
| `app_icon: s` | First themed icon name, otherwise empty. |
| `summary: s` | Portal `title`, or empty. |
| `body: s` | Portal `body`, or empty. |
| `actions: as` | Default action becomes key `default` with an empty label; named buttons become detailed GLib action names paired with labels. Targets are encoded in the detailed action name. |
| `hints: a{sv}` | Always `desktop-entry: s = app_id` and `urgency: y`; byte icons additionally become `image-data`. |
| `expire_timeout: i` | Always `-1` (server default). |

This conversion is implemented in GTK's [freedesktop notification adapter](https://github.com/flatpak/xdg-desktop-portal-gtk/blob/1.15.3/src/fdonotification.c#L219-L378).

Only one caller identity field reaches Canopa: the trusted portal `app_id` in the freedesktop `desktop-entry` hint. The original application's unique bus name does not survive because Canopa sees the GTK backend as its D-Bus sender, and GTK sends an empty `app_name`. This is compatible with Canopa's existing identity rule, which prefers `desktop-entry`, but the fixture should assert `desktop-entry` rather than expecting a nonempty app name. It should use a sandbox with a known application ID; host-process app-ID inference can legitimately produce an empty ID.

## Field survival on the installed stack

| Portal input | What reaches Canopa through GTK 1.15.3 | Verification expectation |
| --- | --- | --- |
| `title` | Freedesktop `summary` unchanged. | Supported. |
| `body` | Freedesktop `body` unchanged. | Supported. |
| application identity | `desktop-entry = app_id`; positional `app_name = ""`. | Supported through `desktop-entry` only. |
| `priority=low` | `urgency` byte `0`. | Supported. |
| `priority=normal` | `urgency` byte `1`. | Supported. |
| `priority=high` | `urgency` byte `1`. | Accepted, but collapses to normal. |
| `priority=urgent` | `urgency` byte `2`. | Supported as critical. |
| default action and target | Freedesktop action key `default`; original action and target remain in GTK's private mapping. | Supported end to end. |
| labeled buttons and targets | Freedesktop action/label pairs; target encoded in a detailed action name and decoded on activation. | Supported end to end. |
| themed `icon` | First themed name becomes positional `app_icon`. | Supported. |
| byte or FD `icon` | The v1 frontend validates an FD icon, converts it to a byte icon for the old backend, and GTK converts it to freedesktop `image-data`. | Payload survives, but Canopa sees a notification image rather than a positional app icon. |
| `markup-body` | Filtered by the frontend because backend version is below 2. | Unsupported; only `body` survives. |
| `display-hint` | Entire key filtered before GTK, including `transient`, `tray`, `persistent`, and `show-as-new`. | Unsupported. Reusing the ID still updates through numeric `replaces_id`, but `show-as-new` cannot request remove/add animation. |
| `category` | Filtered before GTK. | Unsupported; `SupportedOptions` advertises no categories. |
| `sound="default"`, `"silent"`, or sound FD | Filtered before GTK. No freedesktop `suppress-sound`, sound-name, or sound-file hint is produced. | Unsupported. In particular, portal `silent` is not honored and Canopa remains free to use its default sound policy. |
| button `purpose` | Filtered from each button. A labeled button remains a normal button; a purpose-only button fails validation because no label remains. | Unsupported; no purposes advertised. |
| `im.reply-with-text` | Purpose is removed and the freedesktop protocol has no reply payload in `ActionInvoked`. | Inline reply is unavailable. |

The filtering gates are explicit in the [1.22.0 parser](https://github.com/flatpak/xdg-desktop-portal/blob/1.22.0/src/notification.c#L947-L1039). The same release's first-party tests cover both backend versions, including [basic fields, buttons, and removal](https://github.com/flatpak/xdg-desktop-portal/blob/1.22.0/tests/test_notification.py#L122-L165), [FD icon conversion](https://github.com/flatpak/xdg-desktop-portal/blob/1.22.0/tests/test_notification.py#L430-L514), and [v2 sound filtering and validation](https://github.com/flatpak/xdg-desktop-portal/blob/1.22.0/tests/test_notification.py#L542-L654).

## Installed environment record

Commands were run against the live user session on 2026-09-03:

```text
$ pacman -Q xdg-desktop-portal xdg-desktop-portal-gtk xdg-desktop-portal-gnome
xdg-desktop-portal 1.22.0-1
xdg-desktop-portal-gtk 1.15.3-1
xdg-desktop-portal-gnome 50.0-1

$ busctl --user get-property org.freedesktop.portal.Desktop \
    /org/freedesktop/portal/desktop org.freedesktop.portal.Notification version
u 1

$ busctl --user get-property org.freedesktop.impl.portal.desktop.gtk \
    /org/freedesktop/portal/desktop org.freedesktop.impl.portal.Notification version
u 0

$ busctl --user get-property org.freedesktop.portal.Desktop \
    /org/freedesktop/portal/desktop org.freedesktop.portal.Notification SupportedOptions
a{sv} 0
```

The installed GNOME backend also reports Notification version `0` with empty `SupportedOptions`, so selecting it would not unlock v2. At research time `org.freedesktop.Notifications` was owned by `/usr/bin/dunst`; the compatibility fixture must replace that owner with Canopa in an isolated D-Bus session or explicitly stop the desktop daemon for a manual run.

The package releases are recorded upstream as [xdg-desktop-portal 1.22.0](https://github.com/flatpak/xdg-desktop-portal/releases/tag/1.22.0) and [xdg-desktop-portal-gtk 1.15.3](https://github.com/flatpak/xdg-desktop-portal-gtk/releases/tag/1.15.3).

## Consequences for ticket 24

1. Exercise the real three-hop route; do not add `org.freedesktop.portal.Notification` to Canopa itself.
2. Record public and backend `version` plus `SupportedOptions` at fixture startup. Treat version 1 as the expected result for the checked-in environment and branch v2 assertions when a genuine v2 backend is available.
3. Use a known sandbox application ID and assert that it reaches Canopa as `desktop-entry`, while `app_name` remains empty.
4. Make replacement, removal, default activation, and named-button activation state-based end-to-end assertions. Include a back-to-back replacement case to expose the asynchronous numeric-ID race.
5. Assert the installed degradation table above for v2-only fields. Those are compatibility results, not Canopa feature gaps: the GTK backend never sends the fields to Canopa.
6. For action activation, separately verify the installed v1 result and the normative v2 shape. The current path can return ID/action/target and use the activation token to activate the application, but it cannot place platform data or an inline reply into the public signal.

## Repeatable compatibility fixture

Build Canopa, then run the checked-in fixture from a graphical session:

```bash
cargo build -p canopa-notify
scripts/manual/verify-xdg-notification-portal.sh
```

The fixture creates a private D-Bus session, starts the real installed `xdg-desktop-portal` frontend and GTK backend, and gives a headless Canopa process the canonical `org.freedesktop.Notifications` name only inside that private session. It does not stop or replace the desktop's live notification daemon. Calls originate in a Bubblewrap sandbox containing Flatpak-compatible application metadata, so the frontend must derive the known `org.canopa.PortalFixture` application ID; the script asserts that GTK forwards it as `desktop-entry` and Canopa adopts it as App Identity while the positional app name remains empty.

The script also asserts stable field delivery, same-thread replacement, one history item for the replacement sequence, removal, and default and named action round trips. Its FD helper sends a real sealable PNG memfd; the fixture observes GTK's resulting freedesktop `image-data` and uses an image-only notification to prove Canopa decoded it rather than dropping it as an empty no-op. The v1 filtering checks inspect the actual GTK-to-freedesktop call for the absence of markup, display, category, and sound fields instead of relying on reported expectations alone.

The fixture prints the negotiated versions and records v2-only checks as unavailable when the selected backend keeps the frontend at v1. Override the expected public version with `PORTAL_EXPECT_VERSION` when deliberately checking a different installed stack.

## Environment-owned presentation check

The private fixture is deliberately headless, so replacement flicker and desktop activation-token acquisition remain a short graphical check:

1. Start Canopa in service mode in a disposable graphical session where it may own `org.freedesktop.Notifications`.
2. Send a persistent notification through the public portal, then immediately reuse its portal ID with changed title and body. Confirm one banner updates in place without disappearing and reappearing.
3. Activate its default action and one named button while monitoring `org.freedesktop.portal.Notification.ActionInvoked`. Confirm the original portal ID, action, and target. On a v2 backend, also confirm the ordered platform-data dictionary contains `activation-token`; on the installed v1 GTK backend it is expected to be absent.
4. Call `RemoveNotification` and confirm the banner disappears. The Notification Center should retain one closed thread for the replacement sequence, not one item per update.

These are the only manual assertions: they depend on a compositor-created surface and input serial. Payload translation, lifecycle state, history identity, and action routing remain automated in the private compatibility fixture and the Rust D-Bus integration suite.
