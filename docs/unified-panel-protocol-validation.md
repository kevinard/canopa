# Unified panel protocol validation

Validation against baseline `e062bdd`, scoped by
[ADR 0007](adr/0007-unified-panel-protocol.md). Builds use locked dependencies,
`CARGO_TARGET_DIR=niribar/target`, `CARGO_INCREMENTAL=0`, and one Cargo job. No
dependency versions or vendor patches change in this slice.

The new shared contract tests cover compatible buffered edits, retirement on
operation changes, incompatible revision reuse, control kinds, disabled entries,
custom-input permission, section options, item actions and links, opaque domain
values, frame bounds, truncation, EOF, invalid versions, and sync/async parity.
Failed oversized replacements leave the current interaction contract intact.

The Wi-Fi session fixture sends two password edits and submission together and
verifies that one connection attempt receives the last value. The host fixtures
check stable control identity across compatible echoes and reject late events from
replaced content or another session. Subprocess fixtures exercise the shipped
provider's v1 handshake and provider-first v5 response and close. Configuration
fixtures retain the legacy default and require explicit selection for v5 overrides.

Notify's process fixtures exercise both wire versions against a real private
daemon, preserving every item and body in paginated large history. A controlled
socket fixture verifies that only the current revision reaches the daemon and that
v5 Close and stdin EOF release the view and exit successfully. Existing daemon
instance, notification-revision, page and resource guards remain covered.

| Review axis | Remaining findings |
| --- | --- |
| Standards | 0 |
| Spec | 0 |

Core validation (`bash scripts/check.sh --core`) passed all 373 tests, with no
failures or ignored tests. The core dependency graph excludes GUI/audio adapters;
the protocol's default graph remains Serde/JSON only. Process fixtures ran on a
private D-Bus session outside the filesystem sandbox.

Full validation (`bash scripts/check.sh`) passed all 1,685 workspace tests across
51 executables, with no failures or ignored tests. The count excludes one nested
accessibility rerun and four fork tests, which also passed. Formatting, strict
Clippy, 42 contrast pairs, seven updater checks, service ordering, configuration
fixtures, fork provenance and the 551-dependency attribution inventory passed.

Packaging and release smoke checks passed. All four bundled executables run, and
the packaged Wi-Fi provider completes both the default v1 handshake/close and the
explicit v5 response/close paths.

The private desktop fixture passed on real nested Sway/Niri compositors with dark
appearance at scale 1.25. Through the built-in v5 launch path it verified Center
opening, keyboard focus, scrolling to and invoking the fourth action, banner action
delivery, shared image pixels, indexed banner icons, shell SVG icons, DND, appearance
reload, independent shell/daemon restarts and retained attention. The captured
desktop image was also inspected. Full Orca assessment and physical Wi-Fi hardware
acceptance remain outside this automated coverage. No live user configuration or
services changed during validation.

Local evidence:

- `/tmp/canopa-v5-protocol.log`
- `/tmp/canopa-v5-wifi.log`
- `/tmp/canopa-v5-core.log`
- `/tmp/canopa-v5-full.log`
- `/tmp/canopa-v5-package.log`
- `/tmp/canopa-v5-package-smoke.log`
- `/tmp/canopa-v5-package/canopa-x86_64-unknown-linux-gnu.tar.gz`
- `/tmp/canopa-v5-desktop.log`
- `/tmp/canopa-v5-desktop/`
