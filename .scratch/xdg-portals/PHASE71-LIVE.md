# Phase71 active: stock custom-paper delivery verified; PDF annotation/geometry work

LATEST MAINTAINED-RECEIVER CHECKPOINT (supersedes below). **All handles terminal**;
no question/approval/agent/build/native/commit/PR. Full18 ACTIVE, Printoptin,
automaticQt deferred. Read ADR0108 and print/receiver-maintenance-evidence.json.
Normal47003b288279.../unit9cce9bee.../integration0444f8ea... and client903 unchanged.
**Current receiver BF2842406d3f21687be2b4f989083acd66be0933ceae5922324c32717c0377a6**
in the shared print-fixtures checksum store. No installed service/queue/routing
or physical printing. ~5.3GiB free; original incremental/default graph reused.

Canonical vendor overlay `cups-ipp-receiver` pins official2.4.20 archive and
one-file patch tools/ippeveprinter.c (baselineaec786... -> corrected7daac7e2...).
Original CUPS LICENSE/NOTICE assets shared with auth overlay; all1301 files and
exact patch scope verified. New maintained root helpers build-print-ipp-receiver.py,
test-print-ipp-receiver.py and print-custom-ipp.conf. Builder requires current
client receipt+matching config/header/image and publishes one checksum ELF.
Client builder now binds config.h/Makedefs/config.status; old receipt reconfigured
once18.01s +library1.01s, **byte-identical903**. Current bound client receipt saved
immutably as client-build-bound-config-01.json. Don't repeat the one-time refresh;
ordinary --reuse now verifies bound config. Optional per-invocation client reserve
--min-free-bytes; default6GB, this native refresh used3GB. Receiver reserve3GB,
netgrowth cap256MB/time120s, no install/new Cargo graph.

Receiver build01/02 F38208... were reproducible but direct loader inspection
showed system CUPS without launcher env. **Do not use F382** as final candidate.
Builder now retains matching client once bychecksum and emits relative RUNPATH.
Final build03/04 **BF284... byte-identical**, default ldd AND actual no-library-
override11-case run map903. No ABI injection into system executables. Root helper
changes after shared preflight have direct build/loader/native verification;
vendor C patch and Rust sources stayed unchanged during that gate.

Maintained11-case caller: original unmodified3bb baseline **RED7**, exactly prior
six range failures plus strict MIME capability failure. Baseline restored from
verifiedXZ only for that read-only run and retired again. Corrected F382 explicit
client11PASS; final BF284 **no-env11PASS** incl exact declared PDF formats/no jobs.
Three build refusals PASS (unboundconfig/changedconfig/differentlib), no workspace
created. Old compiler's one const warning retained; no unrelated source patch.

Intermediate36 worker+stockfilters+F382 two-copy stampedPDF8sides PASS, all pages
RMS0/readablelabels;37/38 public/Iced bothappearances F382 PASS. **Final39/40**
standard frontend+normalbackend+Iced+stockfilters+BF284 both dark100/light125 PASS,
one-copy simplex andtwo-copy long-edge each: exactgeometry/deviceattrs/margins/
sides/order/labels/stamps, jobs completed9, nondefaultlogicalhints not reapplied.
17305/58281 terminal0. 74557 finalbuild03/42352repro04 terminal0. Earlierbuild and
native handles all terminal. Current target pipeline REQUIRES --receiver-build;
selects only verified canonical recipe/image/config. Public callback supports
CANOPA_PRINT_SOURCE_GUARD for current guard selection. Historical snapshots retain
their exact earlier source/arguments.

Required workspace75507 completed101 in1180.21s, original alltarget/default/
incremental/no-fail-fast graph. StrictClippy5.09s/testcompile1.65s PASS; Portal439lib/
55integration PASS. **Exactly same10 Filesintegration failures** as phase70,
only that Cargo targetfailed (596pass/10fail/41ignored); no video/lib failure.
Post96123 completed0: Settings0.8s/CLI3.61s/config2.81s/vendor10.42s. Shared gate
NOTgreen, no Files fix claim. All current rootnormal/unit/integration hashes
unchanged. No broad rerun needed solely for later docs/checkpoint updates.

