# Audit remediation — 2026-09-23

This follows [the original audit](2026-09-23-project-state.md), against baseline
`0dbd2697f79211bb8ab6900037cce2f333dcd52d`. The original report is retained as a
historical snapshot, rather than rewritten to describe the repaired code.

## Runtime and completeness findings

| Finding | Change and regression evidence |
|---|---|
| S1: terminal subscriptions | Bounded reconnect backoff for data sources and Niri; transport-owned bus reconnection; cancellation-aware PulseAudio startup/teardown. Private UPower service absence/restart and Niri socket absence/EOF tests. Bus-loss messages immediately invalidate dependent state. |
| S2: stale tray item properties | Per-item signal subscriptions refetch icon/status/tooltip state. Private D-Bus test changes an icon twice. |
| S3: ghost tray items | Owner loss removes all matching owner/path registrations from the host and built-in watcher. The private-bus test verifies both views. |
| S4: one-shot menu changes | Menu subscriptions retain their signal stream across layout and property updates. Repeated-signal regression. |
| S5: stale workspace children | Niri unavailability clears compositor cache and dynamic workspace children. Populated-state regression. |
| S6: serialized IPC | Bounded concurrent client tasks keep toggles and second selections responsive while the first selection waits. Socket-level regression. |
| S7: unbounded IPC reads | Parse frame lengths before allocation; bound legacy requests and CLI input/response reads. Oversized-header regression. |
| S8: active socket removal | Per-path advisory ownership lock, active-listener probe, and inode-aware cleanup. Competing/stale listener regression. |
| S9: surviving command descendants | Owned process groups are terminated on timeout, cancellation and provider retirement. Descendant side-effect regression; independent desktop launches retain their intended lifetime. |
| S10: launcher precedence/errors | Resolve desktop IDs before visibility filtering; isolate unreadable entries. Hidden override and malformed-file regressions. |
| C1: NetworkManager destinations | Device, wireless, AP, settings and active-connection proxies have explicit service destinations. Real wire-contract tests on private D-Bus. |
| C2: Wi-Fi placeholders | Actual scan, saved/new connection, personal credential and hidden-network paths for NetworkManager/iwd. Scoped iwd credential agent; accurate security/signal/connection-state mapping. Temporary NM profiles have cancellation cleanup. |
| C3: invalid CI condition | File-dependent Nix condition moved to post-checkout steps; Rust 1.95 baseline, locked all-target lint/tests, explicit D-Bus/AT-SPI dependencies. |
| C4: incomplete release | Package both binaries, defaults, README, MIT license and service unit. Extracted-bundle smoke test runs in release CI. |
| C5: delayed progress/input | Stream scan/connection progress before completion, receive unsolicited backend changes, preserve ordered credential edits, reject busy-view duplicates, and respond to Close. Delayed and buffered-input transport regressions. |

Wi-Fi shutdown has an explicit timing contract: the panel closes immediately,
while its provider can use up to 35 seconds to retire daemon resources. Generic
providers retain the 250 ms grace. Tests cover cancellation before a delayed NM
creation reply and a provider child that cleans up after the generic deadline.
Daemon nonresponse or forcibly killing the whole application remains best effort.

## Quality and project records

- Strict warnings are fixed through removal of unused paths, test-only helpers,
  visibility alignment and small simplifications; no blanket lint suppression.
- Launcher discovery and launch-time lookup run outside the GUI update path.
  Request generations prevent old results reopening a closed/replaced panel.
- Wi-Fi D-Bus adapters and the protocol session loop are independently testable
  library modules, with backend fixtures separated from provider rendering tests.
- MIT was selected by the owner; the root license and Cargo metadata now agree.
- README documents building, configuration, installation and current support.
  ADR 0011 and the glossary describe Canopa v4 and existing native accessibility.
- [Vendor maintenance](../vendor-maintenance.md) records known provenance limits
  and upgrade checks. No comprehensive dependency-license/vulnerability audit
  is claimed.
- All 46 local implementation tickets were cross-referenced with their existing
  implementation areas and routed to human acceptance review where appropriate.
  Original acceptance boxes remain unchanged; role labels are not completion
  counts. See [.scratch/implementation-review.md](../../.scratch/implementation-review.md).

## Verification

Verification commands:

- `cargo test --locked --all-targets -j 2`: 1,176 tests pass (21 more than the audit baseline).
- `cargo clippy --locked --all-targets -- -D warnings`: passes.
- `cargo +1.95.0 check --locked --all-targets`: passes, using a separate target directory with debug symbols disabled.
- Formatting and strict Clippy with the CI Rust 1.95 toolchain: pass.
- `git diff --check` and shell-script syntax checks: pass.
- Package/extract/execute smoke test with a clean PATH: passes.

The complete test suite ran with the installed Rust 1.97 nightly; all targets,
including tests, type-check on Rust 1.95. No real system network daemon is used
by the transport fixtures. The archive smoke test passes with
locally built development binaries; an optimized release archive is built and
smoke-tested by the release workflow.

### Standards review

Two initial findings: dependent state could survive bus loss, and a canceled NM
operation could abandon a temporary profile. Both were addressed. Follow-up
review identified the host/cleanup deadline mismatch, now resolved by the
explicit Wi-Fi shutdown contract. Bus-loss state, canceled activation, delayed
creation replies and host cleanup grace have regression coverage.

### Spec review

One initial finding: queued ordinary credential input was discarded as busy
input. Ordered edit/submit regression now passes. Follow-up review identified
a lost synchronization notification when yielding to input; it is retained as
a pending refresh.

Review totals: Standards 2 initial findings addressed, Spec 1 initial finding
addressed; both follow-up findings addressed. Hardware behavior remains subject
to the evidence limits below.

## Remaining evidence and support limits

The [desktop acceptance matrix](../manual-validation.md) records unexecuted
hardware/compositor, CPU/RSS/startup and accessibility checks. These remain
explicit follow-up work, not passing results. No live Wi-Fi network was changed,
no user compositor/audio service was restarted, and no release was published.

Nix packaging remains unprovided and is explicitly unsupported in the README;
the existing optional CI path is valid but skips until a flake exists. Original
upstream revisions/complete patch inventories for the layer-shell vendor forks
also remain to be established. These limitations do not prevent the documented
Cargo build, but they should not be mistaken for completed packaging/provenance
work.

Follow-up: [desktop, contrast and vendor verification](2026-09-23-desktop-validation.md)
records the subsequent provenance inventory, accessibility fixes and executed
compositor measurements. Nix was explicitly deferred by the owner.
