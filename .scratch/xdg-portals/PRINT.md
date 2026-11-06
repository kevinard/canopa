# Print portal investigation

## Current durable authenticated creation boundary

Late Create login now records the correct owner before the scheduler can accept
the job, preserving existing ownership after creation. Actual private PAM tests
cover lost replies, worker death and refused durable writes.192Print/410Portal
library/55integration/12Lockdown plusPPDruntime1, strict workspace Clippy and the
original compile graph pass.103 native cases/503 observed terminal processes pass,
including both Iced appearances and actualGTK/explicitQt callers. Source snapshots
and the failed baseline are preserved; netphasegrowth about37MB, no binarycopies
or new dependencies. See [ADR0096](../../docs/adr/0096-durable-owner-before-authenticated-creation.md)
and [creation-owner evidence](print/creation-owner-evidence.json).

Send-only authentication, explicit protected recovery, held-job correction, Digest,
longer credentials and broader Print/device/runtime gates remain. Print stays
opt-in; automatic Qt remains deferred and the full18 goal stays active. No complete
workspace execution; ten existing Files integration failures remain.


## Phase55: automatic cleanup authentication scoped verified

The full18-interface goal remains active:11 packaged, six experimental, Print
opt-in. The user chose Canopa Print features first; automatic Qt integration is
deferred. No required user input or external blocker.

Automatic synchronous retirement now permits only exact approved credentials for
the checked account. New/rejected scopes cannot open another password dialog, use
prompt budget or become a user cancellation. Same-scope cleanup can still cancel
the exact owned job. Uncertain records stay available for review, with no document
retry and accepted/unrelated jobs preserved. See [ADR0095](../../docs/adr/0095-cached-credentials-during-printer-retirement.md)
and [scoped evidence](print/retirement-authentication-evidence.json).

The final source snapshot is11files/99014bytes; binary SHA256
26939bdff8a14aa8db6f46fb405098d8b1a7c8481ceff20308db91bae8d69e12.
Format, strict workspace Clippy and the original workspace/default/all-target/
offline/incremental/jobs1 compile graph pass.190 Print/12 Lockdown/408 Portal
library/55 integration plus explicit PPD runtime1 pass. Twelve fixture receipts
cover91 native cases and480 observed terminal processes, including both Iced
appearances, real GTK and explicit Qt callers and reconnect guards. Startup has
no GTK/CUPS/qpdf link. No new native binary, binary copy, dependency, runtime
profile or route. Phase free space gained184041472bytes;6GB reserve retained.
No complete workspace execution; ten existing Files integration failures remain.

The corrected real-worker seven-case matrix and four public Iced failure cases
pass. Full authentication11groups and ordinary printer6groups per appearance,
authworker14, controls27 and reconnect5 pass. All native handles are terminal;
no validation is running. The original red failure and intentionally stopped
candidate01 validation remain preserved. Native applications did not require a
debugger in this phase; prior external SIGTERM cause remains unknown.

Next is [actual account/job ownership](issues/06-authenticated-job-ownership.md).
Private read-only PAM/passwd namespace overlays now measure real scheduler
principal binding using a synthetic alias/password, with no host auth changes.
Metadata/Create auth changes job owner; Send-only auth preserves the original.
Lost Create acknowledgement after late login reproduces a wrong-owner durable
record and leaves the job held. Send-only worker gets explicit401, invokes its
credential callback and never issues authenticated Send. Raw authenticated IPP
control succeeds. CUPS source confirms Finish/GetResponse does not replay the
stream; no general retry may be added for uncertain acceptance. See
[measured investigation](print/principal-binding-investigation.json).

No fix for those two gaps is implemented yet. Build regression assertions from
principal-probe03/04/05 before editing. Replay sources are preserved under
`target/portal-print-55/replay/`; no live private services remain. The two earlier
setup failures remain failed. Separate owner from account authority, persist
before authenticated Create, freeze owner after creation, preserve legacy records
and exact checked cancellation. Finish explicit protected recovery and held-job
Authenticate-Job without document resubmission; Digest/long credentials and
native timeout/stall/file-output credential clearing remain required too.

Full Print still needs broader physical/driverless/Flatpak/device precision,
failure/resource/runtime/default packaging. Six experimental companions and final
workspace/native/release gates remain. No commit/PR or goal completion/pause/block.


## Current Iced authentication boundary

Scoped native checks now pass Iced scheduler/device prompts, wrong-password
retry, private prepared credential handoff, direct and actual GTK/explicit Qt
prepared callers, and cancellation/caller/Lockdown/shutdown withdrawal in both
appearances. Credentials stay request-owned and out of public settings and
persistent Canopa storage. Protected recovery, late account binding, held-job
correction, Digest and longer credentials remain required; Print stays opt-in.
The explicit Qt PDF renderer's own native CUPS lookup is isolated in its fixture;
automatic Qt printing remains deferred. See
[ADR0094](../../docs/adr/0094-request-owned-printer-authentication.md),
[Iced evidence](print/iced-authentication-evidence.json) and
[remaining authentication work](issues/04-authenticated-printing.md).

## Historical authentication worker boundary

The separate worker credential channel passes scoped private CUPS Basic/TLS and
literal per-job field checks. The normal client still has no credential provider;
Iced prompts, private prepared-print handoff, protected recovery, late account
binding and held-job correction remain required. Longer credentials and Digest
also remain open. This is worker acceptance, not authenticated printing feature
completion. See [ADR0094](../../docs/adr/0094-request-owned-printer-authentication.md),
[authentication evidence](print/authentication-evidence.json) and
[the follow-up](issues/04-authenticated-printing.md).

## Current application preferences

Initial supported application preferences now resolve against current device
choices and apply once. Missing printers require selection; unsupported fields
show a notice. Actual GTK/explicit Qt paper precedence and explicit custom default
restoration pass in both appearances. See [ADR0093](../../docs/adr/0093-initial-print-preferences.md),
[preference evidence](print/preferences-evidence.json) and [checkpoint](CONTINUE.md).

## Current advanced printer controls

2026-11-02: the Iced draft offers paper source/type, quality and resolution from
bounded advertised values. Saved standard and driver aliases are resolved; a
full current-option check blocks conflicting defaults and conditional custom-size
violations. The same worker path rechecks the selected driver before intent or
Create-Job. Prepared clients receive quality and integer-DPI x/y rendering hints;
raw device DPCM units remain unchanged. Restoring a custom default may require
Check paper size with the current options.

An actual runtime crash exposed a CUPS thread-exit callback after library unload.
The debugger and unchanged-harness preload differential confirmed the cause;
a lazy process-lifetime handle fixes it. Final validation passes155Print,
12Lockdown,373Portal library,55integration and the explicit publicPPD regression,
plus formatting/strictworkspaceClippy/originalgraph compilation. Eighteen actual
worker cases and five reconnect cases pass. Dark100 and light125 each pass four
public Iced advanced-control groups plus six custom/GTK/explicitQt regressions.
Accepted IPP source/type/quality and four600x1200DPI raster pages are verified;
139 observed final processes terminal. No GTK startup dependency or new native
fixture executable, binary copy, target tree or dependency was added.

