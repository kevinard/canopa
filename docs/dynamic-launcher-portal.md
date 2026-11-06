# Canopa DynamicLauncher portal

The Rust/Iced backend asks the user to review an application or web-app launcher
before installation. It displays the requesting app, icon and name, plus the
target URI for a web app. The name is editable when the caller permits it.
Only Install approves the request. Opening the dialog or pressing initial Enter
does not approve it.

## Build and select

```sh
cargo build --locked -p canopa-portal
```

Use the default desktop features and follow the
[installation guide](settings-portal.md#installation-and-backend-selection) for
the binary, activation service, user unit and descriptor. Add this entry to the
existing `[preferred]` group in the session's selected portal configuration:

```ini
org.freedesktop.impl.portal.DynamicLauncher=canopa
```

For Niri this is normally `~/.config/xdg-desktop-portal/niri-portals.conf`.
Preserve the session's other entries: portal configuration files are selected
as a whole. These build and validation commands do not install Canopa or change
the live session's routing.

## Review and ownership

The backend implements version 1 and supports Application and Webapp launcher
types. It honors `modal` and `editable_name`. The icon remains read-only, including
when `editable_icon` is requested; editing icons is optional in the backend
contract. The original approved PNG, JPEG or SVG bytes are returned unchanged.
This preserves the format information retained by the standard frontend.

The frontend owns application identification, sender-scoped expiring tokens,
desktop-entry validation, installation, launching and uninstallation. The backend
does not write desktop entries or retain install tokens. It denies every
`RequestInstallToken` policy query because Canopa has no trusted-installer policy
yet. The installed frontend can bypass that backend query for an unidentified
host process; this policy is therefore not a system-wide prohibition on
unattended host installation.

Cancel, Escape, window close, owner Request.Close and caller loss return no
approval. Helpers share the existing pool of 16 interactive requests; independent
drafts cannot change one another. A helper cannot return an empty or invalid name
or change a name that was fixed by the caller. Foreign Wayland parenting has the
same optional compositor modal-hint limits described in the
[FileChooser guide](file-chooser-portal.md).

## Bounds and presentation

The backend admits at most 1 MiB of encoded icon data per request. PNG/JPEG
dimensions are limited to 512×512; SVG logical dimensions to 4096×4096. At most
four synchronous preparation workers can run, with their permits retained even
after cancellation. An active codec or vector parser cannot be preempted.
These are logical resource limits, not a hard process-memory or CPU-time quota.

The helper receives only a preview of at most 128×128 pixels through the bounded
anonymous-pipe transport. It uses shared Canopa appearance, fonts and controls.
The icon is shown on a neutral white tile in both appearances. SVG preparation
does not load external images, fonts or files. Original bytes remain authoritative
in the backend and are never replaced by preview pixels.

Names are limited to 1024 bytes and exclude control characters; accepted names
must contain non-whitespace text. Application and parent identifiers are limited
to 4096 bytes, target URIs to 16 KiB. Oversized, malformed or unsupported input
cannot produce approval. Known options require their specified D-Bus types;
unknown options are ignored.

## Validation

```sh
cargo test --locked -p canopa-portal --lib --test integration dynamic_launcher
python3 scripts/test-dynamic-launcher-portal.py \
  --binary target/debug/xdg-desktop-portal-canopa \
  --output /tmp/canopa-launcher-dark --shipped-descriptor
python3 scripts/test-dynamic-launcher-portal.py \
  --binary target/debug/xdg-desktop-portal-canopa \
  --output /tmp/canopa-launcher-light --color-scheme light --scale 1.25 \
  --shipped-descriptor
```

The native fixture uses a private D-Bus, public portal frontend, nested Niri,
registered host identities and disposable data directories. Only its recording
executable can be launched. It checks edited/fixed names, icon previews and exact
PNG/JPEG/SVG bytes, token scope and replay rejection, real publication and removal,
unattended denial, cancellation and parenting. This is host frontend acceptance;
an actual Flatpak installation remains a separate environment.

Dependencies match the [FileChooser fixture](file-chooser-portal.md#validation).
Results and traces remain in the new output directory. See
[ADR0035](adr/0035-dynamic-launcher-review.md) and the
[evidence ledger](../.scratch/xdg-portals/IMPLEMENTATION.md) for actual outcomes.
