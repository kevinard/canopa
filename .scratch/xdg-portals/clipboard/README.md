# Clipboard implementation

Status: private Rust/Iced RemoteDesktop and active-only InputCapture integration
pass scoped acceptance,
2026-11-01. Clipboard v1 methods, authenticated session ownership, independent
unchecked consent, ongoing controls and ordered directed signals are implemented.
Complete Portal checks and 62 scoped native groups pass, including both
appearances, retained-FD revocation and existing input/media regressions.
The later InputCapture checkpoint adds separate unchecked consent and a fresh
transport for each actual activation, with held-pipe revocation and late-completion
isolation. Portal 176 library/53 integration checks and 143 native groups pass;
see the [InputCapture ledger](../input-capture/clipboard-evidence.json) and ADR0062.
The later [frontend phase ledger](../input-capture/frontend-phase-evidence.json)
records the canonical phase correction and final 146 native/45 private-bus checks,
including repeated Enable and stale Release on the normal workspace-built application.
Broader production acceptance remains open. No normal Clipboard advertisement,
installation or live routing change was made. The full portal goal remains open.

[ADR0058](../../../docs/adr/0058-clipboard-session-consent-and-ordering.md)
records the session boundary and receive/Start-response ordering. Five integration
cases exercise owner/kind/phase checks, actual FD directions and replay, directed
signal order, no-reply RequestClipboard ordering, blocked-operation cancellation
and queue saturation. The first public native run additionally proves real Iced
unchecked/denied/clipboard-only grants, initial pre-Start contents, an independent
2 MiB paste, and revocation after UI stop, caller loss and backend death. Its exact
receipt is target/portal-clipboard-16/native-dark-04-run.json. That candidate is
archived losslessly before final hardening is compiled.

## RemoteDesktop session checkpoint (phase 16)

| Check | Result |
| --- | --- |
| Complete Portal library | 172 passed, 6 ignored |
| Complete Portal integration | 48 passed, 2 ignored |
| Formatting, all-target Portal Clippy and binary build | Passed |
| Public Clipboard/Iced/independent peers | 8 groups per appearance |
| Raw transport and independent grant revoker | 8 groups |
| Existing pointer, shared media and EI handoff | 17 groups per appearance |
| Existing keyboard burst and legacy-to-EI handoff | 2 groups per appearance |
| Native total | 62 groups passed |
| Final guarded source inputs | 1144 unchanged |
| Native binary pins and private process cleanup | Unchanged; processes terminal |

The final public fixtures are native-dark-06 and native-light-02; the raw test is
transport-final-02. Exact commands, checksums and all seven native stages are in
target/portal-clipboard-16/final-native-02.json. The complete Rust validation
receipt is final-validation-03.json in the same directory. The final Portal and
harness have verified lossless archives, about 75 MB combined; identical archive
content is reused by checksum. No installed frontend, compositor or routing is
selected by these tests.

The added raw revoker case initially used the running-snapshot helper, which
rejects the expected stopped snapshot before testing its predicate. A direct
bounded shutdown wait fixes only that test; production source remains unchanged.
It now retains reader and writer duplicates and the Bridge owner while revoking,
then proves EOF/EPIPE before owner drop. Earlier wire/native fixture failures and
their corrected controls remain distinct in evidence.json. Scoped success does
not establish the broader slow/abandoned-peer, frontend/helper/compositor fault,
real Flatpak, distribution or InputCapture acceptance matrix.

Both final validation passes reuse root incremental outputs and default features.
The last full-check pass reduces observed storage by about 36 MB; its native
checks add about 37 MB. Native cases share the checksum-addressed compositor and
the root Portal binaries. Existing Cargo caches are retained.

