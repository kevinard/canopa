# Integrated notification validation

Scope: integration phase 5, [ADR 0004](adr/0004-integrated-notifications.md).
Baseline: `259de76`. Run from the workspace root with Rust 1.95.0,
`CARGO_TARGET_DIR=niribar/target`, `CARGO_INCREMENTAL=0`, and one Cargo job.

The core check completed with **354 tests passed, none failed or ignored**.
The graph check confirms that Notify without default features still excludes GUI
and audio adapters. New tests exercise attention across expiry, acknowledgment,
transient receipt, deletion and restart; persisted DND; coalesced state; the default
session socket; the eight-subscriber limit; bounded frames and malformed replies.
The socket fixture seeds large notification bodies and receives only the count,
DND and daemon instance. CLI toggling and restart are exercised through real
processes. Logs: `/tmp/canopa-notifications-client.log`,
`/tmp/canopa-notifications-summary.log`, `/tmp/canopa-notifications-core.log`.

`scripts/test-session-units.py` validates all three units in an isolated user-manager
dependency graph, including simulated enablement. Executable paths are replaced by
a fixture command because this check validates ordering without installing binaries.
A deliberately contradictory ordering edge must fail verification. No running
service manager is contacted. The earlier review suspicion that the original
ordering itself necessarily produced a cycle was not reproduced; the final units
explicitly order the companions between the graphical pre-session and session
targets.

Both review axes were rechecked after the initial findings were addressed:

| Review | Remaining findings | Resolved areas |
| --- | --- | --- |
| Standards | 0; no remaining material issue | Terminology, unavailable-state documentation, named presentation fields, session ordering verification |
| Spec | 0; no remaining material issue | Configuration migration guidance, default-configuration fixtures, coordinated installation and session environment setup |

The full `bash scripts/check.sh` pass completed with **1,655 workspace tests passed,
none failed or ignored**, plus four local-fork tests and seven updater tests. The
workspace count excludes one nested subprocess rerun of a Niribar test. Formatting,
strict Clippy, fork provenance, the single panel-protocol source, all 42 palette
contrast pairs, the 550-entry attribution inventory, and configuration CLI fixtures
passed. An initial run exposed a registry test's stale expected module list; adding
Notifications to that list was followed by this successful full pass. Log:
`/tmp/canopa-notifications-full-final.log`.

The isolated desktop fixture passed in both dark and light configurations at scale
1.0, with a further light pass at scale 1.25. It runs Niri 26.04 inside headless
Sway 1.12 and uses the real default socket, built-in module, companion lookup and
Center provider without a manual provider declaration. It verifies sender action
delivery from banners and Center, native action focus through AT-SPI, image fallback,
appearance reload with the shell unchanged until restart, right-click DND control,
notification receipt while the shell is stopped, explicit unavailability during
daemon loss, and recovery of two retained attention items and DND after both
independent restarts. Screenshots also confirm the recovered Center has rendered.
Artifacts: `/tmp/canopa-notifications-dark-verified`,
`/tmp/canopa-notifications-light-confirmed`, and
`/tmp/canopa-notifications-light-verified` (scale 1.25).

The keyboard-focus fixture initially failed because each short-lived input process
removed the headless seat's last virtual keyboard before AT-SPI inspected focus.
Keeping a virtual keyboard connected for the fixture lifetime fixed the original
reproduction. The assertion requires a focused button or link, and failure artifacts
retain the accessibility tree and screenshot. No production focus workaround or
temporary debug instrumentation was needed.

Packaging the current debug binaries and running `scripts/smoke-release.sh` passed:
all four executables launch, the bundled configuration validates, the companion
target and guide are present, dependency notices are included, and the Wi-Fi
provider handshake and close complete. The archive is
`/tmp/canopa-notifications-package/canopa-x86_64-unknown-linux-gnu.tar.gz`;
logs are `/tmp/canopa-notifications-package.log` and
`/tmp/canopa-notifications-package-smoke.log`. This verifies the package layout with
debug binaries, not an optimized release build.

No service installation or changes to the user's session are part of this
validation. The isolated unit-graph check does not exercise a live graphical-session
launcher; full Orca acceptance remains a separate manual check.
