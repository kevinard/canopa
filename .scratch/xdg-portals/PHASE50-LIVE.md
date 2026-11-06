# Phase50: scoped custom paper verified; physical precision remains open

FINAL scoped evidence: target/portal-print-50/acceptance-01.json and
print/custom-paper-evidence.json, seal23476 terminal0. Worker04/70836, metadata12921,
dark continuation3877 and light65997 terminal; dark successful prefix from54395
explicitly retained as a subset of its failed receipt.15groups/appearance,
152Print/12Lockdown/370PortalLib/55integration,93observedprocessesterminal.
Binary26ad253179ec35df72220789c42a62d199f588abe4b7836e22ada687cf621434.
All Rust/native/seal/docs checks terminal. Finaldocs9files/274links,11AST,39JSON,
whitespace/diff pass. Finalfree7076331520/net98177024bytes. FullgoalACTIVE,
Printoptin; physicalprecisionfalse/open. See CONTINUE.md for authoritative next work.

The following notes retain the actual chronological failures/fixes.

The user chose Finish Canopa Print features first. Qt automatic integration
remains open/deferred, measured in49; no Qt fork/routing changes.

Installed CUPS2.4.20 cups_size_t dimensions/margins are integer hundredths mm.
cupsGetDestMediaBySize public API accepts width/length/flags and yields concrete
geometry. DEFAULT0 permits nearest size; EXACT4 requires exact match. Approval
must also compare returned dimensions, not silently authorize a nearest sheet.
CUPS PPD custom min/max may depend on media-source/quality qualifiers, so inspect
actual API behavior and selected saved-instance options before the final model.
Primary docs: OpenPrinting cupspm.html#cupsGetDestMediaBySize and spec-ppd.html
(customPageSize/min/max), source v2.4.20 cups/dest-options.c.

Current Canopa discovery deliberately skips custom_min_/custom_max_ strings,
Capabilities stores only fixed Media, Draft exposes geometry only for file.
New source-only scripts/desktop-fixture/print-pdf-custom.ppd declares 3x5 through
8.5x14 inch custom range (76.2x127 to215.9x355.6mm),9point hardware margins,
existing PDF passthrough. Validate its PPD and use private owned scheduler to
measure supported in-range125x225mm vs outside range and current worker output.
No Rust edits or new Cargo graph yet. Reuse phase48 validation/binary until source
changes. Reserve6GB, cap3GB, root target/default/incremental/jobs1 throughout.

Probe42177 terminal; receipt custom-query-01/summary.json status measured, all3
observed processes terminal/no emergency cleanup. Actual CUPS accepts125x225mm
with top/left/right3.18mm,bottom12.70mm; rejects50x100/230x360mm and accepts
range endpoints. Unchanged Canopa only exposesA4 and incorrectly falls back toA4
for SavedCustom media=Custom.125x225mm. No jobs/uploads. Preserved as red/missing
feature evidence. No native checks live; Rust changes may now begin.

Implemented candidate: integer hundredth-mm dimensions, isolated exact media
lookup, CUPS-reported printable margins, saved-instance custom preference, Iced
custom controls with explicit check and edit/duplex invalidation, submission
revalidation before Create. Source is now FROZEN during validation/native work.
Original workspace/default/all-target/offline/incremental/jobs1 graph; reserve6GB
and phase baseline7174508544/cap3GB. Validation wrapper /tmp/canopa-print-50/validate.py.

Validation handle7485 live (worker-01); do not edit guarded source while live.

worker01/7485 TERMINAL101: compiler caught moved Destination in UI pattern guard;
fixed borrowed identity comparison via retained clone. No tests/build ran after
failed Clippy. Source frozen again for worker02. Native not started yet.

Corrected full validation handle98472 (worker02) LIVE, supersedes failed01.
Native source-only runner prepared /tmp/canopa-print-50/native.py, not started.

worker02 strict workspace/all-target Clippy PASS183.06s after format3.01s.
Same-graph compilation now LIVE; no guarded source edits. ADR0091 draft explicitly
marks native acceptance pending. Native runner still not launched.

Review concern found while build live: toggling custom off / selecting Printer
default for a saved custom instance currently leaves mediaNone and uses request
A4 geometry. Native runner includes a measured default transition group to
preserve red evidence. After current candidate checks/native terminal, fix by
resolving an explicit Printer-default selection to known concrete preference
(and check again if sides differ), and choose/show an explicit fixed sheet when
turning custom mode off. Add regression for selecting default after fixed sheet.
Do not lose this required fix or mark candidate accepted before it.

Validation98472 TERMINAL0: worker02 format3.01s, strictworkspaceClippy183.06s,
originalgraphbuild711.24s,151Print/11ignored,12Lockdown, fullPortal369library/19ignored
and55integration/2ignored. Guards unchanged; net350404608bytes. No fullworkspace
execution claim. Native now starting; known default-selection fix still required.

