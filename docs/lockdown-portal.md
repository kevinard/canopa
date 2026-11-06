# Canopa Lockdown portal

The Rust Lockdown backend exposes persistent privacy switches to portal
frontends. It uses the standard seven read/write boolean properties. These
switches control participating portal operations; they are runtime policy
owned by the portal module.

| Property | Participating operation |
| --- | --- |
| `disable-printing` | Printing and print preparation |
| `disable-save-to-disk` | SaveFile and SaveFiles |
| `disable-application-handlers` | Opening URIs, files and directories through app handlers |
| `disable-location` | Location access |
| `disable-camera` | Camera access |
| `disable-microphone` | Microphone policy available to consumers |
| `disable-sound-output` | Sound-output policy available to consumers |

Each consumer determines its supported enforcement. The installed frontend
1.22.1 directly consults the first five switches. The interface has no version
property. Lockdown itself presents no dialog; enabled operations continue to use
the existing Rust/Iced portal dialogs.

## Build and select

```sh
cargo build --locked -p canopa-portal
```

Use the default desktop features and follow the
[installation guide](settings-portal.md#installation-and-backend-selection) for
the binary, activation service, user unit and descriptor. Add this entry to the
existing `[preferred]` group in the session's selected portal configuration:

```ini
org.freedesktop.impl.portal.Lockdown=canopa
```

For Niri this is normally `~/.config/xdg-desktop-portal/niri-portals.conf`.
Preserve the other entries: portal configuration files are selected as a whole.
Building and testing do not change the live session's routing or privacy policy.

## Read and change policy

Read the complete snapshot through the standard Properties interface:

```sh
gdbus call --session \
  --dest org.freedesktop.impl.portal.desktop.canopa \
  --object-path /org/freedesktop/portal/desktop \
  --method org.freedesktop.DBus.Properties.GetAll \
  org.freedesktop.impl.portal.Lockdown
```

For example, disable portal camera access:

```sh
gdbus call --session \
  --dest org.freedesktop.impl.portal.desktop.canopa \
  --object-path /org/freedesktop/portal/desktop \
  --method org.freedesktop.DBus.Properties.Set \
  org.freedesktop.impl.portal.Lockdown disable-camera '<true>'
```

Set it to `<false>` to enable that operation again. A successful changed write
commits before its standard PropertiesChanged signal and method reply. No-op
writes on a writable source succeed without a signal. Each GetAll reads one
coherent snapshot. A
caller disconnect does not undo an already admitted, committed change.

## Persistence and source selection

The default database is
`$XDG_STATE_HOME/canopa/portal/lockdown.sqlite3`, falling back to
`$HOME/.local/state/canopa/portal/lockdown.sqlite3`. Empty or relative XDG state
homes are ignored; HOME must be absolute. Missing state uses false for all seven
switches. Startup, reads and unchanged default writes create no state files.
The first changed write creates private directories and a private database.

Use `--lockdown-policy /absolute/path/lockdown.sqlite3` to select another policy
database. `--lockdown-read-only` requires an existing valid source and publishes
its values while rejecting writes
with AccessDenied. An unwritable selected file also rejects mutation. The
`--defaults` flag selects embedded appearance and leaves runtime policy active.
Policy writes do not rewrite the shared Canopa KDL file.

The database has a versioned, validated schema and a 1 MiB source-size limit.
One worker admits at most 16 queued operations. Immediate SQLite transactions
serialize cooperating writers and retain other switches. SQLite lock waits are
limited to 250 ms. Filesystem calls and an active SQLite operation cannot be
preempted; these are logical resource bounds rather than hard I/O deadlines.

SIGHUP reloads the selected policy and appearance independently. A rejected
candidate preserves that module's last valid snapshot. Source errors never
replace restrictive values with false. Replacement or removal requires a valid
explicit reload before mutation. Read-only sources can still be reloaded.

Use the D-Bus interface for ordinary changes. A maintenance tool updating the
database through SQLite transactions must request SIGHUP afterward. Direct file
replacement during an active transaction is unsupported; identity checks detect
observed replacement and do not provide filesystem-wide compare-and-swap.
The backend refuses malformed, foreign-schema, unsupported-version, nonregular,
symlinked or oversized sources. Diagnostics contain no SQL or policy contents.

Normal shutdown closes admission and drains accepted work while publication is
available. A signal-delivery failure cannot roll back a transaction that already
committed. A later Get or restart can still observe that durable value even when
the initiating method reports a publication error.

## Validation

```sh
cargo test --locked -p canopa-portal --lib --test integration lockdown
python3 scripts/test-lockdown-portal.py \
  --binary target/debug/xdg-desktop-portal-canopa \
  --output /tmp/canopa-lockdown-dark --shipped-descriptor
python3 scripts/test-lockdown-portal.py \
  --binary target/debug/xdg-desktop-portal-canopa \
  --output /tmp/canopa-lockdown-light --color-scheme light --scale 1.25 \
  --shipped-descriptor
```

The private fixture exercises real frontend cache updates: it blocks saving and
handlers, restores the Iced save chooser and a disposable recording handler,
restarts the backend, rejects a bad source reload, accepts a valid transaction
reload and refuses an unwritable source. It uses private state, applications,
D-Bus and nested Niri. No live user policy or GNOME settings are changed.
Dependencies match the [FileChooser fixture](file-chooser-portal.md#validation).

See [ADR0036](adr/0036-lockdown-policy-owner.md) and the
[implementation ledger](../.scratch/xdg-portals/IMPLEMENTATION.md) for ownership
and actual validation outcomes.
