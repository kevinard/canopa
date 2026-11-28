# Notifications

Canopa Notify is the desktop's notification service. It implements the
freedesktop Desktop Notifications interface (`org.freedesktop.Notifications`) and
the Notification portal for sandboxed applications, shows banners, keeps a local
history, and provides the Notification Center that opens from the Shell's bell.

In the Canopa session, `canopa-notify.service` starts automatically. Its settings
are in the `notifications` block of `~/.config/canopa/config.kdl` (see the
[configuration reference](configuration.md#notifications)) and on the Settings
**Notifications** page.

## Banners

Banners appear in the bottom-start corner, next to the bell on the default left
bar. Choose another corner with `banners { position "top-end"; }`: `top-start`,
`top-end`, `bottom-start` or `bottom-end` (start and end follow the reading
direction). Banners never take keyboard focus.

- Low and normal urgency banners close after 5 seconds unless the application asks
  for another timeout. Critical banners stay until dismissed. When a notification
  with its own actions (such as **Join call**) closes this way, it stays live in
  the Center, so its actions keep working until you use or dismiss it, or the
  application withdraws it. A notification whose application chose its own
  timeout ends when that timeout passes.
- A banner shows the application icon, summary, body (with basic markup and
  images) and the application's action buttons. Clicking the header runs the
  default action. A single named action appears below the message.
- Applications that support replies get an inline reply field.
- HTTP and HTTPS links in the body open in your browser through the OpenURI portal.
  Opening a link does not dismiss the notification.
- Icons follow the desktop icon theme. `icons theme="…" scale=N` pins a theme and
  icon scale.

## The bell and the Notification Center

The Shell's notifications module shows the number of notifications that still need
attention, including those whose banners have closed. Opening the Center does not
mark anything as read.

| On the bell | Effect |
| --- | --- |
| Click | Open the Notification Center |
| Right-click | Toggle Do Not Disturb |
| `DND` state | Do Not Disturb is on |
| `?` | Notify cannot be reached; the Shell retries |

The Center is 480 pixels wide and fills the screen height. It lists notifications
newest first; **Group by app** groups them by application, each group collapsed to
its newest item with a count, and **Show timeline** returns to the single list.
A dot marks notifications that are still unread; an empty Center says
**No notifications**. **Group by app** and **Settings** are icon buttons in the
heading beside Close (Group by app is accented while on); below the list,
**Mark all read** leads the footer and **Clear all** sits apart at its end.

Each notification offers **Open notification**, **Dismiss**, **Mark as read**,
**Reply…** when supported, its own actions, and **Remove** for finished items.
The application's own actions share the card's width as buttons, the first one
filled, like on its banner. Open, Reply and the
application's own actions appear only while the notification is still live, because the sender can no longer receive them once it has closed or
expired; finished items keep Mark as read and Remove. For the whole list:

- **Mark all read**.
- **Clear all** dismisses and hides everything; **Undo clear** brings it back for
  30 seconds.
- **Clear active notifications** dismisses the live notifications only.

**Settings** in the Center sets the delivery of each application and notification
category shown on the current page (**Show banners**, **Save quietly** or
**Block**) and has **Delete notification history**. Long histories are split into
pages. The Center's title shows when Do Not Disturb or automatic suppression is
active, with quick choices **For 1 hour** and **Until tomorrow**.

To place the bell on another bar, add `"notifications"` to a zone. Any module or
menu can open the Center with `open-provider "notifications"`.

## Do Not Disturb

While Do Not Disturb is on, new notifications are saved to the Center without
banners or sound. Critical notifications still show.

- Turn it on or off from the bell, the Center, Quick settings or Settings, or with
  `canopa-notify ctl dnd`.
- Timed: **For 1 hour**, **Until tomorrow** (08:00), `ctl dnd on --for MINUTES`
  (up to 7 days) or `ctl dnd on --until HH:MM`.
- Daily schedule, optionally on chosen weekdays.
- Break-through: alarms (`alarm.*` categories) and incoming calls (`call.incoming`)
  can be allowed to show.

Notify also suppresses banners automatically, without turning Do Not Disturb on:

- While a window is fullscreen on the focused output, banners are saved quietly.
- While the screen is shared or recorded, banners are saved quietly and their
  content is hidden.

Both are on by default (`suppress fullscreen=#false screen-sharing=#false` turns
them off). Do Not Disturb state, timers and the schedule survive restarts.

## Per-application preferences

| Preference | Values |
| --- | --- |
| Delivery | **Show banners** (`allow`), **Save quietly** (`silence`: Center only), **Block** (`drop`) |
| Sound | on or off |
| Lock screen | `hidden`, `name-only` (default), `content` |

Set them in Settings, in the Center, with `canopa-notify ctl app`, or as `app` and
`rule` nodes in the configuration file. Choices made in the user interface are
stored in Notify's database and kept separately from the file. Configuration rules
can also match a category or urgency; the first matching rule decides. Up to 1024
applications can have preferences.

## Sounds

Notifications play a sound from the freedesktop sound theme, `message-new-instant`
by default, unless the application asks for another sound or for silence:

```kdl
notifications {
    sound enabled=#true theme="freedesktop" default="message-new-instant"
}
```

The Settings **Sound** page has a "Play notification sounds" switch that writes
`enabled`. Silenced, blocked and Do Not Disturb notifications make no sound.

## History

History, unread state, Do Not Disturb, timers and per-app preferences are stored in
`$XDG_STATE_HOME/canopa-notify/history.db` (default
`~/.local/state/canopa-notify/history.db`). Notifications an application marks as
transient are kept only while Notify runs. Remove finished items from the Center,
or erase all inactive history with **Delete notification history** or
`canopa-notify ctl delete-history`.

## Lock screen

The lock screen's `notifications` widget shows critical notifications that arrive
while the screen is locked: up to four lines in the form "App — New notification",
or the summary when the application's lock-screen preference is `content`. With
`hidden`, the application never appears. The lock screen cannot open, dismiss or
act on notifications.

## Command line

`canopa-notify ctl` talks to the running service. It never starts it.

| Command | Effect |
| --- | --- |
| `ctl status [--json]` | Service, configuration, rendering and link-opening health |
| `ctl active [--json]` | Number of active retained notifications, or the list |
| `ctl dnd [status\|on\|off\|toggle\|tomorrow]` | Read or change Do Not Disturb (default `status`) |
| `ctl dnd on --for MINUTES` / `--until HH:MM` | Do Not Disturb with an end time |
| `ctl dnd-schedule FROM TO [--days "mon tue"] [--alarms] [--calls]` | Set the daily schedule and break-through (times `HH:MM`) |
| `ctl dnd-schedule --clear` | Return to the configuration file's Do Not Disturb settings |
| `ctl preferences` | Print Do Not Disturb and per-app preferences as JSON |
| `ctl app APP_ID [--delivery allow\|silence\|drop] [--sound true\|false] [--lock-screen hidden\|name-only\|content]` | Set one application's preferences; omitted options are cleared |
| `ctl read-all` | Mark every notification read |
| `ctl clear-all` | Clear the Center and print an undo token |
| `ctl undo-clear TOKEN` | Undo the last `clear-all` within 30 seconds |
| `ctl clear` | Dismiss the active notifications |
| `ctl delete-history` | Remove inactive history |
| `ctl reload` | Reload the configuration (repeats discovery) |
| `ctl reload-canopa PATH` | Load the `notifications` block of an explicit shared file |
| `ctl reload PATH` | Load a legacy-format file |

`ctl` uses `$XDG_RUNTIME_DIR/canopa-notify/control.sock`; `--socket PATH` (before the
command) selects another. If a command fails after it was sent, its outcome is
unknown: check the state before repeating `dnd toggle` or `clear`. See
`man canopa-notify` for the other modes (`daemon`, `panel-provider`,
`check-config`).

```sh
canopa-notify ctl status
canopa-notify ctl dnd on --for 90
canopa-notify ctl app org.example.Chat --delivery silence --sound false
notify-send "Hello" "Canopa Notify is running"
```

## Running the service

In the Arch package, `canopa-session.target` starts `canopa-notify.service`
(`/usr/bin/canopa-notify daemon --service --socket %t/canopa-notify/control.sock`).
In a local installation, the same unit runs `~/.local/bin/canopa-notify`; see
[installation](installation.md#release-archive).

- Only one notification daemon can run per session. Notify refuses to start when
  `org.freedesktop.Notifications` is already owned; stop the other daemon first.
- Service mode needs a Wayland session with the layer-shell protocol.
- The Notification portal (`org.freedesktop.impl.portal.Notification`, version 2) is
  served by the same process and is selected in the packaged portal routing. See
  [portals](portals.md).
- Logs: `journalctl --user -u canopa-notify.service -b`.

`canopa-notify check-config` validates the `notifications` block without starting
anything; `canopa config check` checks every application.

## Reloading and restarts

Notify does not watch the configuration file. After editing it, run
`canopa-notify ctl reload`; Settings does this after saving. A failed reload keeps
the previous configuration and is reported by `ctl status`. New settings apply to
later notifications; appearance changes repaint open banners.

When Notify restarts, the bell shows `?` until it reconnects. History, unread state
and Do Not Disturb survive; transient notifications and pending application actions
do not. Close and reopen the Center after a restart. If `ctl status` reports that
the banner renderer stopped, notifications are still received and the Center works,
but banners need a restart: `systemctl --user restart canopa-notify.service`.

Database upgrades are one-way. Before going back to an older version, restore a
backup of `~/.local/state/canopa-notify` taken while Notify was stopped
(`./scripts/update.sh` makes one).

## Known limitations

- The configuration file is not watched; reload with `canopa-notify ctl reload`.
- The lock screen shows only critical notifications.
- A stopped banner renderer needs a service restart.
