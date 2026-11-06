# Integration test consolidation — 2026-10-29

Decision: [ADR 0030](../../docs/adr/0030-consolidated-integration-tests.md).
Usage: [test selection](../../docs/testing.md).

## Target and coverage inventories

| Package | Before | After |
| --- | ---: | ---: |
| Files | 69 | 6 |
| Notify | 29 | 1 |
| Shell | 6 | 1 |
| Portal | 4 | 1 |
| Assets | 3 | 1 |
| Config | 2 | 1 |
| Panel protocol | 2 | 1 |
| Total integration executables | 115 | 12 |

Compared the compiled harnesses' actual `--list --format=terse` output with the
original test-function inventory. All **1,014** test names are accounted for,
with zero missing or unexpected functions. Eleven standard harnesses own those
functions; the custom GIO device harness remains separate and passed its runtime
acceptance checks. The standard integration results total **953 passed, 10 failed,
51 ignored**. Existing ignored native/helper cases retain their fixture gates.

The Files script preserves case selectors and case-local function substring/exact
filters. Tests exercise similarly named modules with an actual Rust harness,
multiple selected cases, substring matches in the middle of names, names containing
the case name, exact matches, empty results, and failure propagation from discovery.

## Build verification

- `python3 scripts/test-check-scripts.py`: 15 passed.
- `python3 scripts/test-test-layout.py`: 3 passed. The initial regression failed
  against the original 115-target layout before consolidation.
- `bash -n scripts/check-files.sh scripts/check.sh`: passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed in
  4m 31s, using the normal incremental debug cache.
- Initial `cargo test --workspace --all-targets --locked` compilation finished
  in 24m 41s. A subsequent identical target/feature selection with `--no-run
  --message-format=json` reused **all 745 artifacts**, with **zero rebuilt
  artifacts**, in 7.80 seconds (Cargo's reported duration: 7.62 seconds).

The target reduction measures link fanout. There is no before/after wall-clock
speedup benchmark. Preserved compiler profiles, dependency features, incremental
compilation, and root `target/` cache selection.

## Workspace outcomes and limitations

The full workspace gate initially stopped at `systemd-analyze verify` because
the sandbox denied required socket options. Resuming that step outside the sandbox
passed the isolated unit-graph and cycle checks. The updater's 19 tests passed,
as did vendor provenance, shared protocol/dependency checks, contrast checks,
desktop-runner inventory checks, and release dependency attribution.

The Cargo run passed all **372 Files library tests** (34 ignored), then stopped
after the consolidated Files integration suite: **596 passed, 10 failed,
41 ignored**. These ten failures match the previously recorded groups in
[CURRENT-FAILURES.md](../file-manager/CURRENT-FAILURES.md) and
[UUTILS-IMPLEMENTATION.md](../file-manager/UUTILS-IMPLEMENTATION.md):

- Recovery catalog: the existing `pages_remain_loaded_and_saved_identity_survives_active_undo_expiry` timeout.
- Undo destinations: the existing 500-item, 20-second gate; this run took 42.14 seconds.
- Undo unavailable moves/saved: the eight existing canonical-history binding failures,
  including the corresponding crash-child expectation.

Continued the remaining **22 already-built harnesses**, skipping the 12 already
run. Executables came from the current Cargo artifact manifest; each used its
package working directory and a private session bus. All remaining integration
suites passed: Notify 276, Shell 29, Portal 4, panel protocol 18, Copy 7, and both
ordinary background Files parents. Assets 9 and Config 12 passed in the first run.

Two untouched native AT-SPI unit fixtures failed on this host:

- `presentation::renderer::tests::accessibility::native_accessibility_exposes_content_actions_and_retires_old_controls`
- `app::canopa_accessibility::tests::native_center_accessibility_exposes_geometry_focus_and_revision_owned_actions`

Both failures reproduced when selected individually through Cargo with the same
workspace features. `/usr/lib/at-spi-bus-launcher` exits with a usage error and
the fixtures fail to establish their private accessibility connection. Other
Notify library tests (154) and Shell executable tests (1,260) passed.

Completed the rest of the gate: Settings frontend activation/ReadOne/ReadAll/reload,
configuration validation/migration/privacy/no-clobber, and the vendor library,
event-batch, and native pipe tests passed. The complete workspace remains red for
the ten recorded Files failures and the two native accessibility fixture failures;
this record establishes scoped consolidation verification.

## Retained evidence

Ignored local logs and inventories are in `artifacts/` beside this record. They
include the original Cargo target/function inventories, compiled test-name inventory,
full workspace logs, isolated AT-SPI reruns, and the current Cargo artifact manifest
with freshness results. Compiled executables were retained in the ordinary build
cache rather than copied into the evidence directory.
