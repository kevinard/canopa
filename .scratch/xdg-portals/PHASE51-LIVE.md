# Phase51: printer source/type/quality/resolution investigation

Prior turn PROGRESS: phase50 scoped custom paper accepted; full18goal active,
Print optin, Qt deferred by user. Eleven packaged/six experimental unchanged.
No pendinguserinput or externalblock. All prior50tool jobs terminal.

Start by measuring installed public CUPS destination API before Rust changes:
source/type keyword values; quality enums3/4/5; resolution tuples withdpi/dpcm;
savedIPP and PPD spellings; reported combinations/conflicts and customgeometry.
New owned PDF advancedPPD adds InputSlot/MediaType/cupsPrintQuality/Resolution
and manual+glossy constraint. No app/Cargo/vendor/Rust changes yet.

Budget: stable root target/default/workspace/alltarget/offline/incremental/jobs1,
6GBreserve/3GBphasecap, baseline7075774464free. No binary copies/newnativeprograms/
cachecleanup. Reuse native51source-only privateCUPS probe and storedCUPSexecutables.
Current50binary26ad253179ec35df72220789c42a62d199f588abe4b7836e22ada687cf621434,
worker50/04 validation may be reused until sourcechanges. Never edit guarded Rust
or livefixture inputs while validation/native checks are active.

Investigation terminal: advanced-query01 handle20138 and ppd-query01 handle88332
both exit0, six owned processes each terminal; no jobs/uploads. Fresh mapping01
local ctypes command terminal0; public option192/choice144 byte layouts verified.
Actual source manual/tray-1, type stationery/labels/photographic-glossy, quality
3/4/5, resolutions300/600/600x1200dpi. Job constraints omitted by destination API;
PPD conflict count1 and Manual conditional max127x203.2mm observed. Public
cupsMarkOptions maps source/type through media composite, not standaloneIPPkeys.
Fresh objects confirm customPageSize and canonical source/type mapping.
Bounded cupsGetFd HTTP200 pipe2776bytes, memfd+ppdOpenFile works without disktemp.

Implementation now starts: standard advertised IPP controls and saved PPD alias
resolution; worker-only optional bounded PPD adapter for mapping, constraints and
conditional sizes; IPP conflict check plus fresh preCreate revalidation. Full
settings check in Iced tied to destination/options/generation. No new dependency.
Baseline small source snapshot target/portal-print-51/sources/baseline-01.

Rust implementation: settings.rs fixed four controls/values; caps boxed controls;
JobOptions settings; bounded PPD pipe/memfd adapter with fresh objects, public
IPP/PPD conflict and conditional custom limits; shared query/preCreate checks;
Iced generation+destination+exactoptions confirmation. SavedIPP/PPD aliases and
PageSizecustom fallback. No dependencies/Cargo/vendor/routes changed.
Lint01/37432 terminal101: unusedimport/arrayDefault; lint02/68716 terminal101:
enumsize warnings. Corrections made only after each terminal. Lint03/30970
currently LIVE, source freeze. Rootworkspace/default/alltargets incremental reused.
Explicit publicPPD runtime unit intentionally ignored in ordinary suite; run with
installednative CUPS afterbuild. Worker source-only fixture controls.py prepared
for presets, conflict/customlimits, invalid submits and driverchange beforeCreate.
ADR0092 status explicitly validationpending; no acceptance claim yet.

Lint03/30970 TERMINAL0: formatting3s, strict workspace alltargets Clippy183.06s;
sourceguards unchanged, netphasegrowth10158080bytes. Full validation now starts
with identical source guards and reuses this lint receipt. Keep Rust/PPD frozen.

Worker04/82994 TERMINAL1 solely explicit PPD runtime SIGSEGV. Build727.25s and
154Print/12ignored,12Lockdown,372Portal library/20ignored,55integration/2ignored
all pass. Do NOT label overall receipt passed. Phasegrowth302559232bytes,6GBreserve.
Diagnosis skill applied; loop publicPPDruntime no core dumps. GDB33797 terminal0
shows testthread exit callback into unmapped code, libcups absent from mappings.
A/B same old harness: baseline SIGSEGV0.15s; LD_PRELOAD retaining libcups passes
0.023s. Layout/object failures falsified by all assertions passing when retained.
No Rust instrumentation, extra executable or core dump used for these probes.
Actual private-CUPS controls-worker01/87314 TERMINAL0:17 cases,24 observed owned
processes all terminal, no jobs/uploads/emergency cleanup. All four controls,
savedIPP/PPD aliases, invalidpreferences, conflicts/conditionalcustomlimits,
invalidsubmit and changeddriverbeforeCreate pass on candidate01. Native public
Iced fixture has not run; worker04 overall failed so it must use newvalidation.

