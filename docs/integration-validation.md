# Workspace and dependency integration validation

Local validation on 2026-09-26, Rust 1.95.0, Linux x86-64. The application source
is commit `945e24c`; subsequent changes repair validation/packaging tooling and
documentation. Upstream versions and remaining patches are recorded in
[vendor maintenance](vendor-maintenance.md). Both original histories are ancestors
of import commit `1eab970` and remain on the `legacy/niribar` and `legacy/notify` branches.

Builds used `CARGO_TARGET_DIR=niribar/target`, `CARGO_INCREMENTAL=0`, and
`CARGO_BUILD_JOBS=2` to reuse the existing local cache. Normal workspace builds use
root `target/`. This is debug-build correctness evidence, not release performance
measurement or a hosted CI result.

| Check | Result |
| --- | --- |
| Workspace check, all targets | Passed |
| Combined workspace binary build | Passed |
| Niribar standalone all-target check | Passed |
| Notify standalone GUI all-target check | Passed; one upstream unused-import warning with theme detection disabled |
| Workspace Clippy, all targets, warnings denied | Passed |
| Workspace formatting, shared protocol, contrast, seven updater tests | Passed |
| Workspace tests on a private D-Bus session | 1,626 passed, zero failed or ignored, across 36 test binaries |
| Shared fork library tests | Four Iced layer-shell tests passed, including retired-window cleanup; layershellev library target compiled (no unit tests) |
| Notify core-only dependency graph | GUI and audio adapters excluded |
| Notify core-only all-target check | Passed |
| Notify core-only tests | 336 passed, zero failed or ignored (including one compile-fail doctest) |
| Vendor archive/provenance/patch comparison | Passed against verified published archives |
| Dependency attribution inventory and notice collection | 544 packages; source text, build-directory exclusion, symlink containment and source-archive fallback checked |
| Combined release packager using debug binaries | Passed; all three binaries, Wi-Fi provider handshake/close, current inventory, fork and protocol notices verified in the archive |

Core daemon service-lifecycle and storage-interruption process tests were also
rerun successfully without a competing executable build.

The initial full validation wrapper stopped after successful application tests
because Cargo cannot test an excluded dependency with development dependencies.
The corrected wrapper calls `scripts/test-vendor.py`; that runner then passed
separately. It checks the active registry graph against the root lockfile before
running the copied fork library tests. The upstream source and tracked manifests
are unchanged by the test runner.

Graphical fixtures use disposable compositors, private buses and temporary XDG
paths. Niri was `26.04 (8ed0da4)`; Sway was `1.12`.

- `/tmp/canopa-workspace-desktop-2/result.json`: banner rendering, native AT-SPI
  action delivery to the sender, physical click opening Niribar's Notification
  Center, image fallback and native Center action delivery passed.
- `/tmp/canopa-fork-desktop-3/result.json`: a rendered tray popup requested a real
  input grab and dismissed on an outside click; fractional scale 1.25 and removal
  of every Sway output followed by visible bar restoration passed without
  restarting the shell.
- `/tmp/canopa-placement-2/`: both bars passed top, bottom and left placement at
  scale 1.25. The right edge exposed independent physical-pixel rounding of
  fractional position and extent (114 rather than 115 pixels of clearance).
  The fixture now explicitly permits one physical pixel at fractional scales;
  integer-scale checks remain exact. The final right-edge rerun passed for both bars at
  `/tmp/canopa-placement-right-idle/right/result.json`.

Early graphical attempts during concurrent compilation hit startup/IPC timeouts.
The first popup screenshot also preceded rendering; its fixture now waits for
actual visible frames and checks dismissal occurs after the outside click.
Failures and passing reruns are retained in the corresponding `/tmp` artifact
folders. Tests did not replace live desktop services.

The package smoke archive is
`/tmp/canopa-integration-package/canopa-x86_64-unknown-linux-gnu.tar.gz`.
It contains the full-feature debug binaries used for graphical checks:

| Binary | SHA-256 |
| --- | --- |
| niribar | `e51e6473c083b9e49cd67842fb80571e375d784a371495a38463def61c4eed19` |
| canopa-notify | `9a1edd59159518a72c26d9fe930b2910a40505fe688e6ee73484f6a7e5b71831` |

The existing native accessibility/wire tests and AT-SPI graphical actions passed.
This does not establish Orca acceptance, physical monitor hot-plug behavior,
release-build performance, or hosted CI. Their separate procedures remain in the
application documentation. No release or upstream fork was published.