New hostaudit04 PASS52receipts/**1006 recorded identities absent**, private paths
gone. Prior01..03 unchanged. Receiver finalguard1345 inputs verifies runtime/source
continuity plus later doc-only differences. Evidence indexes28 receipts and11
source-only replay files incl canonical receiver patch; bound client receipt and
canonical component recipe saved. Repro and loader-red receipts remain separate.
Old D55 prototype and F382 intermediate raw executables now **losslessly XZ
archived and retired** (81KB each; receiver-obsolete-archives-01.json). Their raw
paths no longer exist; restore bychecksum only when needed. Original3bb archive
intact; BF284 current raw remains. No per-case ELF copies/cache purge; about40MB
observed filesystem growth across maintained source/fixtures, not a new Rust graph.

NEXT advance **advanced-device03 driverless/PDF controls** using maintained BF284
receiver/configuration and stock configured filters, preserving actual wire/job/
PDF-or-raster effects, prepared geometry and user choices. Wider connection06,
actualFlatpak/release/deployment/defaultPrint, physical-device and sixexperimental
gates remain. HostFUSEexists but no FlatpakCLI/libostree/libflatpak at earlier audit.
No real printing authorized. Do not re-open resolved phase70/68/69 or receiver
range/MIME/loader investigations just because another turn begins.

## Public continuation checkpoint (historical; superseded above)

EARLIER PUBLIC CONTINUATION CHECKPOINT (superseded above). **All handles terminal**;
no question/approval/agent/build/native/commit/PR pending. Full18 ACTIVE, Print
opt-in, Qt automatic integration deferred. Current normal47003b288279... and
CUPS client903 unchanged; no product/Cargo/vendor edits or build in this turn.
No installed queue, service, routing or physical-printer change. About5.3GiB free;
no new ELF, cache purge, fresh target or per-case executable copy.

Actual standard frontend + normal backend + Iced + STOCK configured driverless
CUPS + receiverd55 now PASS both appearances: **native34 dark/100%,35 light/125%**,
each with one-copy simplex and two-copy long-edge stamped prepared PDF. A real
width edit revokes the automatically checked saved paper default; explicit
recheck and Iced approval succeed. Exact125.13x225.13mm page boxes within1e-6pt,
device dimensions/margins/sides,4/8 side order/labels and printed stamps on first/
fifth pages pass. Requested reverse/ranges/scale75/nup4 are not applied a second
time. PreparePrint intentionally asks app for1rendered copy; retained approval
expands2 physical copies. Backend keeps sealed15 image without mapping CUPS;
observed actual production workers map matching imageinode. Private runtime
selection explicit, not bundled-path or installed-desktop acceptance.

Handles19224/75044 exit0. Earlier31/32/33 remain FAILED fixture evidence:
31 assumed saved default was unchecked, but production auto-checks it;32 had
bytes/text oracle error AFTER public delivery;33 expected2copies in rendering
hints despite documented one-rendered-copy protocol (first case PASS). No product
change required. All three private desktops/queues retired. Don't reclassify
these receipts as successes or product regressions. 20554/45734/80472 exit1.

New host audit03 PASS44receipts/**738 retained identities absent**, all private
paths including failed public desktops gone. Prior01/02 unchanged. New
source-guard-public-paper-01 pins1338inputs and two target-only native scripts;
only ADR0107/context/Printguide/fixtureREADME changed after native checks. Runtime
sources and executable remain exactly the prior compiled439/55/PPD1 candidate.
`print/public-paper-evidence.json` indexes all5 public runs, sourceguard/hostaudit
and4 exact source-only replay scripts. Historic worker paper/appearance evidence
remains separate. Original primary public-paper.py checks the PRE-native paper
sourceguard; later doc-only differences are explicit in the new public guard.
Select current verified guard if preparing a future replay; don't restore/modify
runtime source or re-run checks solely because historical documentation differs.
No workspace rerun for this tooling/docs-only turn; prior Files failures remain.

NEXT **maintain corrected receiver source/provenance/build recipe** before final
custom-device gate. Existing official baseline sourceSHAaec786e5...; diagnostic
sourceSHA7daac7e2..., binaryd55 (221880B, stored once), library903. Receipts
receiver-formats-build-01.json, receiver-condition-prototype.patch and
receiver-formats-prototype.patch describe exact gcc command/inputs. Stable
source `target/print-ipp-receiver-compat/ippeveprinter-formats.c`; configured
headers in `target/cups-authentication-compat/cups-2.4.20`, includes original
tools/printer-png.h. Two measured fixes: inclusive media range membership and
preserving declared MIME attrs/docformats without duplicates. Prior unchanged
range10/strictformat11cases red/green already retained; don't re-diagnose them.
Read vendor maintenance/registry and retain upstream LICENSE/NOTICE/provenance.
Never inject changed-private-ABI903 into system cupsd/filter/backend. Diagnostic
receiver was compiled against matching configured headers. Libppd and qpdfjob
prototypes remain UNADOPTED. Broader advanced-device03/connection06, actualFlatpak,
release/deployment/defaultPrint, physical-device and six experimental gates open.
User has not authorized real printing. Keep original root incremental/debug graph.

## Worker/converter continuation checkpoint (historical; superseded above)

EARLIER CONTINUATION CHECKPOINT (superseded above). **All handles are
terminal.** No pending question, approval, agent, commit or PR. Full18 ACTIVE;
Print opt-in, automatic Qt deferred. Current normal
`47003b288279ff8bc3855005d783715b30198a6c15327624f9dd5f4639116b3b`;
current unit harness `9cce9bee3b1c8307f00164d32d62f70b7cccebed842fa84637255db0c13ee6a1`;
integration `0444f8ea341cc97c55ccc5711c59e6c73c9ac4b7687a2945974c78e9e34806bc`.
CUPS client903 and receiverd55 unchanged. No host queue/service/routing changes,
system filter installation or physical printing.

Geometry build01 handle96269 completed0: fmt10.82s, strict workspace
Clippy108.77s, unchanged default workspace/all-target build494.41s. Compiled
harnesses handle21120 completed0:439lib/55integration/publicPPD1, crate cwd and
private D-Bus. Current maintained annotation/geometry contracts PASS all6 A4 and
all6 fractional-custom cases (85424/19747 terminal). Exact qpdf boxes within
1e-6pt; printable and Print/NoView stamps present, screen-only omitted; correct
one/two-copy page order. Actual mixed/inherited crop/rotation/UserUnit and odd
duplex copy-boundary groups all PASS (6636 terminal). Original physical dimensions
including inserted blank backs preserved exactly. The preceding custom geometry
red remains failed (all6), and original annotation red59 retains4 failures.

Current normal **native28/29/30 PASS**, handle10117 terminal:
prepared stamped12513x22513, one-copy simplex and two-copy long-edge; direct
20999x29701 short-edge control. Actual worker -> owned private driverless CUPS ->
original stock configured filters -> receiverd55; no CLI/filter overrides.
Device dimensions/margins/sides,4/8 physical pages and exact expected geometry
pass. All before/after filter page pixels RMS0, readable labels/nonblank marks,
printed stamps remain on first/fifth copy pages. These are worker/converter/CUPS
checks, NOT public frontend/Iced/physical acceptance. Prior59 matrix22..27 remains
the full min/max/named/near-A4/saved-instance paper cohort; paper submission code
is unchanged in470, but don't relabel old receipts as current-binary runs.

Host audit02 passed39 receipts/**520 retained identities absent**, all private
paths/data gone. Audit01 unchanged. Source guard-paper-appearance-01 verifies
1338 current inputs; only post-test CONTEXT-MAP and Print guide differ from build
guard, with four additional documentation files recorded. Runtime sources and
binary unchanged. New ADR0107 and paper-appearance-evidence.json index23 receipts
and15 exact source-only replay snapshots under print/paper-appearance-replay/.
Docs0085/0091/guide/fixture README/context reflect the newer PDF printer path.
Only documentation changes followed successful native tests; no new build graph
or whole-workspace test execution. Earlier phase70 Files failures remain.

Storage free about5.3GiB; root incremental debug graph reused. e0/e7f baselines
losslessly archived once before overwrite (normal36.1MB/harness27.4MB XZ), earlier
59/ceabc archives intact. No cache purge, fresh target, per-case ELF copies or
cleanup request. Root current outputs remain needed. Exploratory libppd patch
and qpdf job prototype are diagnostic, UNADOPTED; no canonical vendor change.

NEXT: public standard frontend + normal backend + Iced revalidation of latest
prepared PDF normalization/annotations, respecting prepared geometry and both
appearances. Then maintain corrected receiver provenance/build recipe before
final custom-device gate; advanced-device03, broader connection06, actualFlatpak,
release/deployment/default routing and six experimental interfaces remain. Host
has FUSE but no Flatpak CLI/libostree/libflatpak (earlier host audit); no physical
printing authorized. Reuse existing checksum-addressed compositor/runtime stores;
do not re-open resolved phase70/68/69 seams solely due continuation.

## Earlier geometry-build checkpoint (historical; superseded)

EARLIER CHECKPOINT (superseded above). Full18 ACTIVE, Print
opt-in, Qt deferred. No question, approval, agent, commit or PR pending. No host
queue/service/printer/routing changes or physical printing. Only live handle at
this checkpoint: **96269**, `replay-source/build-geometry.py`. Re-poll it; do not
restart. Fmt/strict workspace Clippy passed; unchanged workspace/default/all-target
incremental compilation is running. Receipt `geometry-build-01.json` pins1334
inputs. Do not mutate pinned sources while it runs. About5.5GB free; guard3GB.

Submission build03 completed: normal59a78109... passes439lib/55integration and
the explicit public PPD case. Native **22..27 all PASS** through actual worker,
private `-m everywhere` CUPS, STOCK configured filters and receiver d55. No CLI
PageSize/media/scaling or filter-library override. Sizes:12513x22513 (rounding
edge),12500x22500 (named control),7620x12700 minimum/long-edge,21590x35560 maximum,
20999x29701 near-A4/short-edge,12501x22502 long-edge saved Fractional instance.
The last also passes4 metadata cases including range rejection and restores exact
default media/margins/sides. `stock-paper-matrix-01.json` passed; content checks
have readable PAGE1..4, nonblank marks and RMS0 on all4 pages. Device request and
retained attrs, margins, PDF geometry and observed process cleanup pass. Receiver
is still diagnostic, not a maintained/distributed fixture. libppd patch remains
UNADOPTED. Existing failed receipts remain failed.

Prepared annotated PDF revealed two further faults. Stock filter falls back to
A4 when annotations disable its no-scaling copy path. Canopa's own content-only
page reconstruction drops printable stamp appearances. Maintained
`scripts/test-print-render-annotations.py` is RED on59 (4 failures/2 screen-only
passes). Ghostscript `-dPrinted -dPreserveAnnots=false` plus PDF1-copy printer
normalization fixes stamps. Build `annotation-build-01.json`, normale0f45570...,
passes fmt/strictClippy/build,439/55/PPD1 (`annotation-tests-02.json`) and all6
annotation cases (`annotation-contract-02`). Test attempt01 failed only because
sandbox private D-Bus socket creation was denied; unchanged host run02 passes.

That candidate still rounds prepared MediaBox:354.699212598x638.163779528 becomes
354.7x638.16. Unchanged resolutions720/7200/72000 do not help. OfficialGS10.08
gdevpdf.c round_box_coord rounds to .01pt. Strengthened actual-worker contract
`annotation-geometry-red-01` fails all6 geometry cases while stamps remain correct.
qpdf job-API prototype preserves bounds but its form appearance generation has
documented limitations; it was NOT adopted. Ghostscript remains the converter.

Current geometry candidate instead explicitly inherits the sealed original PDF
FD alongside executable pin into the existing isolated qpdf layout worker, ONLY
for PDF copy mode. Pdf owns its read_memory allocation through cleanup, validates
bounded sealed sources/page count/geometry, compares converted physical sizes
within .02pt, and constructs copy sheets/blanks with the ORIGINAL exact physical
geometry. Flattened content keeps its converter coordinate system without scaling
or a second layout. Original path is never exposed or reopened. Pending actual
native verification; no successful candidate claim yet. Changed render runner,
runner/tests and pdf.rs; maintained fixture/test now support custom point sizes
and inspect exact qpdf boxes within1e-6pt. Default fixture bytes unchanged.

Baselinee0 normal280600160B archived once to36145856B XZ with verified restore
SHA; harness e7f94b6a...170357240B->27422420B. See annotation-normal/harness-archive-01.
Earlier59/ceabc,failed e528/5b48,3c50/e664 archives intact. Root outputs retained;
no cache purge/new Cargo graph/per-case ELF copies. No new whole-workspace test
execution: prior phase70 Files failures remain. Current CUPS client903 unchanged.

NEXT after handle96269 completes: adapt compiled verifier to geometry-build01;
run exact annotation regression A4 and fractional custom; then
`prepared-geometry-matrix.py --out ...` for mixed/inherited crop/rotation/UserUnit
and odd duplex blanks. This target-only probe reuses render-annotation.py.
Run native driverless-pipeline28+ with prepared-exact-stamp.pdf at12513x22513,
`--prepared --copies1|2`, stock filters/no overrides; target pipeline now supports
copy-only mode and validates4*copies pages. check-paper-content.py accepts
--copies. Then actual public/Iced and remaining gates. Preserve failed evidence,
refresh docs/issue/evidence/source seal after final edits. Existing host audit01
covers only1..14; write NEW audit02 for all runs, do not overwrite01. All earlier
build/test/native handles (including91346 matrix and42478 geometry-red) terminal.

## Historical submission checkpoint (superseded)

CURRENT CONTINUATION CHECKPOINT (supersedes everything below). Full18 ACTIVE,
Print opt-in, Qt deferred. No user question, approval, agent, commit or PR pending.
No host printer, queue, service, installation or routing change. Latest source is
not yet verified as a complete candidate: consult submission-build-03.json.

Build03 handle76922 was running the original workspace/default/all-target graph;
fmt and strict Clippy PASS, build still live at this checkpoint. Re-poll that
exact handle; never restart because observation expired. Build02 failed on a
missing test CStr import (now fixed); its receipt remains failed. Build01 normal
SHAe528bb5c... passes438lib/55integration/publicPPD1 but FAILED native19 before
job creation. It is not the final fix. Root normal can change while03 builds.

Latest product edits from phase70: cups.rs, cups/media.rs, cups/ppd.rs and
cups/validation.rs. Lookup count+name fix remains. Submission derives fixed
print-scaling=none because pages are already laid out. All caller-built CUPS
option arrays now sort ASCII case-insensitively (CUPS uses binary search).
PPD validation explicitly marks PageSize after bulk options, retains exact
matching named sheets, and checks approved dimensions plus all margins.
Custom records always get a point spelling selected just above the exact
dimension when needed, preserving integer IPP units through CUPS's f32 parse
and truncating backend conversion. New exhaustive1..1,000,000 dimension test;
expected final normal library439, integration55 and ignored publicPPD1.

Public probe cups-ppd-size-01.json proves two independent faults: unsorted arrays
lose print-scaling/color/PageSize lookups; sorting fixes lookup, but bulk marking
still lets media override explicit PageSize. Single ppdMarkOption fixes the
marked record; limits lookup does not mutate it. Native19 caught the original
strict check failure. Do not weaken geometry validation to make it pass.

Actual CLI diagnostic20 (archived3c50 worker metadata, stock filter) proves
Custom.125.13x225.13mm loses one unit: receiver x12512, requested12513. Diagnostic21
uses upward-representable point values and print-scaling=none; exact device attrs,
margins and four PDF geometries PASS without experimental filter library.
content-check-fonts-02 verifies all four pages pixel-identical (RMS0) with visible
PAGE1..4 and nonblank calibration marks. This is CLI evidence, not yet worker
submission acceptance. Run actual worker with stock filters after build03.

The earlier proposed libppd patch remains diagnostic, NOT registered or installed.
Official2.1.1 archive SHA b356aeed1335ef0ca7f799741782a2544e7acee63fb4b047b94e4e0395a9cb62;
target/print-filter-diagnostics/ has one-file patch68918a9c..., source review126,
prototype libraryCA6d8564... and upstream baseline6a553415.... Same maintained
filter-contract01 old RED /02 candidate four PASS. Original upstream test program
exits0 (45 PASS markers), stored once by checksum3f3fe0be.... Prototype matches
all126 original source files except the declared adapter source. Draft provenance
materializes correctly but is NOT root vendor metadata. Simple prepared PDFs can
use stock filter's no-scaling copy path, so do not adopt this dependency merely
because the prototype works. Broader annotated/prepared/raster behavior is still
being checked: prepared-annotations.py handle69562 /prepared-annotations-01/.

Filter-only snapshots: installed and upstream custom PageSize became A4; patched
adapter supplies numeric dimensions, margins and explicit PWG name. Geometry-only
CLI15 passes but content-check FAILS from ~11% auto-fitting. CLI16 no-scaling fixes
it. CLI18 named/asymmetric control passes geometry (with experimental library);
actual stock-worker controls follow. Preserve all failed receipts.

Visual oracle correction: default sandbox font cache is unwritable and renders
legal Base14 Helvetica as missing-glyph boxes, including the ORIGINAL source.
This was not a Canopa text-loss bug. check-paper-content.py now uses system font
config plus a private temporary cache and records NimbusSans file hash; it is
removed afterward. Use --output for old runs to preserve earlier receipts. Old
content checks still show mark transport but do not prove readable glyphs.

Current maintained source additions: test-print-custom-media.py (worker metadata),
print-paper-document.py (four-page centered calibration), test-print-paper-filter.py
(filter diagnostic, no jobs). Build03 pins1333 inputs including those. No Cargo,
dependency, feature or canonical vendor changes. No whole-workspace execution
repeated; phase70 Files failures remain. Source guard-current-01 (1332) is the
prior lookup seal, NOT current-source equality after submission edits.

Old3c50 normal/e664 harness and failed e528 normal/5b48 harness are losslessly
archived once by checksum before overwrite. Restored3c50 raw for CLI20/21 was
retired again after SHA verification; archive preserved. See lookup-restore*-02
and submission-attempt-01-*-archive.json. Current root outputs remain needed.
No new Cargo graph or per-case executable copies. About5.9GB free before build03;
its guard stops below3GB. No cleanup request needed.

All pipeline01..21 handles terminal/private cleanup; recorded descendants terminal.
Existing host audit01 only covers first14 (182 identities). After final runs,
write a NEW host-audit receipt; do not overwrite01 pinned by lookup seal.
Current selected CUPS client903 and honest receiver prototype d55 unchanged.
Receiver still needs maintained fixture provenance/build recipe if adopted for
final gates. Never inject903 into system cupsd/backend/filter; trial libppd applies
only to original pdftopdf process. No native pipeline live at this checkpoint.

NEXT after build03: verify-submission.py --run-label03 (439/55/PPD1), actual worker
stock pipeline22 at12501x22502 with no CLI/library override, then12513 rounding edge,
near-A4, min/max bounds, duplex and saved instance. Check wire/retained attributes,
PDF dimensions, actual nonblank content and process cleanup. Inspect annotation
probe before choosing any further dependency or normalization change. Public/Iced
and remaining Print gates still follow. Do not reopen sealed70 work solely due
continuation; no completion claim for the full goal.

## Lookup-only checkpoint (historical; superseded above)

This section supersedes the receiver-only checkpoint below. All build/native/test
handles are terminal. No pending question, approval, agent, commit or PR. Full18
ACTIVE, Print opt-in, Qt deferred. No host queue, service or physical printer used.

Current normal binary SHA
3c50f2705d2ee03f7700b44a9d2210290ecc31c0a62d740362b656e8e6b195a6.
Selected CUPS client903, receiver prototype d55, canonical eight-file CUPS patch
6ccbc6cc and recipe d26e70f2 remain unchanged. Two product Rust files changed:
print/worker/cups.rs and cups/media.rs. New maintained metadata regression:
scripts/test-print-custom-media.py; centered visible PDF fixture:
scripts/desktop-fixture/print-paper-document.py. No dependency/feature/target change.

The worker previously rejected cold fractional paper. Name lookup alone (e384)
also failed. Public API probes07/08 establish lazy catalog construction overwrites
the borrowed custom PWG record; numeric lookup additionally normalizes near-A4
dimensions even with EXACT. Initialize the catalog through public media-count
before explicit-name lookup. Keep support, exact dimensions and margins strict;
catalog count itself grants nothing. Review target/portal-print-71/review-product.patch
and fractional-diagnosis-02.json; the latter is historical tests-pending evidence.

Maintained real-worker contract09 is RED on old fdfa; contract10 is GREEN on3c50.
Contract14 also passes four cases with near-A4 209.99x297.01 mm, long-edge duplex
and a saved instance (including two out-of-range rejections), no jobs. Build02
passes fmt, strict workspace Clippy and the original default workspace/all-target
incremental build. worker-tests-01.json passes Portal437lib/55integration using
the already compiled harnesses from the crate cwd. Previous workspace Files
failures remain; no new whole-workspace execution or full-green claim.

Actual configured-driver delivery is STILL FAILED. Pipeline10 and boundary
trace11: worker approval and Render are 125.01x225.02 mm; private scheduler retains
that media keyword but supplies PageSize=125x225mm to configured filters. Actual
receiver request/retained job/PDF are125x225mm with different margins. Trace11
retains scheduler-job.out and cups-error.log. CLI diagnostic12 adds explicit
Custom.125.01x225.02mm PageSize: device attributes/margins exact, received PDF A4.
Diagnostic13 also changes media spelling; same A4 failure. These CLI probes are
not successful worker submission acceptance and no workaround was adopted.

Next concrete action: independently trace scheduler media-to-PPD selection and
installed PDF-filter custom handling; merely adding PageSize to Rust is already
disproved as a complete fix. Preserve configured filters and strict geometry.
Host packages measured: cups-filters2.0.1-3, libppd2.1.1-2,
libcupsfilters2.2.1.r32.gd49e9284-1, pdfio1.6.5-1. Upstream libppd2.1.1
ppd/ppd-filter.c lines693..745 removes media-col for Custom names; this is a source
lead, not yet debugger-proven runtime causation. No system/vendor modification.

Prototype source/replays live in target/portal-print-71/replay-source/. Private
pipeline runner supports metadata-only and explicit diagnostic CLI branches;
all raw per-run sources/receipts are retained. Pipelines01/03 used a corner-label
source cropped blank at100%, so only geometry evidence. New centered black
four-page source is qpdf-checked with PAGE-1..4 text. No blank-page comparison can
prove content preservation. Prototype receiver is not canonical/distributed.

Host audit pipeline-host-audit-01.json: all182 recorded identities from14 pipeline
runs absent and private paths removed (including failed runs). Old fdfa normal,
7111 unit harness and failed e384 normal archived once by checksum, losslessly
verified before Cargo overwrite; restored fdfa raw for regression09 was retired.
About6.0GB free after verification. No new build was needed for the latest trace,
metadata14 or unit/integration continuation. Do not request another cleanup.

See print/custom-media-lookup-evidence.json and issue02 for exact receipt paths.
Use current source guard1332; phase70 guard differs by two Rust files and updated
print guide. Future replay must use current normal3c50 and selected client903.
Do not rerun completed70 auth/supervision/packaging checks merely on continuation.

## Receiver-only checkpoint (historical; superseded above)

Phase70 is sealed. Its source guard1330 is still exact; all workspace/native
handles are terminal. Current normalRustfdfa/client903/eightfilepatch6ccbc6cc/recipe
d26e70f2 unchanged. Full18 ACTIVE, Printoptin, Qtdeferred. No pending question,
approval, liveprocess, agent, commit or PR. This turn made progress by completing
workspace/poststages/seal70 and producing new driverless receiver evidence.

Current task remains Print issues02/03 and broader deployment, not rewriting CUPS
as a product. See target/portal-print-71/receiver-diagnosis-01.json. No Rust,
canonical vendor, Cargo/dependency, installation or host configuration changes.
All new work is a bounded source-only diagnostic under target/ plus three small
native receiver prototypes. No new Cargo graph. About3MB extra after phase70.

Installed ippeveprinterSHA b0b8098a1dcfba468a9797da4fcd60d3a5caff18279eb8232212a90c4aae8959
from cups2:2.4.20-1; systemlibcups samepackageversion. Installed IPPSample absent.
A native read-only lpstat confirms an idle configured USB Brother DCP-1510 queue;
NO real document/job sent. Do not print there implicitly. No host CUPS edits.

Fast reproducer: target/portal-print-71/replay-source/validate-receiver.py.
It starts a private D-Bus plus loopback-only receiver (-r off) with archived
phase50 config, sends only Validate-Job/Get-Jobs/Get-Printer-Attributes via system
ipptool, and retires all processes/tempdata. No document/scheduler/Canopa/Iced.
An explicit --library applies LD_LIBRARY_PATH ONLY to the matching-source receiver;
system ipptool/dbus keep systemlib, verified receiver mapping. Never globallyinject.
Config: target/portal-print-50/experimental-fixture-sources/print-custom.conf.
Advertised range x7620..21590, y12700..35560 hundredths mm, PDF/default PDF.

Installed probes02/03 reproduce same matrix: namedA4 succeeds, four inrange sizes
reject with fidelitytrue, bothaxesout incorrectly accepts, oneaxisout rejects.
Withoutfidelity it returns successful-ok plus unsupported media-col (silentfallback).
01 only ran2 of planned10 before firstfail; rawplist authoritative, oldtest_count
was planned count. Do not claim01 ran10. 02/03 used-I, full10cases. Early controls
were tightened to require no unsupported media-col on successful validations.

Ranked hypotheses shown to user: inverse receiver range test, wrong configuration,
wrong client collection encoding. Actual source has inverse range predicate.
Pinned official receiver C SHAaec786e5ccd4f0532513e8026fb1e10a63115bf7bea6380e5239a726097831b1
matches untouched tools/ippeveprinter.c in the verified1301source CUPS cache and
archiveSHAab4d9cd7f3e58060091d2b24972223d6401675f11c49b65abf4f6ef31dea22ff.
Compile against matching existing CUPSprivate headers/client903, not host-privateABI.
First compile01 failed missing printer-png.h include; compile02 adds source/tools
include and succeeds1.488s/221840bytes. Receipts preserve both. No library build.
Baseline program SHA3bb0ed4045e806a673fc1f515df56cdb7870f38e2b61705af6936e2ec8881720.
Native validation04 red6fail/10, exact same probe source as05/06.

One-condition prototype replaces inverse predicate with inclusive x/y bounds.
Candidate sourceSHA474e3863d4f20967e4cefc9d9d3c7c719b5de979da88f454c0b979dc612c4900;
programSHA12c06eb2c804c590c358efb4356b11763fd92e27316b259e07a92e650e77cf06,
compile1.147s. Same10cases05/06 PASS. Interior, inclusivebounds, fractional,
oneaxis/bothaxesinvalid and no-fidelity/no-unsupported all verified, zerojobs.

Next strict capability query independently red: source adds document-format-default
and supported fields even when file already has them; loader ignores docformats.
07 raw11tests completed but JSON report failed on datetime. Preserve rawplist/out
and reporting-failure.json; no retained PIDs for07, do not invent them. Reporter
now preserves typed dates/data;08 red only duplicateMIME fields, retained identities.
Prototype fixes declaredformats loading plus adds defaults only when absent. This
is separate from range predicate, no response normalization. Same11case probe
source dcb6d48b3183a47955b9d7e4e234748b6879baeb2c3dc00728a473101f11707c:
08 red /09+10 PASS. getattrs actual PDFsingle/defaultPDF, proper declaredranges.
No Canopa delivery or physical acceptance claim. No unsupportedattr remains in
successful Validate-Job; Get-Jobs confirms no jobs created.

LIVE PROTOTYPE FOR NEXT STEP (moved without copy; old build path now absent):
target/native-test-snapshots/print-fixtures-by-sha256/d55f8c010413eb8a48e51d358d1fe9839ecd12264b98c74df0768c0bf8bebf3d/ippeveprinter
Program221880bytes, compile1.089s. Source:
target/print-ipp-receiver-compat/ippeveprinter-formats.c SHA7daac7e24bedd9250a99da890acfb3d6119297c71f22bc2370dd4af5f9760736.
CUPSlibrary903 stored at print-client-by-sha256/903.../libcups.so.2. ConfigH SHA
8f5e54b2bce0cf3a9ae7d7690e47169c5534ec55510f83a27d4960bb921a4f70.
Full command in receiver-formats-build-01.json. Same CUPSsource/cache paths;
Csources under print-ipp-receiver-compat are marked diagnostic, not canonical.

Obsolete baseline/range binaries losslessly compressed once by checksum and raw
removed AFTER decodedSHA verification; index receiver-artifacts-01.json. Archives
79092bytes each. Restore with chmod0755. Sources remain for review/provenance.
Keep current d55 raw once for next native run, no per-case copies.
18 retained process identities terminal; host audit no recordedPIDs and no args
matching unique Canopa validation-only receiver, covering possible07 leftovers
without claiming its unknown identities. All retained private data removed.
No process/build/agent running. Rootfree6.301GB at receipt, no cleanup requested.

NEXT CONCRETE ACTION: use this corrected private receiver in an owned actual CUPS
-m everywhere queue, with NO normalizing relay. Inspect strict capabilities, what
CUPS sends, retained job attributes and actual received PDF before claiming media
or controls acceptance. Add Interior/bounds/fractional, duplex and saved instances
from issue02; several resolutions/source/type from issue03. Preserve device vsPDF
geometry distinctions. Read rawworker Operation::CustomMedia/CheckPrinter/Render/
Submit in crates/canopa-portal/src/print/worker.rs and production.rs before writing
the small fixture. Render is a non-CUPS worker; do not force it through old Probe
which accepts CUPS operations only. Use unchanged normalRustfdfa + sealedclient903
for CUPS metadata/submission; existing runtime_probe.py can attach/verify those.
Submit existing fixturePDF alone would not establish custom direct-layout geometry;
use actual render path and inspect approved PageSetup/output MediaBoxes. Actual
public/Iced checks follow once configured-driver path is understood.

Do not patch/install hostCUPS or bypass configured drivers to force success.
Prototype receiver is NOT yet maintained/distributed: if adopted, first canonicalize
small vendor patch/recipe, preserve attribution and verify exact archive/header/
client provenance. RootAGENTS requires workspace after relevant shared/vendor edits;
do not rerun it now for target-only diagnostics. Stage70 checks are final and
should not be repeated merely due continuation. Physicaldevice and actualFlatpak/
release/deployment/sixexperimental remain separate open gates.