One session-owned worker retains a separately named Wayland connection pinned to
the verified compositor PID, current regular-clipboard offers, bounded MIME
metadata, single-use transfer serials and revocable pipes. It streams payloads
through fixed 16 KiB buffers, with at most 16 transfers, 30-second idle/unanswered
and 300-second absolute deadlines. Source replacement and closure cancel admitted
pipes, including their connection to application-held descriptor duplicates.
Regular files/devices are refused as pump targets; broader Wayland receiver
compatibility needs assessment before production completion.

The complete requirements are in [the primary-source research](../INPUT-CAPTURE-CLIPBOARD.md)
and [ADR0057](../../../docs/adr/0057-session-clipboard-transport.md).
[PLAN.md](PLAN.md) retains the integration requirements and distinguishes the
implemented session work from remaining acceptance. InputCapture must
use its real capture/barrier/session implementation, which is still absent.

## Transport-stage validation (historical, before session integration)

| Check | Result |
| --- | --- |
| New pipe/ledger tests | 7 passed |
| Complete Portal library | 170 passed, 6 ignored |
| Complete Portal integration | 43 passed, 2 ignored |
| Formatting and all-target Portal Clippy | Passed |
| Final private compositor / independent wl-copy and wl-paste | 8 groups passed |
| Binary stream | Exact 2 MiB in each direction |
| Source/binary guard | All 1139 source inputs and native pins unchanged |
| Private native processes | Terminal |

The native fixture verifies compositor PID binding, external text/binary reading,
stale generation denial, own-offer correlation with private markers filtered from
session-visible metadata, primary-selection isolation, claimed/completed/replayed
serials, EOF ordering, failed/declined transfers, source replacement and closure
with application FDs still open. It uses existing Niri925c through the shared
checksum-addressed store, no GTK, and no per-run executable copies. The final
harness is SHA-256 `00c624e6250911ad5d8f24e69015eabf0c277c9c66251af9e78370b932d865a6`.

The first native attempt fails after the intended 30-second idle timeout: the
fixture writes 2 MiB while leaving wl-paste stdout unread, so the consumer's pipe
backpressures the whole chain. The corrected fixture drains concurrently and
passes in 3.26 seconds without changing the transport implementation; the final
run passes in 5.70 seconds on the final compiled source. Original failure, corrected
control and final result remain recorded separately. Initial compiler type/move/
trait-scope errors, formatter module ordering and Clippy's equivalent descriptor
predicate correction precede final validation and are not counted as acceptance.

The final checks reuse default features, one incremental compiler job and root
target/. They grow storage by about 1.10 GB within a 2 GB cap, with about 17.8 GB
free afterward. Earlier native-validated Portal14 binaries are preserved in
verified gzip archives before later builds replace root debug outputs; their
combined compressed size is about 81 MB. Current Cargo outputs/caches are retained.
The earlier complete workspace remains red on ten Files recovery/Undo tests;
these app-scoped checks do not establish a green workspace or production portal.

[evidence.json](evidence.json) links the exact commands, sources, binaries, native
results, failures, archive receipts and outstanding requirements. The final native
harness has a verified lossless gzip archive. Intermediate native01/native02 ELFs
were superseded at the same Cargo path before archival; their original execution
hashes, exact relevant source versions, manifests and logs remain. Their source
reconstructions match the original input hashes. This is not a claim of retained
intermediate executables. Full logs and receipts live in target/portal-clipboard-15/.

Build with the existing root default feature/profile selection and use the
library harness path printed by Cargo. Record its checksum and run:

```sh
python3 scripts/test-clipboard-transport.py \
  --harness HARNESS --harness-sha256 HARNESS_SHA256 \
  --niri FROZEN_NIRI --niri-sha256 NIRI_SHA256 \
  --output target/clipboard-native-new
```

Direct compiled harness execution inside the fixture uses the Portal crate's
working directory. Native acceptance needs the existing private compositor/media
fixture dependencies plus wl-copy/wl-paste. This test bypasses the
portal API/consent layer; it never claims sandbox authorization or changes the
live desktop clipboard.