The earlier PDF passthrough run remains failed downstream evidence. Its CUPS job
retains all four approved fields; that does not establish their physical effects.
Raster output verifies resolution through the installed driver rather than an
absent job-environment field. The explicitly declared receiver needs only narrow
duplicate-MIME metadata normalization; requests and job replies are unchanged.
See [ADR0092](../../docs/adr/0092-printer-controls-and-driver-constraints.md),
[evidence](print/controls-evidence.json) and
[remaining device work](issues/03-advanced-device-delivery.md).

Next: Iced authenticated-device controls, broader
app/device/driverless/physical acceptance, publication-crash/kernel/global-resource
checks and runtime/default packaging. Qt automatic integration stays deferred;
six experimental interfaces, companions, finalworkspace/release and ten existing
Files failures remain. Full18 goal active; Print remains opt-in.

## Historical custom paper and saved preferences

2026-11-02: custom paper uses integer hundredth-mm input, selected CUPS exact
geometry and printable margins, explicit Iced confirmation, edit/side invalidation,
saved media spelling resolution and fresh validation beforeCreate. Printerdefault
restores the saved custom sheet; fixed selection shows an explicit paper. Invalid
and stale geometry cannot create a job. Errors stay beside the size controls.

Current validation passes152Print/12Lockdown/370Portal library/55integration,
formatting, strictworkspaceClippy and originalgraph compilation. Both appearances
cover15nativegroups with actualGTK and explicitQt prepared callers, direct PDF,
scrolled125% controls, saved defaults and capability change beforePrint. Qt's
retained3-page source is copied into6pages with exact source dimensions.93observed
finalprocesses terminal. Dark acceptance combines a successfulprefix and scoped
continuation; earlierfailed/interrupted receipts retain their status. About98MB
netstoragegrowth,6GBreserve,noexecutablecopies/newnativeprograms/cachecleanup.

Legacy CUPS device metadata rounds the sheet despite exact approved API/PDF
geometry; physical/driverless/customIPP precision remains open. Qt automatic
integration remains deferred by the user's choice. See
[ADR0091](../../docs/adr/0091-exact-custom-printer-paper.md),
[evidence](print/custom-paper-evidence.json),
[precision follow-up](issues/02-custom-paper-device-precision.md) and
[checkpoint](CONTINUE.md). Full18-interface goal active; Print stays opt-in.

Next: source/type/quality/resolution and authenticated-device controls, broader
saved preferences and app/device acceptance, publication-crash/kernel/global
resources, realFlatpak/physical/runtime/default packaging. Sixexperimental,
companions, deferredQt and finalworkspace/release/tenFilesfailures remain.


## Historical automatic Qt routing gap

2026-11-02: an actual public-API QPrintDialog caller with the installed Qt6.12.0
Wayland/xdgdesktopportal theme uses the Qt dialog and CUPS, with no Print portal
methods on cancel or print. Healthy Print/FileChooser version4 and a separate
explicit Qt D-Bus/Iced control prove the frontend/backend path. Direct native
printing delivers three exact pages; explicit portal printing delivers approved
two copies as one six-page job. Both saved scheduler controls are terminal9.

The native automatic cases remain mismatch evidence, not automatic Qt acceptance.
All24 observed fixture processes terminal; one55KB executable in the existing
checksum store, no binary copies/Cargo rebuild, about1.9MB net growth. Official
Qt sources, installed library/plugin pins, actual window captures and private
bus monitor are retained. See [ADR0090](../../docs/adr/0090-automatic-qt-print-routing-gap.md),
[evidence](print/qt-routing-evidence.json) and
[follow-up](issues/01-automatic-qt-print-integration.md). The user chose to finish Canopa Print features first; Qt client integration stays
tracked for later. Full goal active.

## Historical Iced printer outcome review

2026-11-02: --recover-print has Files/Printers tabs, metadata-only listing and
separately confirmed printer record removal. Full-receipt SHA256 revision and
worker liveness are rechecked under the exclusive catalog lock. Busy/unavailable
records cannot be removed; missing matches stay unconfirmed. Separate Check and
clean up preserves accepted/live jobs and never sends a document again. Startup
opens printer-only review after readiness; catalog failures stay independent.

147Print/12Lockdown and complete Portal365 library/55 integration, formatting,
strict workspace Clippy and original-graph compilation pass. Actual Iced/private
CUPS checks pass dark100/light125, including Keep/removal/job preservation, a new
claim during confirmation, scoped cleanup, absent-job review, printer-only startup,
file-copy preservation and narrow scrolling.13 normal-service public groups and
5 reconnect cases also pass;84 observed final processes terminal. Controlled
accepted-record UI data is distinct from actual final document acknowledgement.
The false-empty file-catalog heading is fixed; red source/screenshot/receipt stay
preserved. Measured phase free space increases about65MB, no executable copies/
new native driver. See [ADR0089](../../docs/adr/0089-iced-printer-outcome-review.md),
[evidence](print/printer-review-evidence.json) and [checkpoint](CONTINUE.md).

Next: automatic Qt/live toolkit printing, advanced/custom-media/device/saved-
instance/authenticated controls, publication-crash/kernel/global-resource tests,
real Flatpak/physical/runtime and default packaging. Six experimental interfaces,
companions, ten Files failures and final workspace/native/release gates remain.
Print stays opt-in; the full18-interface goal stays active.

## Historical checked CUPS cancellation connections

2026-11-02: a real private scheduler switch after final ownership revalidation
reproduced cancellation of an unrelated reused job ID. The worker now retains
libcups thread/HTTP/auth context, permits lookup reconnects, rejects connect during
cancellation and performs fresh ownership lookup after a rejected reconnect.
Three changing connections remain unresolved. One supervisor serves all receipts.

The retained five-case actual-worker regression passes stable, benign reconnect,
replacement, repeated reconnect and multiple-record cases;8 observed processes
terminal. Formatting, strict workspace Clippy, original-graph compilation,
140Print/12Lockdown and complete Portal358 library/55 integration pass.13 normal
service dark crash/restart groups pass with51 observed desktop/CUPS processes
terminal, and the live-thread lifetime regression stays busy until whole exit.
No UI/dependency/routing change or application executable copies; about52MB net
storage growth. The original failed regression remains failed evidence. Assumes
namespace continuity within an established connection; no broader atomic IPP or
adversarial proxy cancellation claim. See
[ADR0088](../../docs/adr/0088-checked-cups-cancellation-connections.md),
[evidence](print/printer-connection-evidence.json) and [checkpoint](CONTINUE.md).

Next: Iced review/absence handling for unresolved printer records, followed by
remaining toolkit/device/auth/resource/Flatpak/physical/runtime and six-interface/
release gates. Print remains opt-in. Ten Files integration failures and full
workspace execution remain open.

## Historical durable printer ownership and scoped restart cleanup