Native37635 TERMINAL1, custom-paper-dark01 setup failed before Canopa/Iced:
ippeveprinter rejects combining -P with legacy -f/-2 options. All3 observed
processes terminal, no cleanupkill. Corrected fixture to -P plus -Fapplication/pdf
(no Rust change); official2.4.20 main enforces exclusive attribute-source modes.
PPD mode advertises PDF plus other formats; fixture only sends/tests PDF. Removed
unused large harness from native executable pins (not used in this fixture).

Native54248/custom-paper-dark02 interrupted: wrapper exit143/SIGTERM, no final
receipt/cause established. Five real worker cases pass (exact125x225 geometry,
outside range low/high, altered margins and conflicting named paper beforeCreate).
Initial Iced capture inspected, actions written but not completed. All17 recorded
owned processes precisely retired by start-time/pidfd SIGTERM; cleanup terminal0,
noSIGKILL. Receipt marked interrupted, not passing. Added signal diagnostics and
per-action receipts; next replay uses prewritten actions, no mailbox wait.

Native26236/dark03 TERMINAL1 with19 observed processes allterminal/noemergency.
Fiveworker +fourIcedblocking cases pass. Confirmed/unconfirmed/out-of-range
captures visually inspected. Actualdirectdelivery125x225PDF4pages has no readable
PAGE labels: sourceA4 at100% is center-cropped by existing actual-size layout.
Fixture will select50% to fit rather than change layout semantics. Receiver also
reports legacy CUPSmedia-col12488x22471 vsapproved12500x22500, a new precision
concern. Measuring realCLI keyword/structuredcollection/both before productfix.
Source frozen for next metadata pipeline probe; default-selectionfix stillpending.

CUPS pipeline10197 TERMINAL0: keyword/collection/both CLI jobs ALL round custom
receiver geometry to12488x22471 and all margins318 on legacyPPD path.9observed
processes terminal, noemergency. Structured option is NOT a demonstrated fix; do
not introduce it speculatively or claim exact downstream physical dimensions.
Driverless probe TERMINAL1 beforefirstupload (lp error),3processes terminal. Its
generatedPPD only advertisesA4: ippeveprinter -P doesn't expose custom ranges.
Need truthful -a custom IPP attributes receiver for finalnative; legacydriver
quantization remains explicitly open/scoped external pipeline evidence.

After all native probes terminal, captured11 changed Rust files as candidate02
source snapshot. Fixed Printerdefault resolving known custom preference, explicit
fixed-paper choice when custom unchecked, changed-sides default reconfirmation;
added meaningful unit regression and inline size-error near check control.
Current validation41218 LIVE (worker03), source frozen, samegraph/budget. Native
next must select50% for directA4source on125x225paper, replay explicit default
selection, and record source/API/job/receiver geometry separately.

Validation41218 TERMINAL0 worker03: fmt2s, strictworkspaceClippy107.04s, original
graphbuild453.18s,152Print/11ignored,12Lockdown,370Portal library/19ignored and
55integration/2ignored. Source guards unchanged, net94371840 bytes. Current Rust
contains default-transition +inline-error fixes. No further Rust change planned.

CUPS-only normalized attribute probes76655/2633 terminal0 (3CLIjobs each,9/7observed
processes terminal), but media-col is dropped by ippeveprinter despite actual
CreateJob request containing it. Official2.4.20 valid_job_attributes range loop
explains incompatible test behavior; do not call these probes custom acceptance.
Initial -a probe30888/43204 terminal1: duplicate MIME/media-col attributes; kept
first declared MIME only with diagnostic testrelay, no invented metadata/request
changes. Raw/forwarded packets and strict ipptool outcomes retained. Experiment
conf/relay sources archived under50/experimental-fixture-sources and removed
from repo source; no new backend dependency/testnativeprogram/installed changes.

Current native rerun uses existing working PPD receiver, explicitly narrower
CUPS approval/PDF scope. Must record legacyreceiver12488x22471, not claim exact
physical/customIPP device acceptance. A4 directsource is explicitly scaled50%
to fit125x225sheet; layout semantics unchanged. Explicit Printerdefault selection
now follows fixedpaper transition. Full goal remains active; customdevice/
precision and broaderphysicalruntime gates open. Source frozen during native.

Native80054/dark04 TERMINAL1:10 groups pass including actual direct125x225PDF
withPAGE1..4 at explicit50% (legacyreceiver rounding recorded separately), then
saved instance capability lookup fails.25 observed processes terminal. Reduced
metadata42484/custom-query02 TERMINAL1 reproduces actual eventFailed Saved custom
paper unavailable;3processes terminal. No fallback toA4 is now permitted, but the
custom saved preference needs public PPD spelling resolution. Actual installed
pwgMediaForPPD tested125x225mm/cm and5x9in, invalidNone, layouts fromcups/pwg.h.
Added one worker-only public function binding and resolve Custom.* before exact
size/margins/device validation. All probes were terminal before Rust edits.
Source now frozen for worker04 validation; userfullgoalactive, Qtdeferred.

