# Canopa Background portal

The Rust Background backend observes application windows through Niri and presents
questions through Canopa Notify. Choosing a notification's main action opens an
Iced review dialog. Automatic observation does not open a toplevel window.

Background is advertised in the packaged descriptor following scoped native
acceptance. See the [implementation ledger](../.scratch/xdg-portals/IMPLEMENTATION.md)
for exact checks, environment limits and the broader workspace gate outcome.

## Build and session requirements

```sh
cargo build --locked -p canopa-portal -p canopa-notify --bins
```

Use the default desktop features and install both applications' activation files
as described in the [Settings guide](settings-portal.md#installation-and-backend-selection)
and [Notification guide](notification-portal.md). Background requires Canopa
Notify's request-expiry capability; another notification backend cannot provide
its private question transport. This does not require changing the public
Notification portal selection.

Add this entry to the session's selected `[preferred]` portal configuration,
preserving its existing selections for other interfaces:

```ini
org.freedesktop.impl.portal.Background=canopa
```

For Niri this is normally `~/.config/xdg-desktop-portal/niri-portals.conf`.
Configuration files are selected as a whole; follow the Settings guide when
installing or selecting a backend.

The backend needs the current session's `NIRI_SOCKET` in its D-Bus activation
environment. Niri's session setup imports it. A missing socket, incomplete or
stale observation, or unresolved live window identity makes `GetAppState` fail;
the backend does not substitute an empty application list. The frontend then
defers that observation pass.

## Decisions and ownership

- Allow requests permission for background runs.
- Allow once keeps the current instance without changing its permission.
- Block refuses background operation and can terminate the current instance.
- Cancel, Escape, window close, caller loss and presentation failure use the
  instance-only outcome. They do not persist permission or request termination.

The standard XDG frontend owns permission storage and instance termination. Its
existing Ask policy can remain Ask after a decision. Public `RequestBackground`
consent uses the Iced Access backend when required. The frontend also validates
and writes autostart files; the deprecated backend `EnableAutostart` method
returns NotSupported. This was verified with the installed frontend 1.22.1.

Questions are runtime-only notifications, expire after twenty seconds and are
withdrawn on completion or cancellation. Their expiry is owned by Notify and
also works under Do Not Disturb or after an abrupt portal-backend exit. Ordinary
portal notifications retain their existing lifetime. The review helper consumes
only the activation token from its own input action. Initial Enter chooses no
permission, and a backend exit terminates its helper.
Cold Notify startup has a separate ten-second setup allowance within the overall
request lifetime; ordinary notification RPCs retain a two-second deadline.

## Application observation

The observer waits for a complete Niri window snapshot, follows its event stream
and reconnects after disconnection. It aggregates focused/running state only for
verified Flatpak metadata identities. It excludes verified host windows and
fails when a window's identity cannot be resolved. A client-declared Wayland app
ID is not used as sandbox identity.

Process roots, ownership, birth identity and bounded regular `.flatpak-info`
metadata are pinned and revalidated. The projection expires after two seconds
without a fresh verified observation. Limits bound windows, frames, metadata,
queued work and interactive requests. Niri currently supplies numeric connection
PIDs rather than compositor-issued process handles. The initial PID association
and inherited Wayland connections therefore retain the limitations described in
[ADR0034](adr/0034-background-observation-and-decisions.md). Other sandbox formats
and inaccessible process namespaces remain unsupported observations.

## Validation

```sh
cargo test --locked -p canopa-portal --lib --test integration
python3 scripts/test-background-portal.py \
  --binary target/debug/xdg-desktop-portal-canopa --notify target/debug/canopa-notify \
  --output /tmp/canopa-background-dark --shipped-descriptor
python3 scripts/test-background-frontend.py \
  --binary target/debug/xdg-desktop-portal-canopa --notify target/debug/canopa-notify \
  --output /tmp/canopa-background-frontend --shipped-descriptor
```

Native fixtures use private Niri/Weston, D-Bus, Notify, configuration and permission
stores. Frontend acceptance creates real bubblewrap namespaces with synthetic
Flatpak metadata/instance records. Block can terminate only the disposable child
identified by that fixture. No command from an autostart file is launched. These
checks establish the installed frontend path; a real Flatpak installation remains
a separate acceptance environment. The observer fixture accepts the current
compiled `integration` test executable through its `--harness` option. See the
[FileChooser guide](file-chooser-portal.md#validation) for native dependencies;
the observer/frontend fixtures additionally require bubblewrap.

Use a fresh output directory for each run. Light appearance and fractional scale
are selected with `--color-scheme light --scale 1.25`. These commands do not
install services or alter the desktop session's portal routing.
