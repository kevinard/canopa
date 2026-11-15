# Testing

Run every command from the workspace root. Compile through `bash scripts/cargo.sh`
(see [building](building.md)) so the free-space guard applies; the check scripts
already do.

## Check gates

`scripts/check.sh` is the entry point:

| Command | When | What it runs |
| --- | --- | --- |
| `bash scripts/check.sh` | default while editing Files | `cargo fmt --check`, then the Files fingerprint unit tests and the `uutils_copy` target |
| `bash scripts/check.sh --files --test CASE [--test CASE…] [FILTER] [-- ARGS]` | focused Files work | the selected Files integration cases |
| `bash scripts/check.sh --files --lib [FILTER] [-- ARGS]` | focused Files work | Files library unit tests |
| `bash scripts/check.sh --core` | broad check without desktop libraries | dependency-boundary guards, headless Shell, Notify, config, CLI, panel protocol, Files and portal cores built and tested with `--no-default-features` |
| `bash scripts/check.sh --workspace` | before a release, after shared-crate, fork or cross-application changes, or when focused tests hint at a wider regression | tooling self-tests, i18n and contrast checks, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, the dependency-attribution check, `cargo test --workspace --all-targets --no-fail-fast` on a private D-Bus, portal smoke tests and `scripts/test-configuration.py` |

The `--workspace` gate also downloads the pinned Niri archive into
`target/fork-archives` when it is missing, so it needs network access the first
time. `scripts/test-check-scripts.py` asserts the exact command lists of the
`--workspace` and `--core` gates; update it together with `check.sh`.

For other crates, run their tests directly:

```sh
bash scripts/test-notify.sh -p canopa-notify --test integration dbus_contract::
bash scripts/cargo.sh test --locked -p canopa-config --test integration themes::
bash scripts/cargo.sh test --locked -p canopa-settings --lib
```

`scripts/test-notify.sh ARGS` wraps `cargo test -p canopa-notify -p canopa-panel-protocol`
in `dbus-run-session`. Tests that talk to D-Bus always use a private session bus.

## Test layout

Each crate has one ordinary integration test executable named `integration`
(`tests/integration.rs`); the other files in `tests/` are its modules. Filter by
module path:

```sh
bash scripts/cargo.sh test --locked -p canopa-files --test integration cases::transfers::
bash scripts/cargo.sh test --locked -p canopa-files --test integration -- cases::transfers:: cases::undo_changed_copy::
```

Files keeps six standalone targets for fixtures that need their own process:
`uutils_copy`, `local_actions`, `local_operation_attention`,
`file_clipboard_wayland`, `device_runtime` and `network_runtime` (the last two host
the GIO helper entry point themselves). The workspace has at most 18 integration
executables; `python3 scripts/test-test-layout.py` enforces the layout and budget.

`check-files.sh` translates `--test CASE` into the `cases::CASE::` module filter of
the shared `integration` executable (or runs a standalone target by name). With a
function filter it lists the compiled suite and runs the exact matching names.
`--lib` and `--test` cannot be combined.

When adding a test file, register it as a module in that crate's
`tests/integration.rs`. Keep process-global mutations in isolated subprocesses;
`--exact` filters for subprocesses must include the module prefix.

## Development practice

- Keep the same features and profile between edits so incremental artifacts in
  `target/debug/` are reused. Do not set `CARGO_INCREMENTAL=0` or use a fresh target
  directory for ordinary checks.
- Direct `cargo test FILTER` still builds every enabled target before filtering;
  select `--lib` or `--test integration` explicitly.
- The workspace Rust step uses `--no-fail-fast`. When resuming an interrupted broad
  check, keep the original workspace and feature selection: excluding an
  application unifies a different feature set and recompiles a different graph.
- Cargo runs each test executable from its crate directory. When re-running an
  already-built test binary by hand, use the directory containing that crate's
  `Cargo.toml` as the working directory and `CARGO_MANIFEST_DIR`.

## Native desktop tests

Some behaviour can only be checked in a real compositor. These harnesses start a
private D-Bus session, XDG directories and a nested Niri, Sway, Weston or Cage, and
never touch your live session. They write logs and screenshots into an output
directory that must not exist yet.

- **Desktop acceptance matrix**: `python3 scripts/run-desktop-acceptance.py --output NEW_DIR`
  runs every scenario in `scripts/desktop-fixture/acceptance.json` (or `--case ID`
  for one; `--list` to list them). It needs `--niri PATH --niri-sha256 HEX` and a
  `--stasis PATH`; binaries default to `target/debug/`. Every `scripts/verify-*.py`
  must appear in the manifest. CI runs it in [Desktop acceptance](ci.md).
- **Per-feature verifiers**: `scripts/verify-*.py` (Shell, launcher, Files,
  greeter, lock, polkit, pinentry, Settings, idle…), run by the matrix or alone.
- **Portal harnesses**: `scripts/test-*-portal.py`, `test-print-*.py`,
  `test-screencast-*.py`, `test-remote-desktop-*.py` and similar exercise the
  portal backend against the forked frontend and compositor. They are run by hand
  when working on that portal.
- **Measurements**: `scripts/measure-*.py` record CPU, memory and copy throughput
  (`measure-files-copy.py` writes to `target/copy-bench/` by default).

[`scripts/desktop-fixture/README.md`](../scripts/desktop-fixture/README.md)
describes the fixtures, compositor selection and requirements in detail, and
`python3 scripts/check-desktop-runner.py` checks that a machine has the tools.

Session rendering changes need native runs with the default backend and with
`ICED_BACKEND=tiny-skia`. Use `verify-greeter.py`, `verify-polkit.py`,
`verify-pinentry.py`, `verify-session-tools.py` and `verify-wallpaper-followups.py`
for login, authorization, PIN, lock and picker coverage, each with a fresh output
directory. The wallpaper followups also authenticate while one monitor stays
powered off through clock updates, then retire and restore the first lock output;
run those checks with both renderers. `ICED_BACKEND=wgpu` requests GPU initialization;
recovery may select software after a setup or presentation failure. Check client
logs as well as test exit status for `session renderer: wgpu` or
`session renderer: tiny-skia`; the same line reports the graphics backend and
adapter. A wgpu run on llvmpipe exercises GPU presentation through a software GPU,
so it does not establish physical GPU/driver acceptance. The static wallpaper
daemon must not initialize an Iced renderer.

For the shared presentation development loop, use
`bash scripts/cargo.sh test --locked -p canopa-session-ui --lib`. Its tests cover
graphics-constructor panic containment, retry wakeups after an earlier deadline,
per-output frame gating, retained image/tree identities, text bounds, fingerprint
typography, RTL clock order and warning placement. These checks complement the
native authentication and output-lifetime fixtures above.

Private runs share one read-only copy of each Niri executable, stored by SHA-256
under `target/native-test-snapshots/compositor-by-sha256/`; do not copy
compositor binaries into every output directory.

Things only a person can verify (real hardware, real servers, Orca on a real
session) are tracked as checklists in the `verification` issues on GitHub.
