# Everyday controls, lock-screen feed and portal v2 presentation

Status: accepted (2026-11-11). Core, provider, renderer-state and portal-backend
checks pass; desktop, lock-surface and real-frontend acceptance are pending.

## Context

Notify had a single Do Not Disturb toggle, no automatic suppression, delivery as
its only per-application rule, a Center without bulk read/clear or grouping, no
lock-screen presentation, and a portal v2 backend without display hints, button
purposes, replies or a banner timer that kept callbacks live (root ADR 0031).
Settings will gain a Notifications page and needs a control API for all of this.

## Decision

### Do Not Disturb

Effective Do Not Disturb combines three inputs, in this order of reason:
manual (on until turned off), a timer (`for N minutes`, `until` an instant within
31 days, `until tomorrow` = 08:00 local on the next morning, or the coming morning
before 04:00) and a daily schedule (`start`, `end`, optional start days; `end <=
start` crosses midnight, `end == start` is a whole day). Turning it off inside a
scheduled window skips only that window. Toggle and timer state persists in the
daemon store; a timer that runs into the schedule reports the schedule's end.

Schedule windows are resolved to instants per local start date. A start or end in
a DST gap becomes the first valid time after the gap; in a repeated hour the first
occurrence is used, so a window ends once. Evaluation is pure over an explicit
instant and time zone and is tested with a fixed DST zone. A runtime clock task
publishes a state change at every timer/window edge, so the bar and Center follow
without polling. Critical notifications always pass. An optional break-through
rule lets alarms (`alarm`, `alarm.*`) and incoming calls (`call`,
`call.incoming`) pass Do Not Disturb and fullscreen suppression too. An explicit
drop rule still wins.

### Automatic suppression

Service mode follows Niri's IPC event stream. The focused output is fullscreen
when its active workspace's active tiled window has a tile covering the whole
logical output; Niri has no fullscreen flag, and a maximized column beside the
bar is smaller. Any streaming cast in Niri's cast list counts as screen sharing:
it includes Canopa's ScreenCast portal (which records through Niri's Mutter
ScreenCast interface), xdg-desktop-portal-gnome and wlr-screencopy recorders. A
paused cast does not count. A lost IPC connection clears suppression and retries.

Suppressed notifications are saved quietly to the Center (`fullscreen` or
`screen-sharing` decision reason). Critical and break-through ones still show.
While the screen is shared, every banner is presented with its application,
"New notification" and its actions only, including banners already visible, and
the reply composer is withheld. Both suppressions can be turned off.

### Preferences and layers

Per-application preferences are keyed by App Identity (desktop entry first, then
application name, ADR 0002): sound (on/off) and lock-screen visibility
(`hidden`, `name-only` default, `content`). Delivery (banners / Center only /
mute) stays a Notification Rule (allow / silence / drop). Following ADR 0011,
configuration (`do-not-disturb { … }`, `app "id" …`) and the UI-managed store are
separate layers; the UI layer wins, per field for apps and as a whole for the Do
Not Disturb preferences, and clearing it returns to configuration. The control
socket carries typed commands for Settings; see
[the control API](../settings-control-api.md).

### Center

"Mark all read" clears Requires Attention without ending live notifications.
"Clear all" dismisses live notifications, marks everything read and records a
persisted Center watermark: older items that are finished and read are hidden,
while live, unread and newer items stay. Undo, for 30 seconds and once, restores
the previous watermark and the exact unread revisions; dismissals have already
reached their senders and are not undone. Delete Notification History remains the
destructive operation. Grouping by application and per-group collapse are
view-local provider session state, like Settings/Back navigation (ADR 0052): a
section control per app and a collapsed summary row. The reading view's title
names an active Do Not Disturb or suppression; the Center's Settings view starts
with Do Not Disturb choices (off, on, one hour, until tomorrow).

### Lock-screen feed

`lock-screen-subscribe-v1` on the control socket streams a bounded
`LockScreenSnapshot` (`canopa-notify-client`): at most four entries of display
text plus a count. Notify applies the policy before anything crosses: only
critical notifications, and alarm/call notifications when they break through, that
are live or unread; nothing for apps whose rule is `hidden` or whose sender asked
`hide-on-lock-screen`; otherwise "App — New notification", or "App — summary" when
the app's rule is `content` and the sender did not ask to hide content. The
contract is read-only: no IDs, targets, bodies or actions, and the locker sends
nothing back. Canopa Lock renders it as the passive `notifications` widget, enabled
by default in the lock-screen layout. Sender hints are runtime-only and fall back
to the app rule after a restart, which still hides content by default.

### Portal v2 presentation (W15)

- **Banner-hide timer.** Ordinary portal notifications request
  `keep_after_banner`: their server-default display timeout hides the banner but
  leaves the sender lifecycle active, so the binding, callbacks and Center actions
  stay live until the user acts or dismisses, or the application withdraws it.
  Urgent ones still stay on screen. Timed request questions keep their withdrawal
  deadline. This replaces the earlier expiry-on-timeout boundary in ADR 0031.
- **Display hints.** `transient` → a Transient Notification without history;
  `tray` → Center only (`display-hint` reason); `persistent` → resident;
  `hide-on-lock-screen` / `hide-content-on-lock-screen` (both spellings) → lock
  limits; without `show-as-new` a replacement whose banner already hid updates
  quietly (no banner, no sound), with it the replacement is shown as new.
  `transient` with `tray` is rejected. Unknown hints are ignored.
- **SupportedOptions** advertises categories `im.received`, `alarm.ringing`,
  `call.incoming` and button purposes `im.reply-with-text`, `call.accept`,
  `call.decline`, `call.hang-up`, `call.enable-speakerphone`,
  `call.disable-speakerphone`. A known purpose supplies a default label; an
  unlabelled unknown purpose is skipped and a labelled one is an ordinary button.
- **Inline replies.** The first `im.reply-with-text` button opens a composer in
  its banner. The surface takes exclusive keyboard focus only while the composer
  is open, and the banner-hide timer waits. Enter or Send invokes the action;
  Cancel or Escape closes it. The Center's "Reply…" reopens the live banner with
  its composer, so reply text never crosses the Shell. A `ReplyText` sender event
  precedes its `ActionInvoked` the way `ActivationToken` does. The portal adapter
  binds it to that action only and sends it after the target (exported
  `ActivateAction`) or after the target and platform data (`ActionInvoked`). The
  text is never persisted, and diagnostics and broadcasts see an empty
  `ReplyText`. Replies are limited to 4,096 bytes.

## Consequences

- `Summary` gains optional Do Not Disturb reason/end and suppression fields; the
  Shell and Notify ship together, and older summaries still parse.
- Freedesktop clients get no reply composer (no KDE `inline-reply` capability yet).
- Fullscreen detection is a geometry heuristic. A window that fills an output
  without fullscreen and without a bar also counts.
- The installed GTK portal backend still filters v2 fields before Canopa
  (`xdg-notification-portal-v2.md`); these features need Canopa's backend selected.
