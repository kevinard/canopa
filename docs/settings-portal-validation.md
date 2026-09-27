# Settings portal validation

Scope: [ADR 0008](adr/0008-settings-portal.md), against baseline `24651ce`.
Builds use locked dependencies, `CARGO_TARGET_DIR=niribar/target`,
`CARGO_INCREMENTAL=0`, and one Cargo job. No dependency versions or vendor patches
change. The lockfile adds only the new workspace crate, and its release notices
are included in the attribution inventory.

Focused configuration and backend tests passed. The shared-only intake fixture
verifies common-source precedence, no application legacy fallback, shared
fingerprints and appearance, unrelated feature-schema isolation, strict envelope
and appearance rejection, safe diagnostics and dangling symlinks.

The backend process fixture exercises actual D-Bus signatures and values, namespace
filtering, constant NotFound errors, startup defaults, discovering a newly created
common file, changed signals, no-op reloads, invalid reload retention, restart,
duplicate-owner refusal, invalid startup, SIGTERM and bus loss. Its private bus
contains no installed activation service directories.

The initial readiness probe cached an error from before name acquisition; it now
uses uncached reads. The duplicate-owner regression then exposed zbus's default
AllowReplacement/ReplaceExisting flags. The backend explicitly disables both;
the fixture verifies that the first owner remains authoritative.

The real frontend fixture also passed. A private bus activates the supplied binary
using the packaged D-Bus service file and `.portal` descriptor. Through the installed
`xdg-desktop-portal` 1.22.0 frontend, the fixture checks public `ReadOne` and `ReadAll`, RGB
normalization, an atomic configuration-file replacement, and both public
`SettingChanged` signals. No live desktop bus, configuration or service manager is
used. The systemd graph fixture includes a synthetic D-Bus socket and verifies the
new on-demand service without starting it.

| Review axis | Remaining findings | Resolved finding |
| --- | --- | --- |
| Standards | 0 | Isolate the Rust fixture from installed activation services |
| Spec | 0 | None |

Core validation (`bash scripts/check.sh --core`) passed all 375 tests, with no
failures or ignored tests. The new backend builds without desktop dependencies;
Notify's GUI/audio exclusion check still passes. Full-workspace strict Clippy,
formatting, fork provenance, 42 contrast pairs, seven updater checks, service
ordering and the 552-dependency attribution inventory also passed.

The first full run encountered an intermittent failure in the unchanged Niribar
`second_instance_preserves_first_listener_and_stale_socket_can_recover` fixture:
the recovery bind reported a still-held lock. The fixture passed in isolation.
Concurrent fork inheritance is a possible cause, not a confirmed diagnosis. No IPC
production code, test retries, or concurrency settings were changed for this slice.

The full validation rerun passed with normal test concurrency: 1,687 workspace
tests across 54 executables, the nested AT-SPI verification, and four fork tests
(1,692 reported Rust test passes, no failures or ignored tests). The real frontend
activation/reload fixture and configuration CLI fixture also passed in that run.

Packaging and the release smoke test passed using the final debug binaries. The
archive contains all five executables, portal activation metadata, the on-demand
unit, installation guide and dependency notices. All five executables run, the
bundled common configuration validates, and both provider protocol smoke checks
pass. This validates the archive layout; it is not a release-mode build or a live
desktop installation.

Local evidence:

- `/tmp/canopa-settings-unit3.log`
- `/tmp/canopa-settings-clippy.log`
- `/tmp/canopa-settings-frontend.log`
- `/tmp/canopa-settings-core.log`
- `/tmp/canopa-settings-full.log`
- `/tmp/canopa-settings-ipc-repro.log`
- `/tmp/canopa-settings-full2.log`
- `/tmp/canopa-settings-package.log`
- `/tmp/canopa-settings-package-smoke.log`
- `/tmp/canopa-settings-package/canopa-x86_64-unknown-linux-gnu.tar.gz`
