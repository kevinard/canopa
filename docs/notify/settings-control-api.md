# Notification preferences control API

Settings' Notifications page talks to Canopa Notify over the existing per-user
control socket, `$XDG_RUNTIME_DIR/canopa-notify/control.sock` (ADR 0007, ADR 0055).
Each request is one line, `control <json>\n`; the reply is one JSON line,
`{"reply": "<kind>", "value": …}`, or a line beginning `error `. Errors never
carry notification content. Clients reconnect for every request; push updates
come from `summary-subscribe-v1\n` (bar-sized state) and Notify's own Center.

Preferences managed here are the UI layer (ADR 0011). They override
`~/.config/canopa/config.kdl` `notifications` configuration and survive restarts.

## Read everything

```json
{"command": "notification-preferences"}
```

Reply `notification-preferences`:

```json
{
  "do_not_disturb": {"active": true, "reason": "schedule", "until": 1762930800000, "next_change": 1762930800000},
  "do_not_disturb_preferences": {
    "schedule": {"start": "22:00", "end": "07:00", "days": ["mon", "tue", "wed", "thu", "fri"]},
    "break_through": {"alarms": true, "calls": false},
    "suppress_fullscreen": true,
    "suppress_screen_sharing": true
  },
  "do_not_disturb_source": "user",
  "suppression_inputs": {"fullscreen": false, "screen_sharing": false},
  "suppression": null,
  "apps": [
    {"app_identity": "org.gnome.Fractal", "delivery": "silence",
     "configured": {}, "user": {"sound": false, "lock_screen": "content"},
     "sound": false, "lock_screen": "content"}
  ]
}
```

- `reason`: `manual`, `timer` or `schedule`; `until`/`next_change` are Unix
  milliseconds.
- `do_not_disturb_source`: `default`, `configuration` or `user`.
- `days` is omitted for every day. `end <= start` crosses midnight.
- `apps` lists applications with configured or UI-managed preferences or an
  app-wide rule. Settings adds installed applications from the desktop catalog
  itself; any App Identity (desktop-entry ID, else application name) can be set
  before the application has notified.

## Do Not Disturb

```json
{"command": "request-do-not-disturb", "request": {"mode": "off"}}
{"command": "request-do-not-disturb", "request": {"mode": "on"}}
{"command": "request-do-not-disturb", "request": {"mode": "toggle"}}
{"command": "request-do-not-disturb", "request": {"mode": "for", "minutes": 60}}
{"command": "request-do-not-disturb", "request": {"mode": "until", "until": 1762930800000}}
{"command": "request-do-not-disturb", "request": {"mode": "until-tomorrow"}}
```

Reply `do-not-disturb-status` with the `do_not_disturb` object above. `for` takes
1–10,080 minutes; `until` must be within 31 days. `off` inside a scheduled window
skips the rest of that window. `until-tomorrow` ends at 08:00 local time.

Schedule, break-through and suppression replace the UI copy as a whole; `null`
returns to configuration:

```json
{"command": "set-do-not-disturb-preferences", "preferences": {
  "schedule": {"start": "22:00", "end": "07:00"},
  "break_through": {"alarms": true, "calls": true},
  "suppress_fullscreen": true, "suppress_screen_sharing": true}}
{"command": "set-do-not-disturb-preferences", "preferences": null}
```

Reply `command-applied`.

## Per-application rules

```json
{"command": "set-app-preferences", "app_identity": "org.gnome.Fractal",
 "delivery": "silence", "preferences": {"sound": false, "lock_screen": "content"}}
```

The command replaces the UI-managed values for that application:

| Field | Values | Meaning |
| --- | --- | --- |
| `delivery` | `allow`, `silence`, `drop`, or omitted/`null` | Banners; no banners (Center only); mute (discard). `null` removes the app-wide user rule. |
| `preferences.sound` | `true`, `false`, or omitted | `false` never plays this application's sounds. |
| `preferences.lock_screen` | `hidden`, `name-only`, `content`, or omitted | Lock-screen visibility for its critical notifications; `name-only` is the default. |

Omitted preference fields fall back to configuration, then defaults. Category
and urgency rules keep their existing `set-category-permission` command.

## Center operations

```json
{"command": "mark-all-read"}
{"command": "clear-all"}
{"command": "undo-clear", "undo": 42}
```

`clear-all` replies `cleared` with `{"undo": <token>}`; the token is valid for 30
seconds and once.

## Configuration equivalents

```kdl
notifications {
    do-not-disturb {
        schedule from="22:00" to="07:00" days="mon tue wed thu fri"
        break-through alarms=#true calls=#true
        suppress fullscreen=#true screen-sharing=#true
    }
    app "org.gnome.Fractal" sound=#false lock-screen="content"
    rule app="org.gnome.Fractal" outcome="silence"
}
```

## Command line

```text
canopa-notify ctl dnd [status|on|off|toggle|tomorrow]
canopa-notify ctl dnd on --for 60
canopa-notify ctl dnd on --until 08:00
canopa-notify ctl dnd-schedule 22:00 07:00 [--days "mon tue"] [--alarms] [--calls]
canopa-notify ctl dnd-schedule --clear
canopa-notify ctl app org.gnome.Fractal --delivery silence --sound false --lock-screen content
canopa-notify ctl preferences
canopa-notify ctl read-all | clear-all | undo-clear TOKEN
```