Current validation70836 LIVE worker04; PPD parsercasefold and mm/cm/lowercase
metadata cases added before sourcefreeze. Actual library resolver5case receipt
50/ppd-parser01.json confirms125x225mm/cm/lowercase,5x9in,invalidNone;32byteABI.
No native or metadata check live besides compilation. Keep source frozen.

Validation70836 TERMINAL0 worker04:fmt3s,strictworkspaceClippy112.04s,originalgraph
build441.63s,152Print/11ignored,12Lockdown,370Portal library/19ignored and
55integration/2ignored. Guards unchanged, net91209728bytes. Nextactualmetadata
uses currentbinary, fourinstances (base/mm/cm/lower), then native current04.
No further Rust edits planned; still freeze while metadata/native live.

Metadata12921/custom-query03 TERMINAL0 measured:4cases/base+savedmm/cm/lower,
all correct125x225 geometry/margins,5observedprocesses terminal. PublicPPDparser
fix now actualgreen. Sourcecandidate04 snapshot11files saved (no binarycopies).
Native19893/dark05 TERMINAL1:11groups pass includingactualdirectPDF and savedcaps;
thenGTKprobe exits before newdelivery. Its completion path was pointed at prior
existing JSONL receipt; that can end the caller's mainloop before Print transport
finishes. Fixture now givesGTK a fresh receiver-created per-case marker; JSONL
receipts remain append-only.26processes terminal/noemergency. No Rust change,
reuse04validation; native source-only correction. Finaldesktopacceptancepending.

Native54395/dark06 TERMINAL1:12groups pass incl actualGTKpreparedcustom6pages
after freshmarker, proving fixturelifetimecorrection. Then explicitQt prints
correct6numberedpages butPDF354x638points versus125x225mm (354.331x637.795pt),
so blanket .1point geometry assertion fails.35processes terminal/noemergency.
Prepared authority requires preserving the toolkit's actual physical page, not
re-layout toforceexactmm. Fixture now retains actualsmallQt sourcePDF, asserts
its three sourcepages within nearest0.5point ofapprovedsize and exact source*2
geometry inreceived6pages. Otherdirect/GTK .1point checks unchanged. No Rustchange.
Do not claim Qt clientquantization is a Canopa rendererfix or automaticQt routing.

Dark continuation3877/dark07 TERMINAL0,5groups/22observedprocesses allterminal.
ActualQt sourcePDF retained; source3 pages354x638pt and receivedsource*2exactly.
Preparedapprovedhint125x225mm preserved; toolkitpointquantization distinguished.
ExplicitPrinterdefault restores125x225 afterfixedsheet; changingPPD tofixedonly
betweenPreparePrint/Print yieldsResponse2/no newjob. Source/Rust unchanged from
validated04. Dark coverage combines12currentbinarygroups fromdark06 (its final
Qt blanket assertion remainsfailed evidence) with3remaininguniquecases indark07;
do not relabeldark06 passed. Lightfullmatrix nowstarting, samebinary/validation04.

Light53137/light01 TERMINAL1:9worker/Iced groups pass, then directPDF blanklabels
again becausefixedScale pointer was outsidevisibleclipped viewport. Actualoutput
1024x640logical at125%, helper760x640; Scale belowviewport.19processes terminal.
No productchange. Next source-only replay scrolls with existingfinger-scroll
fixture, capturesfield, then reads boundedtailinstructions. /tmp/canopa-print-50/
actions-light.json pauses at light-tail.json (absent),180s bound. Native ready
image scrolled-for-scale.png; primary mustinspect and write tail usingcorrect
logicalcoords,50%Scale/capture/approve. Rust remains validated04/frozen.

Light65997/light02 TERMINAL0 full15groups/31observedprocesses terminal. Scroll
confirmedScale field atlogical500,504; tailentered50 andapproved. Captures inline
error, scrolled50%, saveddefault and darkQt inspected. Seal23476 TERMINAL0 verifies
allcurrentguardedsource/binary pins and93terminalprocesses acrossacceptedreceipts,
15groups/appearance; sourceSnapshot11files. Startup lddnoGTK/CUPS/qpdf. Measured
phase+97984512bytes,free7076524032; noexecopies/newnativeexes/cachecleanup.
ADR0091/guide/map/PRINT/ROADMAP/fixtureREADME/issue02 updated. Finaldocchecks pending.

Final documentation check terminal0:9docs/274local links,11Python AST/no bytecode,
39JSON, newdoc/PPDwhitespace andgitdiffcheck pass. Receipts/evidence updated with
checks/finalstorage. CONTINUE next is remainingPrint features; no currentchecks
live, no pendingquestion/approval, fullgoalactive. Phase50scopedwork finalized.
