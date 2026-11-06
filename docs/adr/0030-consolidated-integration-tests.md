# 0030: Consolidate integration tests into shared executables

Status: Accepted, 2026-10-29.

[Cargo discovers a separate executable for every top-level integration-test source](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#integration-tests).
The workspace accumulated 115 such targets, including 69 in Files and 29 in
Notify. A library change can require linking each consumer even when the test
bodies are small. A name filter alone does not reduce the targets Cargo builds.

Disable automatic integration-test discovery in packages with integration cases.
Declare an `integration` target that imports the existing cases as modules. Test
source files and their public API seams remain intact. Notify's shared daemon
and notification helpers compile once in that suite. Shell's desktop-only cases
retain their feature conditions at module declarations.

Files retains separate targets for `uutils_copy`, `local_actions`,
`local_operation_attention`, `file_clipboard_wayland`, and the custom
`device_runtime` harness. This keeps the frequent Copy checks small, preserves
the feature-specific headless/native workflows, and leaves the custom GIO helper
entrypoint intact. The resulting workspace has 12 integration-test executables.

Subprocess fixtures continue to select one helper with `--exact`; helpers in the
shared executable use module-qualified names. Crash tests, resource-limit changes,
private buses, and other subprocess isolation remain in the fixtures. Empty binary
test harnesses for Files and Portal are disabled for ordinary `cargo test` runs;
binaries with unit tests remain enabled.

The Files check script accepts the established case selectors and translates
ordinary case names into libtest module filters. Module-only selection runs in
one Cargo invocation. A function filter first lists the compiled suite and selects
matching case-local names, then runs those full names with `--exact`. Both calls
reuse the same executable and cache; substring matching does not depend on the
new module prefixes or require the function name to start with the filter.
Direct Cargo callers select `--test integration` and pass module filters.
Files uses `cases::CASE::` prefixes so substring matching cannot confuse a case
such as `transfers` with `operation_transfers`.

`scripts/test-test-layout.py` guards the executable budget, checks every case file
is registered exactly once, and rejects unqualified subprocess helper filters.
The workspace gate includes this guard. New test files must be registered in
their suite so automatic discovery cannot silently restore one executable per file.

No compiler profile, dependency feature selection, linker, incremental setting,
or build-cache location changes. Target counts establish the reduced link fanout;
they do not establish a wall-clock speedup. The first run must build the new
suite executables; later edits reuse them. A test-body change recompiles its shared
suite, which trades a larger test crate for fewer links.
