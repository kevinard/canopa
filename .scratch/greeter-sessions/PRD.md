# Installed desktop session discovery

Status: implemented; focused and native discovery acceptance passed

Discover installed Wayland sessions by default from fixed system directories.
Keep manual definitions as ID overrides and allow disabling discovery. Support
configured directory precedence and optional X11 discovery with an explicit
X-server/session wrapper. Parse desktop names, visibility, TryExec, localized
labels and Exec arguments/field codes without executing commands during discovery.
Preserve bounded, quoted greetd handoff and provide a list-sessions diagnostic.
Handle an empty catalog without authenticating or inventing a nonexistent command.
Keep schema validation host-independent and take one catalog snapshot per frontend
startup, with no polling. Test isolated descriptors and real native selection/
handoff under both kiosk compositors. Do not modify host login configuration.

## Result

`Config::load` validates only schema/manual definitions. Frontend startup, preview
and `list-sessions` resolve a bounded catalog from system Wayland session desktop
entries. No user XDG data directories are consulted. Manual IDs override detected
IDs and keep their configured order; remaining entries sort by name/ID. Discovery
can be disabled with `discover-sessions #false`. Ordered `session-dirs` and
`xsession-dirs` support system layouts; X11 requires an explicit `x11-wrapper`.

Desktop parsing honors localized names, main-group isolation, duplicate rejection,
Hidden/NoDisplay, unsupported launch modes, TryExec, desktop quoting/field codes,
working directory and DesktopNames/XDG environment. Executables are checked,
resolved through absolute PATH components, and never executed during discovery.
Normal sources require root ownership and non-writable ancestor chains; explicit
development configuration retains its existing ownership exception. File scans
are nonblocking and bounded, including nonregular/oversized/invalid-UTF8 entries.
Both manual and discovered commands use the same 8 KiB handoff validation.

Empty catalogs stay visible without starting authentication. The selected catalog
is a startup snapshot with no polling, and greetd still exclusively owns PAM and
session launch. The packaged greeter config enables discovery rather than listing
Canopa manually; installing its bundled system desktop entry exposes Canopa.

## Validation (2026-11-01)

- `cargo test --locked -p canopa-greeter --lib --test configuration --test
  compositor --test sessions --test greetd_transport`: **19 passed**, one
  transport acceptance intentionally ignored in ordinary Cargo. Includes literal
  shell-boundary execution in a harmless fixture, argument/field-code rejection,
  locale fallback, visibility/directory masking, command-free TryExec, path lookup,
  manual overrides, empty catalog, X11 wrapper construction, resource limits and
  user-XDG isolation. See `focused-final.log`.
- Strict workspace/all-target Clippy, formatting and diff whitespace checks:
  passed. Initial KDL-v1 boolean spelling in a new test was corrected to KDL-v2
  `#false`; the initial lint's collapsible condition was fixed. Earlier outputs
  remain in `focused.log` and `clippy.log`.
- Product-launcher native matrices: **24 passed under Cage** and **24 under
  Weston**, using fixture-owned discovered desktop entries. Password/OTP/device
  flows, user/session keyboard and pointer actions, exact discovered argv/XDG env
  handoff, cancellation/retry/failure behavior, scoped widgets, and no
  authentication with an empty catalog passed. See `cage-native/summary.json` and
  `weston-native/summary.json`.
- Actual system discovery through `canopa greeter --defaults list-sessions`
  detected Niri and Weston with absolute commands and XDG session environment;
  `system-catalog.json`. This check ran outside the filesystem sandbox because
  its ownership mapping exposes system root as nobody; the normal root-ownership
  policy correctly rejects that mapped view. No policy bypass was added.
- Packaged discovery configuration validates; Python fixture compiles and the
  native acceptance manifest validates. Documentation staging: **38 files, 56
  offline links resolve**. Normal root debug/incremental builds were retained.

Only the greeter implementation, its acceptance fixture, packaging example and
documentation changed. The full workspace test gate was not repeated; existing
Files recovery/host AT-SPI failures remain recorded in `.scratch/greeter/PRD.md`.
No shared dependency/fork or authentication transport changed. Actual X-server
launch/seat setup and arbitrary installed desktops require target-machine
acceptance; optional X11 argv/environment construction is covered here.

The prior compositor benchmark predates discovery. Its original configuration is
now archived with the existing data and matches the recorded SHA-256; the report
states that historical scope. No fresh performance numbers are claimed.

The host display manager, installed session files and PAM were not modified.
