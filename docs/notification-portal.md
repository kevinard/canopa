# Canopa Notification portal

Canopa Notify implements `org.freedesktop.impl.portal.Notification` version 2
in Rust, using its existing Iced banners and Notification Center. It shares
Notify's delivery rules, Do Not Disturb, history, sound and captured-revision
action handling. The standard XDG frontend keeps application identification and
notification permission policy.

The backend lives in `canopa-notify daemon --service`, under the additional bus
name `org.freedesktop.impl.portal.desktop.canopa.notify`. Its implementation path
is `/org/freedesktop/portal/desktop`. The dialog backend continues to serve
Settings, FileChooser, AppChooser and Access through its own descriptor.

## Installation and selection

The release bundle includes `canopa-notify.portal` in
`share/xdg-desktop-portal/portals/`, its matching D-Bus activation file in
`share/dbus-1/services/`, and the existing `canopa-notify.service` user unit.
Follow [Notify service setup](notifications.md) and install the descriptor in the
frontend's portal data directory, usually `/usr/share/xdg-desktop-portal/portals`.
For the local companion layout, the activation file belongs in
`~/.local/share/dbus-1/services` and the unit in `~/.config/systemd/user`.
The fallback activation command finds `canopa-notify` through the bus daemon's
PATH; use an absolute executable path there for a different installation layout.

Add this interface entry to the selected configuration's existing `[preferred]`
group:

```ini
org.freedesktop.impl.portal.Notification=canopa-notify
```

Use the existing Niri portal configuration as the basis of a user override.
The frontend selects one configuration file; it does not merge a short override
with the distribution's other entries. See [Settings portal selection](settings-portal.md)
for desktop-name precedence and applying routing changes. Canopa does not edit
this configuration automatically.

## Behavior and current limits

Application ID plus notification string ID identifies a live registration.
Reusing that pair replaces its content; withdrawal closes that registration.
Sender exit leaves it actionable. Portal notifications have no automatic expiry;
this first version leaves their banners visible until user action, dismissal or
withdrawal. Notify's normal daemon-restart contract applies: retained history
becomes stale and live callbacks are invalidated.

Default and button actions retain their original target types. Exported `app.*`
actions use standard application D-Bus activation; other actions return through
the public portal signal. Banner and Center input can supply fresh native tokens.
The generic panel handoff carries activation separately from opaque action values
and revalidates the captured control before sending. Missing or expired context
forwards the valid action without a host token, retaining Notify's existing
provider fallback. Replacements cannot change a committed
callback's target or inherit another invocation's token.

Plain `body` remains literal text. `markup-body` supports bold, italic and safe
links through Notify's body interpreter. Unsupported portal markup cannot load
local body images. Ordered themed-icon candidates and sealed descriptor-backed
PNG/JPEG/SVG icons are supported. Descriptor media becomes owned pixels or sound
bytes before receipt; asynchronous workers do not reopen sender paths. Raw image
pixels are runtime resources, following Notify's existing history contract.
Sound accepts default, silent, or sealed Ogg/WAV descriptor input; actual playback
depends on Notify's audio configuration and backend availability.

Input is bounded: 64 dictionary fields, 63 labelled buttons plus default action,
1 MiB icon input, 8 MiB sound input, bounded target nesting/text, 32 queued
operations and 1024 portal registrations within Notify's shared live-resource
limits. SVG has no external image/font resolution. Categories and button purposes
receive ordinary presentation; `SupportedOptions` is empty. Display hints are
advisory and currently do not override Notify policy. Priority
low/normal/high/urgent maps to low/normal/normal/critical urgency.

## Verification

Build the cooperating services from the workspace root:

```sh
cargo build --locked -p canopa-shell -p canopa-notify --bin canopa-shell --bin canopa-notify
dbus-run-session -- cargo test --locked -p canopa-notify --bin canopa-notify notification_server::portal::tests::
cargo test --locked -p canopa-panel-protocol --test integration v5_sessions::
python3 scripts/test-notification-portal.py --output /tmp/canopa-notification-new --shell-binary target/debug/canopa-shell
```

Use a fresh output directory. The native fixture owns its bus, compositor,
application registrations and input. It checks sender-exit survival, typed
exported/non-exported callbacks, actual native activation consumption,
withdrawal and Center source/serial-bound context. `--scheme light --scale 1.25`
selects the light/scaled check. Python GI is the test caller; the backend and
presentation use Rust/Iced. The fixture does not establish Flatpak sandbox
acceptance or audible playback on physical hardware.

See [Notify ownership](adr/0031-notify-owned-portal-adapter.md),
[panel activation context](adr/0032-panel-action-activation-context.md), and the
[implementation evidence](../.scratch/xdg-portals/IMPLEMENTATION.md) for exact
checks and the current shared-gate status. The upstream
[backend contract](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.impl.portal.Notification.html)
and [public contract](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Notification.html)
define the standard wire interfaces.