2026-11-02: production printing persists a bounded exact-tag intent before CUPS
creation and keeps acknowledged IDs/accepted state durable. Startup/failure cleanup
claims only dead workers, matches name/user/queue/configured endpoint and retires
unaccepted jobs; accepted records are discarded without cancelling acknowledged
jobs. Process pidfds preserve busy while a zombie leader still has a live thread.

140Print/12Lockdown, complete Portal358 library/55 integration, formatting, strict
workspace Clippy and original-graph compilation pass. Both appearances pass13
native groups including actual main death at unknownCreate, midSend and lostfinal
ACK, exact retirement/no retry, live/accepted/unrelated isolation, changed-server
retention and full-journal refusal beforeCreate.15 prepared-format/copy/duplex
groups pass too.201 observed final desktop/CUPS processes terminal; source/binary
pins match. Native red/green lifetime proof closes the zombie-leader assumption.
Net growth about58MB, no new native executable/copies. See
[ADR0087](../../docs/adr/0087-durable-printer-job-ownership.md),
[evidence](print/printer-recovery-evidence.json) and [checkpoint](CONTINUE.md).

Next: atomic cancellation under live namespace replacement/internal reconnect,
Iced review/absence handling for unresolved printer records, then the broader
Print/toolkit/device/resource/Flatpak/physical/runtime and six-interface/release
gates below. Ten Files failures remain; no fresh complete workspace execution.
Print remains opt-in, not default or packaged routing.

## Historical CUPS final acknowledgement and failed crash audit

2026-11-02: a real lost final HTTP reply previously left CUPS's last IPP status at
success, causing false public/worker acceptance. The worker now requires HTTP200
and IPP success together. Actual public lost-reply/control cases and the retained
repository worker regression pass in both appearances;15 normal PDF/PS/SVG/copy/
duplex cases also pass. All133 observed final native processes terminal; source/
binary guards match.135 Print/12 Lockdown and full Portal353 library/55 integration,
formatting, strict workspace Clippy and original-graph compilation pass. Phase
net disk use decreases about7MB; no executable copies/new native driver.

Separate actual main-death-during-upload audit FAILS both appearances. Parent-death
KILL retires the worker without RAII cancellation. A known created CUPS job remains
held4/job-incoming after restart; the retained spool record confirms it. Lost
Create replies also leave held jobs unidentified to the product. Preserve these
failed receipts; do not claim crash cleanup or disable hard retirement. Next:
durable precise printer-job ownership/restart cleanup, then the broader Print,
six experimental-interface and release gates below. Ten Files failures remain;
no fresh full workspace execution is claimed. See
[ADR0086](../../docs/adr/0086-cups-final-http-acknowledgement.md),
[evidence](print/cups-acknowledgement-evidence.json) and [checkpoint](CONTINUE.md).

## Historical prepared PS/SVG printer acceptance

2026-11-02: measured one-copy prepared PS/SVG rejection by a PDF-only CUPS queue
is fixed. Production uses existing copy-only PDF normalization for these formats
with one copy too, preserving physical layout; one-copy PDF passes through and
prepared file output keeps exact bytes. No dependency or renderer changes.

Formatting, strict workspace Clippy, original-graph compilation,135 Print/12
Lockdown and full Portal353 library/55 integration pass.30 public printer cases
pass across dark100/light125, including one-copy PS/SVG, both duplex edges and
collation, actual received content/blank backs and intrinsic geometry.12 public
file regressions pass on the same binary. All201 observed processes are terminal;
source/binary guards match. Net growth about94MB, no new native executable/copies.
See [ADR0085](../../docs/adr/0085-prepared-printer-format-normalization.md),
[evidence](print/prepared-printer-evidence.json) and [checkpoint](CONTINUE.md).

Print remains opt-in and absent from packaged routing. Next: automatic Qt dialogs
and live toolkit printer emission, advanced/custom-media/device/saved instances,
publication/CUPS main-death and ambiguous submission, remote/kernel waits/global
resources, real Flatpak/physical tests and runtime packaging/default advertisement.
Six other experimental interfaces, full workspace/release and ten Files failures
remain. No fresh full workspace execution is claimed.

## Historical opt-in normal service and public PS/SVG file acceptance

2026-11-02: explicit `--experimental-print` enables Print in the normal backend,
sharing Requests, Lockdown, recovery location and shutdown. It is absent by default
and remains outside packaged routing. Both actual Iced appearances pass public
PS/SVG new/replace/mismatch, old hardlink, consumed-token fresh consent and real GTK
caller checks. A pending consent helper closes on normal shutdown with no output.

29 normal-service groups pass (12 dark100,12 light125,5 source-crash/restart), all65
observed processes terminal. After real PreparePrint overwrite approval, killing
only the main backend during a held source snapshot retires its identified worker,
retains the original bytes/inode/hardlink, and a fresh backend prints successfully.
This establishes source-snapshot main death, not publication/CUPS/power-loss or
kernel D-state/cross-restart quota safety. No `/dev/fuse` exists on this host.

Formatting, strict workspace Clippy, original-graph compilation,134 Print/12
Lockdown and full Portal352 library/55 integration pass. Phase growth is about19MB,
with no executable copies/new native driver and the6GB reserve preserved. See
[ADR0084](../../docs/adr/0084-opt-in-print-service-and-public-prepared-formats.md),
[evidence](print/public-service-evidence.json), [guide](../../docs/print-portal.md)
and [checkpoint](CONTINUE.md). No fresh complete workspace execution; ten preceding
Files failures remain.

Next: prepared PS/SVG printer delivery/automatic Qt clients, advanced/custom-media/
device/saved instances, publication/CUPS main-death and ambiguous submission,
remote/kernel waits and global resource accounting, real Flatpak/physical tests,
runtime packaging/default advertisement. Six other experimental interfaces and
final workspace/native/release/routing remain. Print is opt-in under development.

## Historical ordered ranges, recovery handoff and prepared-format file acceptance

2026-11-02: direct ranges now retain their declared order and repeated pages,
matching measured prepared-client behavior instead of sorting a union. The
regressions cover combined imposition, page-set/reverse/duplex, clipping and output
limits. Recovery reserves layout space for embedded scrollbars and opens exact
Unix-byte paths through FileManager1 to real Canopa Files in both appearances.
Missing-service errors, scrolling, retained file hashes and the exact catalog pass.

Validation passes134 Print/12 Lockdown, full Portal352 library/55 integration,
formatting, strict workspace/all-target Clippy and original-graph compilation.
The final native executable passes36 layout/copy groups against22 GTK references,
physical PDF geometry and8 cell maps, both actual recovery appearances, and6
controlled Production file deliveries/refusals of real GTK-prepared PS/SVG.
All189 observed final native processes are terminal; source/binary pins match.
The phase uses about369MB less disk, with no executable copies/new native driver.
See [ADR0083](../../docs/adr/0083-print-range-sequences-and-recovery-folder-handoff.md),
[evidence](print/layout-sequence-evidence.json) and [checkpoint](CONTINUE.md).

