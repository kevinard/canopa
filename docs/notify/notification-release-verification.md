# Notification release verification

Recorded 2026-09-16. Implementation starts at Notify `8898be0` and Niribar
`05e6e5f`; the paired host implementation is committed as `e4280a1`. The selected
static Desktop Notifications 1.3 profile is implemented. The compatible host
uses Canopa protocol v4. This record distinguishes source verification, actual
desktop evidence, and environment limits.

The [2026-09-21 completion follow-up](project-completion-verification.md) records
subsequent failure-injection coverage, live installation, and the remaining hosted
CI gate. Counts and machine measurements below retain their 2026-09-16 scope.

## Automated checks

| Check | Result |
| --- | --- |
| Notify default-feature private-bus workspace suite | 430 passed, 0 failed, 0 ignored |
| Notify core-only private-bus workspace suite | 331 passed, 0 failed, 0 ignored |
| Compatible Niribar complete suite | 1,155 passed, 0 failed, 0 ignored |
| Notify Clippy, all targets, `-D warnings` | Passed |
| Workspace and host formatting; whitespace | Passed |
| Core dependency graph | Excludes GUI/audio adapters, including dev/build edges |
| Canonical panel protocol versus applied host snapshot | Exact match |
| Independent Standards and Spec review | Zero outstanding findings on each axis |

Commands: `bash scripts/test-notify.sh`, `bash scripts/test-notify.sh --no-default-features`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`, and
`dbus-run-session -- cargo test --manifest-path /path/to/niribar/Cargo.toml --locked --offline`.
Niribar retains its pre-existing lint warnings; the clean Clippy gate applies to
Notify's workspace. Native tests exercise the actual private AT-SPI interfaces,
wire cache signatures and generation-owned callbacks. Private portal fixtures
cover early/alternate response handles, cancellation and failures.

The final applied host files were byte-compared with the tested staging tree.
Only its temporary enclosing-workspace escape stanza was omitted. No live desktop
service, running bar or installed user configuration was changed.

Two earlier full runs stopped: configuration startup/FIFO deadlines failed while
a release compile was competing for resources, then an old `Mail` section-ID
assertion failed after the intentional `app/Mail` change. The unchanged
configuration file passed in isolation and in the final full run; its exact
initial timeout cause was not established. Host integration fixtures still using
v3 were updated to v4 and now reject both v3 and v5. A new test-only Clippy finding
was corrected. No timeout, assertion or warning gate was weakened.

## Release build and installation

A source export with its own initially empty Cargo target built using Rust 1.95.0,
`--release --locked --offline`; the minimum version is pinned in workspace metadata.
An installed older stable toolchain (1.94.1) could not build locked KDL 6.7.1.
The first long compilation was interrupted before completion, then resumed with
the same target. The export was refreshed and the final code/manifests compared
against the working checkout. No developer target or configuration was copied
into the export. This is a verified local source-export build, not a claim that
hosted CI has run or that cross-platform binaries were produced.

The staged installation at `/tmp/canopa-release-install` contains the optimized
binary, example configuration, manual, user unit and root licenses. Binary help,
manual rendering with `groff`, and `systemd-analyze --user verify` pass. Verification
substituted the staged executable path and supplied its MANPATH; the shipped unit
uses `%h/.local/bin/canopa-notify`. Real service startup/shutdown and migrations
are covered by the executable suite and disposable desktop checks, without
starting the unit in the user's live session.

Release binary SHA-256: `7256a64ad9fc377b2835f4beb649464cbb22d690a7f82194686449412a8fbdc3` (38,367,760 bytes).
The same binary was used for the following measurements. Root and vendored
licenses/provenance and [resolved dependency attributions](dependency-attributions.md)
are included; the [README](../../crates/canopa-notify/README.md) covers installation, upgrade, diagnostics
and removal.

## Runtime baseline

Machine: Intel(R) Core(TM) i5-6200U CPU @ 2.30GHz, 4 logical CPUs, Linux-7.0.12-arch1-1-x86_64-with-glibc2.43.
Three runs started fresh private buses and XDG state, warmed up 32 transient
requests, then issued 1,000 transient and 1,000 retained notifications per run.
Each notification had a 4 KiB body and was followed serially by
`CloseNotification`. Receipt latency includes client/bus/daemon work; the retained
path includes SQLite. This is not a burst, graphical-frame or audio benchmark.

| Workload | Median range | p95 range | p99 range | Final RSS range |
| --- | --- | --- | --- | --- |
| Transient | 1.71–1.99 ms | 2.26–2.66 ms | 2.60–3.11 ms | 14.17–14.31 MiB |
| Retained | 8.50–9.46 ms | 13.48–13.56 ms | 14.54–14.81 ms | 17.49–18.29 MiB |

SIGTERM exited successfully in 3.27–3.37 ms, released the control socket and left
zero timeout registrations and no sender-publication error in all three runs.
Raw measurements: [run 1](evidence/notifications-2026-09-16/runtime-1.json),
[run 2](evidence/notifications-2026-09-16/runtime-2.json),
[run 3](evidence/notifications-2026-09-16/runtime-3.json).

Initial regression investigation thresholds on an equivalent idle machine are
2× the worst measured p95 (5.32 ms transient, 27.12 ms retained), and 1.25× the
worst observed process high-water memory (22.9 MiB). These margins are derived
from this small baseline and trigger investigation, not a universal performance
promise. Rebaseline deliberately when workload, hardware, features or toolchain
change. Reproduce with `scripts/measure-runtime.py` after builds finish.

## Desktop and capability acceptance

[Actual compositor and Orca evidence](notification-desktop-acceptance.md) records
ordered images/fallback, all actions, narrow layouts, safe links, activation context,
first/replacement announcements and Center keyboard reading. The repeatable
`scripts/test-desktop.py` additionally creates Niri/Sway plus a private bus, invokes
both native banner and Center actions, verifies two actual sender callbacks, and
captures the resulting desktop. Its dedicated manually dispatched CI workflow
requires a provisioned `canopa-desktop` runner and compatible host checkout; the
local pass does not establish a hosted run. Orca is a separate recorded manual pass.

The capability set includes `body-images` and `persistence`; `body-hyperlinks`
depends on runtime OpenURI availability. Sound and activation-context advertisement
retain their availability checks. The full [compliance matrix](freedesktop-notification-compliance.md)
records exact interfaces, hints, policies and evidence.

## Explicit profile limits

- Classic Notifications 1.3 and the documented portal bridge are supported.
  A native Notification portal v2 backend, inline replies, snoozing and grouping
  remain separate product work.
- Static local PNG/JPEG and bounded theme resolution are selected. SVG/XPM,
  animated icons, automatic desktop theme tracking and arbitrary vendor hints
  are excluded. The [format assessment](static-icon-completion-research.md)
  explains the choices.
- Claimed app identity supports convenience rules, not authenticated permissions.
  Renderer reconnection uses explicit daemon restart. Abrupt migration/import
  interruption and SQLite busy-error injection were not exercised in this initial
  recording; see the completion follow-up for the later tests. General power-loss/filesystem
  certification, every compositor/device/locale, and compositor occlusion feedback
  are not claimed by these fixtures.
- The Center requires the matching Niribar host and v4 configuration. Banners
  expose all accepted named actions through their own bounded scrolling surface.
  Links require a working desktop OpenURI portal.
