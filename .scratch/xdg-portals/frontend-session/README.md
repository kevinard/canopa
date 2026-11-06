# Original-owner frontend sessions

Status: scoped canonical acceptance passes, 2026-10-31. Shared validation has
resumed after user disk cleanup: workspace Clippy and attribution checks pass;
the full Rust run records ten Files failures. A later compilation stopped at its
6 GB growth budget, leaving about 23 GB free. All 17 available remaining test
executables pass with their correct crate working directories. The initial
disk-full failures remain evidence. The
broader goal remains incomplete; installed routing and normal RemoteDesktop
registration/advertisement are unchanged.

The original mixed-source backend-kill test releases held input but receives no
public Session.Closed. The frontend now pins implementation Session proxies to
their original unique owner and watches the well-known name. Loss atomically
revokes lookup/admission. Cleanup targets the original owner with no activation
and a five-second asynchronous deadline; one worker completes public closure.
The frontend loop remains responsive while ConnectToEIS holds the session lock.
Pending creates cannot revive after replacement, reacquisition or early backend
Closed. Ordinary client/backend notification rules remain.

The canonical overlay retains its input-order patch and adds session-owner.patch
for two files. Both reproduce the official 1.22.1 archive with the original LGPL
license. All 499 source inputs match the final prepared source; old canonical
sources and frozen executables remain unchanged. The canonical recipe digest is
b00f9e7bd1ca48403116e94d7b50ee67218435fde3aa826894ea21a081557076.
The canonical executable is
cb35d770e84578dec58682cc2deeb0b8e2a93b1458e2371b3c0ce2036bf2a4d7.
Lossless canonical and sanitizer ELF archives are retained under
 target/portal-frontend-session-13/binary-archives/ with verified decompression hashes.

| Check | Result |
| --- | --- |
| Distinct isolated-bus scenarios | 10, covering five backend session families |
| Prepared/canonical/address+undefined builds | 45 groups each; all pass |
| Canonical mixed monitor/window SIGKILL, UI-stop and client loss | 4 runs, 36 groups, 44 actual video frames |
| Existing pointer consent/handoff regressions | 17 groups per appearance, 14 frames |
| Existing ordered keyboard Notify | 14 groups, 12 frames |
| Real frontend device availability | 6 groups |
| Standalone ScreenCast | 17 groups per appearance, 14 frames; backend-death public Closed required |
| Existing FileChooser | 12 groups per appearance; held-Shift fixture retained |
| Canonical native total | 148 groups, 84 frames |
| Sanitizer mixed-source SIGKILL | 9 groups, 11 frames; no memory-error report |
| Settings activation/read/reload | Passed in 0.75 s |
| Source/patch/archive verification and same-cache reuse | Passed,499 files |
| Required workspace gate | Exit 101 after 426.07 s: Clippy ENOSPC; tests not reached |
| Scoped native processes | All terminal |
| Resumed workspace checks | Clippy/attributions pass; Files integration 596 pass/10 fail/41 ignored |
| Storage-limited continuation | Compilation stopped at 6 GB growth; 17 ready executables pass, 2035 tests/9 ignored |

The original frozen frontend fails exact-final-fixture loss/replacement/stall and
three pending-create controls. An intermediate draft also fails while a real
ConnectToEIS call holds its session lock; the worker correction passes. The
strengthened standalone ScreenCast test fails at public backend-loss Closed on
the original frontend, then passes all 17 groups per appearance on the canonical
build. Assertions remain required. Leak checking was disabled in sanitizer runs;
they establish scoped memory-error coverage, not leak acceptance.

The initial workspace run stopped at a Notify incremental-cache write after the
filesystem filled. Its same-cache continuation passes Clippy, attributions and
Settings, then records the same ten Files integration failure names as the prior
gate. The exclude-Files continuation and CLI build encounter ENOSPC; configuration
and vendor stages remain pending. Matching failure names do not establish
baseline causality or a passing full gate.

After the user's storage request, the next continuation retains the original
full-workspace feature selection and stops compilation at 6 GB net growth rather
than filling the disk. Its completed test binaries run without more compilation:
17 executables pass, with 2035 passing tests and nine ignored. The direct runner's
initial workspace cwd causes three Notify packaging failures; all three pass
from the crate cwd, and the entire ready set then passes with Cargo's correct
working directories. All 1501 guarded source inputs match their expected values,
including the explicitly recorded storage-tooling delta. No Rust application
source changed in this correction. See [storage evidence](../storage/README.md).

[evidence.json](evidence.json) pins the results and retained red controls. Full
execution is in target/portal-frontend-session-13/; canonical build sources and
frozen executable are in /tmp/canopa-xdp-session-canonical-13/. The shared overlay,
license and before/after hashes are in vendor/source-overlays/ and provenance.
See [ADR0055](../../../docs/adr/0055-frontend-session-owner-lifetime.md).

Next: finish only the still-unbuilt/unexecuted shared targets with the existing
cache and bounded storage, retaining these completed results; then continue source-
loss/cancellation, physical/touch and compositor distribution acceptance and the
remaining interfaces in the [roadmap](../ROADMAP.md). This milestone does not
complete the broader goal.