Next are public/toolkit/token/Iced/printer PS/SVG acceptance, advanced/custom-media/
device/saved-instance controls, full backend/crash/stalled-filesystem and cross-
restart resources, ambiguous CUPS submission, real Flatpak/physical tests and
runtime packaging/Print registration. Six experimental interfaces, full workspace/
release/routing and ten recorded Files integration failures remain. Print stays
private/unregistered. Scoped prepared file proof does not close the public or
printer prepared-format gate.

## Historical recovery review and copy flow

2026-11-02: the Iced recovery window, verified immutable export, fresh Files picker
consent, index-only dismissal and asynchronous startup review pass scoped native
acceptance. Both document and opaque/empty displaced bytes can be copied without
replaying a journal or consuming retained data. Source folders are forbidden as
new destinations. Busy/unavailable states, stale messages and cancellation remain
conservative. Normal Print format/nonempty requirements are unchanged.

All131 Print/12 Lockdown and full Portal349 library/55 integration tests pass,
with formatting, strict workspace/all-target Clippy and original-graph compilation.
Two normal-worker recovery cases, dark100/light125 actual Iced flows,6 filesystem,
3 retirement,24 renderer and18 public Print groups pass. Light explicitly verifies
fresh overwrite consent and preservation of an old hardlink. Actual normal backend
startup opens recovery after readiness and still does not register Print. All320
observed accepted processes are terminal. Net phase storage growth is about361MB,
with no application binary copies/new Rust driver and6GB reserve maintained.
See [ADR0082](../../docs/adr/0082-print-recovery-review-and-export.md),
[UI evidence](print/recovery-ui-evidence.json) and [the checkpoint](CONTINUE.md).

Next are broader Print layout/device/toolkit and crash/lifetime gates, including
native FileManager1 folder handoff, full backend death/power loss, actual remote
kernel waits, cross-restart budgets and converter descendants, prepared PS/SVG,
physical/real Flatpak, ambiguous CUPS and runtime packaging. Print remains private;
all six experimental interfaces and final workspace/release/routing gates remain.

## Historical phase40 startup-discovery foundation

2026-11-02: bounded durable discovery and completion evidence pass scoped
acceptance. The private catalog records intent before staging and each worker
before it can mutate a destination. A synced SHA-256 completion record distinguishes
complete payloads from partial copies. Process start time/boot identity and a
shared registration/discovery lock prevent offering active work. Discovery is
read-only: uncertain transactions are never replayed or automatically restored.

All116 Print/12 Lockdown and full Portal334 library/55 integration tests pass,
with formatting, strict workspace/all-target Clippy and original-graph compilation.
The normal-worker discovery fixture verifies an owned producer's confirmed SIGKILL,
retention of both bodies, successful index cleanup and two simultaneous publications.
Six filesystem,3 retirement,24 renderer and18 public frontend/Production/Iced groups
pass. Public scope is files dark100 and private CUPS light125. All272 observed
native processes are terminal; no installed routing/services/queues changed.
See [ADR0081](../../docs/adr/0081-print-startup-recovery-discovery.md),
[recovery evidence](print/recovery-evidence.json) and [the checkpoint](CONTINUE.md).

Next is the Iced recovery screen, digest-verified immutable snapshot, fresh Files
picker destination, explicit index-only dismissal and startup integration. The
catalog and journal are evidence, never permission to overwrite after restart.
Actual full backend death/power loss, remote/kernel waits, cross-restart budgets,
converter descendants and broader Print layout/device/Flatpak/CUPS/runtime gates
remain. Print stays unregistered and all six experimental interfaces remain open.

## Historical phase39 retirement checkpoint

Public Print remains unregistered pending its remaining acceptance gates,
2026-11-02. Caller completion is now separate from direct-worker retirement.
The owned task retains all operation/storage/consent guards and waits for confirmed
exit before settlement; shutdown returns/logs pending work after15s and leaves
admission closed. The current source passes99 Print tests, full Portal317 library/
55 integration regressions,3 real-worker confirmation-barrier groups,6 FIFO/
publication groups,24 renderer groups and36 public frontend/Production/Iced/CUPS
groups across both appearances. All312 observed processes are terminal, pins and
inputs match, startup linkage has no GTK/CUPS/qpdf, and phase net disk growth is
about173MB. See [ADR0080](../../docs/adr/0080-print-pending-worker-retirement.md),
[retirement evidence](print/retirement-evidence.json) and [the checkpoint](CONTINUE.md).

The barrier is test-only and can delay confirmation of an already-dead worker;
it does not claim to produce a physical kernel D wait. Staged/new and previous
bytes survive until safe settlement. Real FIFO cancellation and existing physical
copy/duplex behavior remain green, including22 actual private printer deliveries
and GTK/explicit Qt DBus-PDF clients. Automatic QPrintDialog portal use remains
unestablished. File metadata/setup run in workers and staging handles in the owner
are O_PATH-only; ambiguous outcomes do not trigger automatic replay.

Startup recovery/index/UI and abrupt backend/power loss are next. Actual remote/
kernel-wait acceptance, converter-descendant/cross-restart accounting, broader
page-set/reverse/duplex layout, advanced/custom/saved-instance controls, prepared
PS/SVG, physical-printer/Flatpak, ambiguous CUPS submission and runtime packaging
remain. Installed services, queues, permissions and routing stay unchanged. Earlier
checkpoints retain their original inputs and limitations; newer evidence supersedes
pending claims only for the cases it actually verifies.

## Current Rust foundation

2026-11-02: `crates/canopa-portal/src/print/` now contains bounded request/settings
parsing, authenticated original-client proofs and a volatile one-use token store.
Tokens are scoped to the current frontend and original client, including native
clients with empty application IDs; direct callers cannot borrow the identity in
a forged request path. Frontend loss/reacquisition, original-client loss, expiry,
concurrent consumption and retained-descriptor retirement are tested on private
buses. [ADR0070](../../docs/adr/0070-prepared-print-authority.md) records the policy.

All23 new cases pass; complete Portal library238/8ignored and integration55/2ignored
pass. Formatting, strict workspace/all-target Clippy and all-target compilation
pass. The successful validation reuses the original whole-workspace/default-feature
incremental graph and root target/; its net storage growth is negative, with about
9.5GiB free. Receipts are `target/portal-print-29/model-04.json` and
`portal-regressions-01.json`. Earlier compile/lint corrections and a harness-selection
preflight failure are retained separately. No new full-workspace test gate ran;
the prior ten Files integration failures remain unresolved.

This foundation does not export a Print interface. The Rust worker checkpoint
below now adds snapshot/submission. Iced printer/page setup and print-to-file consent, D-Bus
lifetime integration and actual client-rendering/native Print acceptance remain.
The private CUPS transport proof below remains a separate C API probe.

## Rust worker checkpoint

