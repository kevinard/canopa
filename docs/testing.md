# Tests and compilation

Ordinary integration cases share an executable named `integration` in each crate.
The existing source files under `tests/` become modules. Files test names start
with their case prefix, such as `cases::transfers::`.

Run the focused Files checks from the workspace root:

```sh
bash scripts/check.sh
bash scripts/check.sh --files --test transfers --test undo_changed_copy
bash scripts/check.sh --files --test transfers copy
bash scripts/check.sh --files --lib operations::fingerprint::receipt_tests
```

The default still selects fingerprint unit tests and the separate `uutils_copy`
executable. Other ordinary Files cases share one compiled integration suite; the
script passes the selected case names as module filters. An optional function
filter is applied within each selected case. Function filtering lists the compiled
suite, then runs the exact matching names using the same binary and build settings.
Module-only selection needs one Cargo invocation. Select `--test integration` to run
the whole ordinary Files suite. Multiple standalone targets require separate
invocations so each receives its own filter.

Direct Cargo examples:

```sh
cargo test --locked -p canopa-files --test integration cases::transfers::
cargo test --locked -p canopa-files --test integration -- cases::transfers:: cases::undo_changed_copy::
bash scripts/test-notify.sh -p canopa-notify --test integration dbus_contract::
cargo test --locked -p canopa-config --test integration themes::
```

Use the same feature selection and incremental debug cache between edits. Direct
`cargo test FILTER` still selects every enabled target before applying the filter.
The workspace has at most 17 integration executables, including the five
standalone Files targets. Library unit tests and programs launched by process-level
tests build separately. The first build links the new suites.

Files keeps `uutils_copy`, `local_actions`, `local_operation_attention`,
`file_clipboard_wayland`, and the custom `device_runtime` executable separate for
focused development and feature-specific native fixtures. Continue using the
maintained native verification scripts for these workflows.

When adding a test source, register its module in that crate's
`tests/integration.rs`. Keep process-global mutations in isolated subprocesses;
subprocess `--exact` filters must include the module prefix. Run
`python3 scripts/test-test-layout.py` to check discovery and the binary budget.

Use `bash scripts/check.sh --workspace` for the full cross-application gate.
The [test compilation decision](adr/0030-consolidated-integration-tests.md)
records the layout and its tradeoffs.

The workspace Rust step uses `--no-fail-fast` so a failing test executable does
not prevent the remaining executables from running. Keep the same workspace and
feature selection when resuming interrupted checks; excluding an application can
change unified dependency features and compile another set of artifacts.
Cargo runs each test harness from its crate directory. When continuing with
already-built executables, use the directory containing that artifact's
`Cargo.toml` as its working directory and `CARGO_MANIFEST_DIR`. Running a harness
from the workspace root can make relative packaging fixtures fail. Preserve the
completed results and record which executables remain unbuilt or unexecuted.

Private desktop and media runs share one read-only Niri executable per SHA-256
under `target/native-test-snapshots/compositor-by-sha256/`. Each run's
`compositor/bin/niri` links to it. Mutable build outputs are copied into that
store once, with checksum verification before publication. Retain logs, receipts,
patches and useful captures; avoid copying whole executables into every run.
An artifact directory containing these links depends on the shared store when
moved or archived. Archive obsolete unique binaries losslessly when exact replay
is still useful; verify the decompressed checksum before removing a raw copy.