Now fixed process-lifetime OnceLock library handle, still only lazyworker loading;
returned prepared-client quality/source/type and integerDPI x/y hints; preserves
rawDPCM transport and rounds up onlyrenderhints. Measured GTK scalarencoding in
renderer-hints-query01. UIerrors now besidecontrols, customconfirmation hidden
until entireoptioncheck passes. No incoming apppref restoration added; broader
app/preference/auth/device acceptance remains open. New source validation next.

All prior handles terminal before candidate02 edits. Full worker05 /45271 now
LIVE, Rust/PPD FROZEN. Candidate02 source15files archived; prior service baseline
reconstructed by removing only newblock and SHA-verified against worker04 input.
Core dumps disabled in validation wrapper. Public native fixture renderer hints
assertions updated but not launched, must select final passedvalidation receipt.

Worker05/45271 TERMINAL101 at lint only: explicit drop(api) became obsolete now
that API holds process-lifetime library reference. Changed preflight to lexical
scope after terminal, same two identity checks retained. Worker06/67446 LIVE,
fullvalidation/sourcefreeze. Reconnect regression required for Api/recovery change.
Prepared scripts all corelimit0 and final passedvalidation/sourceguards required;
controls final adds rawqueue metadata fallback; new native/reconnect not yet run.

FINAL VALIDATION06/67446 TERMINAL0: format3s/strictworkspaceClippy112.04s/original
graph build490.14s,155Print(12ignored),12Lockdown,373Portal lib(20ignored),
55integration(2ignored), publicPPDruntime1 all pass; sourceguards unchanged.
No preload in fixed regression. Currentbinary SHA
ca82d1c156254ea33286419f451c86a03ec6a3b9f95af7257c8c9d4b86b1d354.
Final controls02/22922 18cases25processes; reconnect01/81877 5cases8processes;
advanced-raster-dark01/34182 4groups26 and light01/7249 4groups25; custom-regression
dark01/34190 6groups24 and light01/31679 6groups31. All139 observed final terminal.
All active validation/native handles now TERMINAL. No guarded Rust changes since06.

Dark initial PDFnative01/95401 failed downstreamfields despite correctUIresponse
and CUPScontrolfile (public ippReadFile inspection saved). Source/type/quality
verified in acceptedCreateJob correlated bydevicejobid on finalrasterfixture;
4actual pageheaders600x1200DPI. Receiverjobenv omitsmedia-col so do not use it as
source/type authority. Normal source-only relay removes known duplicateMIMEattrs
only, preservingdeclaredvalues and jobrequests/responses. NewprivateRasterPPD
changes onlyPDF->gstoraster output, reusesinstalledfilter/gs; no executablecopy.
Independent pipeline01/11191 duplicateMCS/errorfloodfailed;02/61178 unsupportedPDF
inputfailed;03/85591 incompatible-a/-f receiverflagsfailed; rasterpipeline01/27329
completed4jobs22processes but oldflag expected3pages (actual4), fieldsfalse inenv.
Those are measurement/failed evidence, not finalacceptance. Sourceinputs archived
where needed. Allthesehandles terminal too. Finalnative usesactualsourcepagecount,
rawacceptedIPP and public rasterreader withboundedrowbuffer/1GBdecodedcap.

Visual darkconflict/menu/corrected and light125menu/confirmedscrolled/customdefault
inspected. Source/type/quality/resolution remainreadable; correctedPrintenabled.
Netstorage about151MB sofar,6GBreserve intact; no nativefixturebuild/binarycopies/
newtarget/cachecleanup. DefaultPrint/Qtpriority/sixexperimental/fullgoal unchanged.
Docs updated; final source/process/storage/doc seal remains to run.

Final source/evidence/document seal uses acceptance-01.json; its status is the
authority. No additional Rust, runtime or installedstate changes are planned for
this phase. Next fullgoalwork is supported initial app preferences, thenauth and
remaining Print/device/failure/runtime/default packaging gates.