2026-11-02: [ADR0071](../../docs/adr/0071-isolated-print-document-worker.md)
records the actual isolated Rust descriptor/CUPS worker, two document/operation
slots, retained task cleanup and final-acknowledgement commit. Strict Clippy,
formatting and original-graph compilation pass. All34 Print cases pass, alongside
the complete Portal249 library/55 integration passes (8/2 ignored).

`target/portal-print-worker-native-05/summary.json` passes9 groups on normal
Portal509b1f20…: exact received offset/flags, immutable/sealed bytes, delayed
nonblocking input, future-drop and explicit cancellation retirement, slot reuse,
actual Rust-client IPP handoff, invalid-destination refusal and hard-retirement/
idempotent cleanup. The original603-byte PDF reaches the loopback receiver after
source mutation, with copies2, long-edge duplex and structured21000×29700 media.
An injected pre-finish stall moves only the owned partial job4→7; accepted job1
stays9. All observed processes are terminal and pins are unchanged.

Keep earlier native failures: raw queues advertise copies but lack paper/duplex
attributes, so the worker correctly refuses those explicit choices. The positive
pass-through PPD advertises them. Its first integer-point A4 geometry converted
to20990×29704; exact point dimensions fix the fixture. The final assertion checks
the actual media collection rather than requiring a simple media keyword.
Production Rust sources/binary remain identical across these fixture corrections.
The PPD deliberately exercises pass-through transport, not document conversion.

The successful all-target build takes949.31s and grows about1.5GB within the
8GB floor/3GB growth cap. Shared libc/libloading/uucore features and artifact
identities match phase29 exactly; no alternate feature/profile graph was selected.
No new full-workspace test gate ran, and the ten existing Files failures remain.
No installed queues/services, permissions or portal routing changed.

