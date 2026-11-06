# USB implementation and ownership checkpoint

Status: experimental Rust/Iced backend and canonical frontend candidate,
2026-11-01. USB remains unadvertised; installed programs and routing are unchanged.
[ADR0064](../../../docs/adr/0064-usb-consent-and-frontend-ownership.md) records the
ownership decision. [Research](../USB.md) describes the original contract and
[UMockdev preparation](../USB-UMOCKDEV.md) records synthetic-device provisioning.

## Current implementation

- Exact backend v1 wire, two out arguments, shared request ownership/16-dialog
  admission, cancellation and caller-loss handling.
- Bounded device metadata and plain labels, no backend device opens or input FDs.
- Unique offered-subset grants with read-only downgrade and no write escalation.
- Shared Iced theme, exported Wayland parent, unchecked individual selection,
  explicit Allow, fixed actions, scrollable devices and keyboard access.
- A canonical one-file frontend patch validates grants atomically, rejects late
  consent, rechecks cached-device availability/permission, preserves exact FD modes
  and removes the unbalanced reacquisition reference. Eight-FD batching remains.

## Evidence

Receipts live in root target/portal-usb-22/. The complete Portal check in
final-validation-03.json passes formatting, all-target Clippy, 179 library tests
(7 ignored), 55 integration tests (2 ignored) and the normal binary build, with
unchanged guarded inputs. Initial compile failures 01 and 02 are retained.

frontend-recipe-transition-01.json proves the exact old/new 499-file source trees
from the pinned official archive. Only src/usb.c changes; the stable source/build
cache is reused. frontend-build-01.log and frontend-candidate-01.json record the
successful one-job build and frozen frontend. The package signature and private
UMockdev runtime are recorded in umockdev-package-01.json; no system package is
installed.

The final reusable fixture fails on the old frozen frontend in
frontend-regression-baseline-09/summary.json. It records stale FD modes, leaked
frontend copies, finishable late consent, seven invalid grant cases accepted,
and cached FD delivery after permission revocation and unplug. The exact same
fixture passes on the candidate in frontend-candidate-08/summary.json. Earlier
baseline/probe drafts are separate evidence, including the initial private-bus
startup error and the corrected unique-request cancellation barrier.

The final native-dark-04 and native-light-02 runs use the exact normal executable
produced by the original-graph workspace build. Both appearances pass fourteen
groups, all 22 helper pidfds are terminal, all owned processes have exited and all
guarded inputs match. Earlier native-dark-03/light-01 evidence remains historical
on the pre-workspace executable. final-source-proof-01.json binds the final main,
actual workspace harness, 1,388 guarded repository inputs and all 499 frontend
inputs. final-candidate-archives.json retains the final main/harness losslessly;
pre-workspace archives retain the distinct earlier binaries. Each checksum has
one archive, with no per-case compositor copy.

The required workspace gate completes in 1,674.56 seconds with exit 101. Files
integration is the sole failing Rust target: 596 pass, ten fail and 41 ignored.
The exact failure-name set matches the preceding phase; its cause is unestablished.
Portal 179 library/55 integration tests, all other Rust targets, formatting and
workspace Clippy pass. The original post-Rust Settings, CLI, configuration and
vendor stages also pass. All guarded source inputs remain unchanged. See
workspace-analysis-01.json; the nested crash-helper failure is not an eleventh
main-harness failure. Workspace success is not claimed.

Observed workspace growth is 160,731,136 bytes, including concurrent small staged
shortcut evidence. Final USB evidence occupies about 168 MB, primarily verified
compressed executables. The observed filesystem difference from the prior
checkpoint is about 0.38 GB; about 14.47 GB remains free. Native checks reuse the
same checksum-addressed Niri and one new frozen frontend. No synthetic-device
result establishes real-device, real Flatpak or compatible distribution acceptance.
[evidence.json](evidence.json) records exact checks and remaining work.

## Remaining

Resolve or establish ownership of the broader Files failures before a green
release gate; validate real hardware and Flatpak identity/USB filtering; package/select the compatible standard frontend
before normal USB advertisement. GlobalShortcuts and Print remain unimplemented;
the broader eighteen-interface goal and experimental capture interfaces remain open.