Crash policy is still unverified. The CUPS processing-time limit is documented
separately from incoming-job handling. Its scheduler assigns incoming deadlines
and can promote timed-out jobs to pending. **Inference:** `job-cancel-after` alone
does not prove safe pre-commit cleanup; test actual backend death before/after
Create-Job and during Send-Document before choosing policy.
[CUPS processing limit](https://openprinting.github.io/cups/doc/spec-ipp.html#job-cancel-after),
[pinned incoming-job setup](https://raw.githubusercontent.com/OpenPrinting/cups/v2.4.20/scheduler/ipp.c#L1594),
[pinned timeout handling](https://raw.githubusercontent.com/OpenPrinting/cups/v2.4.20/scheduler/job.c#L263).

## Printer catalog and private Iced draft checkpoint

Phase31 adds isolated CUPS catalog/capability queries and a private Iced print
draft, with the existing Files Save picker embedded in the same window. The
current source passes46 Print tests, complete Portal library261/8ignored and
integration55/2ignored regressions, formatting, strict workspace/all-target Clippy
and the original graph all-target build. Twenty-four scoped native groups pass
in printer/file modes and dark100/light125. Sixteen dialogs and61 observed owned
processes are terminal; no jobs are submitted or output files created. Empty
configured catalogs, saved standard instance choices and blocked metadata cleanup
are exercised. The final executable has no GTK or CUPS startup dependency.

This remains a draft helper: public Print, pinned file/replacement authority,
toolkit transform/copy compatibility and backend-death policy are unfinished.
UI right-edge clipping, full valid n-up hint display, advanced/custom printer
capabilities and legacy instance aliases also remain. See
[dialog evidence](print/dialog-evidence.json), [the checkpoint](CONTINUE.md) and
[ADR0072](../../docs/adr/0072-printer-catalog-and-iced-print-draft.md).

## Contract and frontend boundary

Phase32 adds retained file consent (ADR0073): O_PATH parent/existing-file pins are
captured before overwrite confirmation and passed by SCM_RIGHTS to the actual
Rust parent adapter. A path/metadata reply cannot replace those descriptors.
The final source passes54 Print tests, full Portal270/55 regressions, formatting,
strict workspace Clippy and original graph all-target compilation. Twenty-eight
scoped native groups cover20 parent reviews/20 Iced helpers, including file-pin
receipt, late-appearance refusal and unchanged existing bytes. All85 observed
processes terminate. Publication, public Print/token/Lockdown and toolkit/crash
compatibility remain unfinished; see [file authority evidence](print/file-authority-evidence.json).

The inspected canonical frontend is
`/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1`, including Canopa's
session/input patches. Its public Print interface is **version 4**. Current
official public XML and rendered documentation also describe v4. Backend Print
has **no `version` property**, in either the pinned XML or current rendered
reference; do not invent a backend v4 property. Current backend raw XML could not
be retrieved through the web tool, so current backend signatures below were
cross-checked against the official rendered reference, not a claimed bytewise
comparison. [Pinned public XML][public-local], [pinned backend XML][impl-local],
[current public XML][public-main], [current backend reference][impl-current].

All methods belong at `/org/freedesktop/portal/desktop`; `a{sv}` is a variant
dictionary and `h` is a D-Bus Unix-FD index, not a path or a process-local integer
to put in JSON. Exact argument order:

| Interface | Method | Inputs | Outputs |
| --- | --- | --- | --- |
| `org.freedesktop.portal.Print` | `PreparePrint` | `parent_window:s, title:s, settings:a{sv}, page_setup:a{sv}, options:a{sv}` | `handle:o` |
| public | `Print` | `parent_window:s, title:s, fd:h, options:a{sv}` | `handle:o` |
| `org.freedesktop.impl.portal.Print` | `PreparePrint` | `handle:o, app_id:s, parent_window:s, title:s, settings:a{sv}, page_setup:a{sv}, options:a{sv}` | `response:u, results:a{sv}` |
| backend | `Print` | `handle:o, app_id:s, parent_window:s, title:s, fd:h, options:a{sv}` | `response:u, results:a{sv}` |

Sources: canonical [public method declarations][public-methods],
[backend declarations][impl-methods], [D-Bus FD definition][dbus]. There is no
Print Session, job-status signal, printer-enumeration method or portal job-cancel
method. Public replies use Request.Response; backend replies complete the pending
method. Backend Request.Close has no arguments or results. Codes are 0 success,
1 user cancellation, 2 other termination. Caller Close suppresses the public
Response signal. [Request contracts][request-public], [backend Request][request-impl].

The canonical frontend owns caller identity, public request paths/authorization,
per-interface routing and the initial Lockdown `disable-printing` check for both
methods. It derives `app_id` from application info; forwards settings/page setup
without interpreting their contents; passes the received FD list and FD index
onward; filters backend PreparePrint results to `settings`, `page-setup`, `token`
on success; emits an empty results dictionary for Print. It does **not** discover
printers, render, spool, validate readable content, mint/store tokens, export a
print-to-file URI through Documents, or report a job ID. [Canonical Print
implementation][frontend-print], [prepare/result forwarding][frontend-prepare].

Options forwarded to PreparePrint are `modal:b` (default true), `accept_label:s`
(v2, mnemonic underscores), `supported_output_file_formats:as` (v3; lowercase
`pdf`, `ps`, `svg`), `has_current_page:b` and `has_selected_pages:b` (v4). Print
receives only `modal:b`, `token:u`, `supported_output_file_formats:as`.
`handle_token:s` belongs to public Request path creation and is not forwarded.
The pinned backend XML accidentally lists three PreparePrint options under
results; frontend forwarding and the current backend reference resolve that
documentation error. Invalid option types/formats are omitted by this frontend:
its callers ignore `xdp_filter_options`' false result and pass no error sink.
Backend validation remains necessary. [Option lists/callers][frontend-prepare],
[filter behavior][filter-local], [request path creation][request-path].

## Settings, content and ownership

PreparePrint receives **no document FD**: it returns user-selected `settings:a{sv}`,
`page-setup:a{sv}` and a `token:u`; the app then formats a document and calls
Print. A valid token bypasses the second settings dialog; without one Print
must present a dialog. Lifetime is backend-defined. The docs do not require
persistence, a 300-second lifetime, reuse, or successful printing after expiry.
[Pinned contract][public-local].

Settings values are **strings**, including booleans/numbers: `orientation`,
`paper-format`, `paper-width`, `paper-height`, `n-copies`, `default-source`,
`quality`, `resolution`, `resolution-x/y`, `use-color`, `duplex`, `collate`,
`reverse`, `media-type`, `dither`, `scale`, `print-pages`, `page-ranges`, `page-set`,
`finishings`, `number-up`, `number-up-layout`, `output-bin`, `printer-lpi`,
`output-basename`, `output-file-format`, `output-uri`. Page setup uses string
`PPDName`, `Name`, `DisplayName`, `Orientation`, and **double millimetres** for
`Width`, `Height`, `MarginTop/Bottom/Left/Right`. Page ranges are zero-based
inclusive strings (`0-2,4,9-11`); the UI displays one-based pages. Settings
orientation uses `reverse_landscape`/`reverse_portrait`, page setup uses
`reverse-landscape`/`reverse-portrait`. The public prose capitalizes output format
values, while GTK's serializer uses lowercase `pdf/ps/svg`; preserve tested toolkit
wire compatibility rather than copying prose capitalization. Initial dictionaries
are hints, not authorization to write an `output-uri` or submit to an arbitrary
destination. [Exact keys/types][settings-local], [GTK file format handling][gtk-file].

Only offer `print-pages=current`/`selection` when the corresponding capability
option is true; the backend has no original document selection to infer. Validate
finite positive paper sizes, finite nonnegative margins within the sheet, bounded
numeric strings and range syntax before publishing a usable setup. The public
key list does not specify a printer-identity key: keep the selected queue in the
token record. Ignore or preserve bounded advisory extension hints deliberately;
never forward arbitrary `cups-*` strings as unchecked submission authority.

**Compatibility gate:** GTK's portal client applies returned settings while
rendering a file, then sends that FD with the token. Its file backend sets manual
page selection, copies, collation, reverse, scale, page-set and number-up; CUPS
backend code separately maps device/backend settings. Therefore a blanket mapping
of every returned setting to IPP job attributes can apply layout or copies twice.
The public protocol does not say which operations were already baked into a
document. Define a prepared-document path (app layout already applied) separately
from direct Print (backend-selected layout), and prove actual GTK/Qt consumers
before choosing final per-key behavior. This is an implementation risk, not a
claim that the current GNOME behavior is a normative solution. [GTK portal
client][gtk-client], [file preparation][gtk-file], [CUPS preparation][gtk-cups].

GNOME reference behavior: random nonzero collision-checked tokens hold app ID,
printer, settings, page setup and preview intent; expire after 300 seconds; are
consumed once for the matching app ID; unknown/wrong-app tokens fall through to a
dialog. It duplicates the received FD and uses GtkPrintJob, with an optional
external Papers/Evince previewer. Its token path can reply success without
checking the submission result: do not copy that failure behavior. These are
reference choices, not additional XDG requirements. [GNOME source][gnome-print].

Proposed Canopa ownership:

- Reuse the request host's 16-slot admission, supplied-path export before UI,
  caller-authenticated Close, cancellation and unique-owner loss. The backend
  method's sender is ordinarily the **frontend**, not the sandboxed app. Current
  `Requests` observes that sender; after a completed PreparePrint it provides no
  token lifetime management. [Existing host][requests-local].
- Keep bounded, volatile, one-use token records in the Rust backend, tied at least
  to originating backend caller unique name and `app_id`; clear on frontend loss,
  backend shutdown and expiry. A stronger original-client binding is possible
  only with authenticated frontend ownership plus validation of its encoded
  request sender; the XML supplies no separate client unique-name argument.
  Do not treat an arbitrary supplied handle/app ID as authenticated identity.
  Pin this policy before implementation, especially for empty native app IDs.
- Own the received descriptor with RAII until a bounded snapshot/submission
  completes; do not resolve a caller-supplied filename. FD passing does not make
  mutable input immutable. Snapshot in a cancellable worker before preview and
  submission; avoid changing shared offsets/status flags accidentally. The
  contract requires readable content, not explicitly regular or seekable files:
  regular files/memfds, pipes, invalid/write-only FDs, current offsets, truncation
  and indefinitely open writers require deliberate tests and documented limits.
- Token records retain the selected destination and approved print-to-file
  target, never merely re-trust a returned settings string. Unknown/expired tokens
  must not silently choose a printer or write a file. Recheck destination
  capabilities after loss/change, and recheck local Lockdown before handoff if
  Canopa chooses immediate revocation of pending requests. The frontend only
  checks at method entry. [Existing Lockdown ownership][lockdown-local].

Request.Close ends interaction; it cannot guarantee reversal after a scheduler
has accepted a job. Proposed commit point is successful final document acceptance,
not dialog acceptance, child spawn or job-ID allocation. Before commit, stop
workers and cancel only a known owned partial job; after commit, the spooler owns
the job. Report submission success, not physical completion. If acknowledgement
is lost after acceptance, do not automatically retry and duplicate a job. Backend
death can leave an incomplete/held job: cleanup/expiry and the chosen policy need
proof. [IPP job operations/states][ipp-model].

## Rust/Iced implementation seam

Recommendation (design inference): keep protocol/token/job policy in Rust,
ordinary dialogs in Iced, and hide printer transport behind a typed destination,
capabilities and submission adapter. A small **Rust worker linked to libcups** is
a practical first adapter; libcups is printing infrastructure, not GTK UI. It
preserves configured queues/classes/instances and discovery without reproducing
DNS-SD, authentication, IPP and scheduler integration in the UI. A pure Rust IPP
adapter is possible, but this investigation did not establish an available,
audited dependency or equivalent discovery/authentication/legacy coverage. Do not
substitute parsed `lpstat` output for a capability model or shell commands for
job ownership.

The CUPS destination API provides enumeration/removal callbacks (`cupsEnumDests`),
bounded connection (`cupsConnectDest`), capabilities/defaults/media
(`cupsCopyDestInfo`, `cupsFindDestSupported/Default`, media helpers), option
validation/conflicts, and explicit job creation/document streaming/finalization
and cancellation (`cupsCreateDestJob`, `cupsStartDestDocument`,
`cupsWriteRequestData`, `cupsFinishDestDocument`, `cupsCancelDestJob`). Put calls
off the UI/async executor with deadlines and bounded IPC; copy callback-owned data.
Choose scheduler submission versus direct-device submission explicitly: only a
scheduler supplies its configured filtering pipeline. Direct printers must accept
the actual document MIME type or require an implemented conversion path.
[Official API][cups-api], [installed declarations][cups-header].

Use typed capability-driven controls for media/source/type, color, sides,
resolution/quality, copies/collation and supported finishing; distinguish
unavailable, paused/rejecting and removed destinations. Preserve the user's exact
selection across list refresh by stable identity, not row index. Never silently
substitute a default when a prepared printer disappears. Normalize units/ranges
explicitly; CUPS page-range options use one-based pages, unlike portal strings.
Unsupported options/conversions must be visible and must not return fake success.
[CUPS options][cups-options]. Authentication, TLS failures, PPD-specific legacy
controls, credential storage and driverless-versus-legacy distribution support
remain explicit product/compatibility decisions.

Reusable Canopa pieces: existing request/helper admission, parent/modality and
shared appearance from ADR0026; anonymous-pipe bounded JSON plus inherited owned
FD support and helper death on backend loss; Files' public `PreviewRequest` PDF
model uses private snapshots, `pdfinfo`, one-page `pdftoppm`, 128 MiB input and
1,800-pixel rendering. Its process runner kills owned process groups and bounds
output, but is private to Files, and has no independent wall-clock deadline.
The preview API currently takes paths and accepts only regular files, so feed an
owned private snapshot or extract a deliberate descriptor seam rather than pass
untrusted paths. [ADR0026][chooser-adr], [helper transport][process-local],
[PDF preparation][preview-local], [process runner][files-process].

Read-only host inventory: `pacman -Q` reports cups/libcups **2.4.20-1** (epoch 2),
cups-filters **2.0.1-3**, libcupsfilters **2.2.1.r32.gd49e9284-1**, libppd
**2.1.1-2**, poppler **26.10.0-1**, ghostscript **10.08.0-1**. `pkg-config
--modversion cups` reports **2.4.20**. Headers and `lp`, `lpstat`, `cupsfilter`,
`ippeveprinter`, `ipptool`, `gs` exist. PATH's `pdfinfo`/`pdftoppm` currently resolve
to Codex runtime wrappers; installed Poppler does not prove a deployed Canopa
runtime PATH. No cups/IPP Rust dependency is present in workspace manifests/lock.
This is dependency availability only, not a running scheduler, discovered printer,
conversion or authentication check. This initial inventory did not execute the listed printing binaries.

## Private runtime readiness follow-up

`target/portal-global-shortcuts-28/print-host-runtime-readiness-01.json` verifies
host UID1000, libcups2.4.20, readable headers and client tools. The installed
cupsd is root-owned0700; the IPP backend is root-owned0744. Do not infer that the
user can execute either installed program or change their permissions for tests.

The exact cups2:2.4.20-1 package is already cached. Readiness receipt02 verifies
its detached signature against the installed Arch public-key set (signing key
ADC8A1FCC15E01D45310419E94657AB20F2A092B), archive SHA256
4a9ff0c218eadce87a1c4392fd4f90b2aec345193996634dbe63c338aabf16fa,
and the daemon/backend member hashes. Initial gpgv attempt01 could not parse the
installed ASCII-armored key set as a binary keyring; explicit dearmoring fixes
only that input format. The derived public-key copy was removed after successful
verification; the original installed keyring hash and converted hash remain in
the receipt. No package installation or download occurred.

`target/portal-print-runtime-01/summary.json` then passes an actual private
scheduler readiness probe: one481,952-byte daemon extracted from that verified
package into a checksum-addressed test store, owned/executed as UID1000. Both
explicit fixture configurations pass cupsd -t. The daemon runs in the foreground
with all mutable directories under the fixture and only its private Unix socket.
An explicitly directed lpstat -r reports that private scheduler running; the
owned daemon then exits0 and its binary checksum remains unchanged. No installed
service, printer or queue was contacted or changed; no document/job was submitted.
The installed root-only daemon's permissions remain intact.

This proves a runnable unprivileged scheduler seam, not printer transport. The
fixture's private ServerBin is intentionally empty and CUPS reports missing
filters. Before a document-handoff claim, provide a verified fixture-owned IPP
backend and deliberate filter path, add only a synthetic printer, and prove real
accepted bytes/attributes and failures. Reuse the one checksum-addressed daemon;
never copy it into each artifact directory. Portal/token/Iced work and toolkit
transform compatibility remain unimplemented and open.

### Private document handoff and cancellation

`target/portal-print-runtime-03/summary.json` passes a stronger prototype with
actual libcups destination APIs, the owned scheduler and stock ippeveprinter.
The printer advertises no DNS-SD service, uses a private D-Bus for its discovery
client, and listens only on127.0.0.1/::1 (verified by actual listener/PID records).
Its explicit hostname reaches the listener creation in the
[pinned printer source](https://github.com/OpenPrinting/cups/blob/v2.4.20/tools/ippeveprinter.c#L1632-L1682).
Only a new fixture queue is configured on the private Unix scheduler socket.

The compiled test consumer creates a job, streams a603-byte fixed PDF and checks
final document acceptance. The real synthetic-printer command receives the same
SHA25625f517a8d3212db5d86dd5b0dbd2354789bd36e98f148af79e42e2bc299c1f8f,
application/pdf, copies2, two-sided-long-edge and A4. The scheduler reaches
completed state9. A second newly created owned job is cancelled into state7;
job1 remains completed. The three owned services exit0, and all binary/source
pins match. This is a C API/test-infrastructure proof, not a Rust portal method,
Iced consent, token, toolkit-transform, filter or physical-printer proof. The
raw queue intentionally isolates byte transport; lpadmin's deprecation diagnostic
is retained, so this is not production queue-configuration acceptance.

Retain the failed predecessor02: CUPS accepted the document but could not launch
the backend because the isolated ServerBin lacked daemon/cups-exec. The
[pinned scheduler source](https://github.com/OpenPrinting/cups/blob/v2.4.20/scheduler/process.c#L528-L555)
confirms that POSIX spawn uses that helper. Prototype03 adds the helper from the
same signature-verified package into the checksum store, without changing any
installed executable. Prototype02 also attempted the default /etc/printcap
output; its root-owned0644 file is not writable as UID1000 and its2021 mtime is
unchanged. Receipt installed-printcap-readonly.json preserves that observation.
Prototype03 explicitly directs Printcap into the fixture. Missing-filter warnings
remain deliberate for this raw transport probe; real filtering/conversion and
prepared-versus-direct toolkit compatibility remain open.

## Scope and staged acceptance

Mandatory backend behavior: both exact methods, real request retirement,
capability-aware user selection, settings/page-setup return, token-based reuse,
readable-FD ownership, actual document handoff and truthful failures. A settings
dialog plus success-shaped mock is not a Print replacement. Print-to-file is part
of the published settings/options and should be included in full replacement:
choose and confirm the destination in Iced, retain that authority, respect the
app's supported output formats, and avoid creating/truncating at PreparePrint.
Preview is **optional**: there is no public preview method/result. PreparePrint
cannot show document pixels yet; an optional preview intent must wait for Print
and remain an Iced surface, not launch a GNOME previewer. Full printer management,
arbitrary format conversion, queue UI and physical job tracking are separate
features; advanced device controls depend on actual capabilities.

1. **Contract foundation, unadvertised:** model settings/page setup with bounded
   extension handling, prove exact introspection and result shapes on a private
   bus through the canonical frontend. Cover absent/malformed options and the
   frontend's omission behavior, native/identified app IDs, foreign Close,
   duplicate handles, admission, Close/client/frontend loss, backend death,
   cancelled/failed PreparePrint, one-use/expired/wrong-owner tokens and races.
   Existing upstream mock Print tests establish forwarding, not printer handoff.
   [Canonical test scope][print-tests].
2. **Transport vertical slice:** implement bounded Rust worker/adapter and Iced
   discovery/settings against a disposable scheduler and synthetic printer.
   Prove PreparePrint → app-formatted PDF FD → valid token → one accepted job;
   direct Print must show settings and submit separately. Verify printer ID,
   actual spool bytes/checksum, document format and selected device attributes.
   Add multi-page labeled documents for ranges, copies, collation, reverse,
   duplex, n-up and orientation so a doubled transformation is detectable.
3. **Native consumer and file output:** private D-Bus, the canonical frontend,
   nested Weston/Niri and real Iced dialogs in dark/100% and light/125%; exported
   and expired parents, modality, keyboard and cancellation. Include a real GTK
   portal client and Qt portal client (GTK may be a test consumer, never Canopa's
   backend UI), actual file FD closure/unlink, rendered page pixels, and
   print-to-file overwrite/cancel/format/byte-path cases. Optional preview must
   show the intended page and never submit until the separate print action.
4. **Failure and advertisement gate:** remove/replace a destination after
   preparation; stop/restart scheduler; refuse job/create/send; fail authentication
   or TLS; truncate/change/oversize/encrypt input; hang a reader/converter; exhaust
   private spool space; kill UI/worker/backend/frontend/client before job creation,
   during upload and after final acceptance but before reply. Assert bounded exit,
   no success on rejected handoff, no duplicate jobs/revival, exact cancellation
   semantics, partial-job disposition and retained accepted jobs. Shared/vendor
   changes require the root workspace gate once, preserving its original graph.
   Keep focused/native receipts distinct from that gate and physical acceptance.
   Only then consider descriptor/routing changes; none are authorized by this note.

Concrete safe handoff fixture (proposal, not executed): private `cupsd -f` with
explicit temporary `-c` and `-s` files, all mutable directories beneath one bounded
fixture root, a private socket/loopback port and only disposable queues. Route a
fixture queue to loopback `ippeveprinter` with DNS-SD disabled (`-r off`), private
spool storage and an absolute harmless job command that records byte hashes and
`IPP_*` job attributes; retain one small document as evidence. Prove the command
actually receives the submitted document and that the job state reflects its
outcome. `ippeveprinter -k` permits retained document inspection; no physical
device or installed queue is needed. A fake executable named `lp` is insufficient.
If unprivileged private cupsd cannot run, use an isolated IPP test server plus a
separate private discovery fixture and record the scheduler-coverage gap.
[Scheduler flags][cupsd], [directory configuration][cups-files],
[synthetic printer flags and job environment][ippeve].

Open gates: final prepared-versus-direct transform mapping for real toolkit
clients; authenticated original-client token scope; pipe/offset/content limits;
legacy/driverless conversions and printer credentials; partial-job crash cleanup;
deployed runtime dependencies. None requires new compositor protocol. The existing
Rust/Iced window/request seam is sufficient; printer transport and semantic
compatibility are the missing work.

[public-local]: /tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/data/org.freedesktop.portal.Print.xml:24
[impl-local]: /tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/data/org.freedesktop.impl.portal.Print.xml:29
[public-methods]: /tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/data/org.freedesktop.portal.Print.xml:268
[impl-methods]: /tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/data/org.freedesktop.impl.portal.Print.xml:86
[public-main]: https://raw.githubusercontent.com/flatpak/xdg-desktop-portal/main/data/org.freedesktop.portal.Print.xml
[impl-current]: https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.impl.portal.Print.html
[dbus]: https://dbus.freedesktop.org/doc/dbus-specification.html#basic-types
[request-public]: /tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/data/org.freedesktop.portal.Request.xml:63
[request-impl]: /tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/data/org.freedesktop.impl.portal.Request.xml:24
[frontend-print]: /tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/print.c:140
[frontend-prepare]: /tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/print.c:210
[filter-local]: /tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/xdp-utils.c:267
[request-path]: /tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/xdp-request.c:202
[settings-local]: /tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/data/org.freedesktop.portal.Print.xml:94
[gtk-client]: https://raw.githubusercontent.com/GNOME/gtk/gtk-3-24/gtk/gtkprintoperation-portal.c
[gtk-file]: https://raw.githubusercontent.com/GNOME/gtk/gtk-3-24/modules/printbackends/file/gtkprintbackendfile.c
[gtk-cups]: https://raw.githubusercontent.com/GNOME/gtk/gtk-3-24/modules/printbackends/cups/gtkprintbackendcups.c
[gnome-print]: https://raw.githubusercontent.com/GNOME/xdg-desktop-portal-gnome/main/src/print.c
[requests-local]: /home/kevinard/projects/canopa/crates/canopa-portal/src/requests.rs:83
[lockdown-local]: /home/kevinard/projects/canopa/docs/adr/0036-lockdown-policy-owner.md:7
[ipp-model]: https://www.rfc-editor.org/rfc/rfc8011.html
[cups-api]: https://openprinting.github.io/cups/doc/cupspm.html#working-with-destinations
[cups-header]: /usr/include/cups/cups.h:479
[cups-options]: https://www.cups.org/doc/options.html
[chooser-adr]: /home/kevinard/projects/canopa/docs/adr/0026-file-chooser-portal.md:1
[process-local]: /home/kevinard/projects/canopa/crates/canopa-portal/src/process.rs:27
[preview-local]: /home/kevinard/projects/canopa/crates/canopa-files/src/preview.rs:210
[files-process]: /home/kevinard/projects/canopa/crates/canopa-files/src/process.rs:49
[print-tests]: /tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/tests/test_print.py:299
[cupsd]: https://openprinting.github.io/cups/doc/man-cupsd.html
[cups-files]: https://openprinting.github.io/cups/doc/man-cups-files.conf.html
[ippeve]: https://openprinting.github.io/cups/doc/man-ippeveprinter.html
