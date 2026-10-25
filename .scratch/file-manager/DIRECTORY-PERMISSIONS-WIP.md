# Directory permissions — active implementation checkpoint

Current checkpoint (supersedes historical running-session notes below): settled
pipeline49499 TERMINAL0. Focused22 parents/1 helper passes30.08s; full core326
parents/21 invoked helper definitions passes602.08s across35 Cargo targets;
default build72.19s, fmt7.16s pass. All-target Clippy55662 terminal0 (57.85s).
102 hashes frozen/verified in directory-permissions-compiled-source.json.
Binary26cd734718a717da9d83e792495ec12ffd8bb8450be6e678fd3914f774384fcf.
Static71460 terminal0: fixture AST/native inventory/whitespace/packaged docs
(24 files39 links). Native123/24681 dark Recovery23,124/31163 light1.25 Recovery23,
125/53756 dark Retry22 and126/84268 light1.25 Retry22 ALL TERMINAL0, PTY private
Niri/D-Bus:90 checks,46 captures all inspected via diagnostic contact sheets
(first protected-folder capture also individually). Logs have no GTK warnings,
criticals or panics. Secondary filesystems removed. No compiler/private fixture alive; the existing host Niri session is preserved.
ADR0053 accepted only for recorded permission scope; proof in
 directory-permissions-verification.json. Full PRD ACTIVE, continuation PROGRESS;
no blocked/completed status. Next public tracer below: readonly retained-stage
cleanup. Preserve unrelated Shell research and all historical artifact logs.

First full core48177 terminal0:322 parents/21 helper definitions,666.26s. Its
102-path source/proof is an explicitly intermediate candidate before root-epoch
fix, not final evidence. New tests add4 parents, final expected326/21. New-root
and approved-Merge foreign replacement repro91426 terminal101; fixed by keeping
created-stage/approved identity instead of reading destination back. Both foreign
555 and writable700 stay untouched. New postinstall refusal counts installed
root once and Continue retains exact readonly creation authority; initial source
read failure makes no empty output or Undo. Focused22 now all pass.

Full PRD goal active; this is progress. Descriptor authority for every child,
readonly-stage GC, request lifetimes and broader acceptance remain open.

Previous goal turn classified progress: ADR0052 tiling + selection contrast,
scoped proof operations-tiling-verification.json. Full goal active. Root/Files
instructions/context and relevant ADR0006/0045 read. Diagnose/codebase-design
skills applied (already announced earlier conversation). -j1/incremental only,
private fixtures, .git read-only; no subagents or native sessions. Old compiled
source100 final binary0eefb305...; old core304/20 invoked helper definitions.

New public seam directory_permissions.rs:5 parent tests/1 child helper. Child-only
umask022/077 via sh exec; no global umask mutation. Private700, read-only555 roots
and nested555, cancelled+Continue separate Undo, Recovery with source/originals
preserved. Parent repairs only its own private tree for TempDir cleanup on errors.
Red22329 terminal101:private700 becomes755 in all Copy/cancel/Recovery at022.
Probe40223 terminal101:all5 parents fail;077 yields700 including readonly555;
private-parent700 has no effect at022; wrong mode present from first Progress,
not applied then reset. Ranked hypotheses shown before probes:default creation,
parent inheritance, later reset. Default creation confirmed by independent umask,
parent change and progress observations; source metadata captured but never used.

Production implementation in progress (NOT yet validated): schema18 adds durable
per-lane directory_access records (path/raw identity/parent/original mode) and
runtime opened directory FDs. Record-before-fchmod temporarily adds owner rwx;
restore through same FD live or guarded original location on restart; keep an
externally edited mode. Copy installs initial source0777 bits from private stage,
never exposing a private700 folder as755. Each transfer iteration restores the
prior loan and grants only an explicitly created captured destination parent.
CreatedFolder/Copied journal rows carry optional access_parent identity, including
resumed steps whose original CreatedFolder entry may expire. Identity refs bind
that parent. Undo permits these parents or an original parent physically recreated
by prior Undo; no blanket chmod of unowned Merge targets. Restore between Undo
iterations, and on normal/error exits. Recovery inspection exposes pending access
restoration and independent lanes retain separate authority. No public API added.

First compile/test73824 started; inspect directory-permissions-first-green.log
and authoritative handle. Do not edit Rust while compiler runs. Need finish
compiler errors/failing tests, remove [directory-permissions] temporary logs,
then add strong public mode tracers: unowned readonly Merge refusal, external
mode edits/replaced parent guards, expired prefix Continue/Undo ownership,
real cross-device Move failures/source retention and readonly source Retry/Undo,
process kill during live permission loan + restart restoring correct modes.
Need check startup pending ordering and source permission restoration before
permission record clear, preserved 17 input migration/readers future refusal,
full core/Clippy/build/fmt and actual native ordinary Retry/Recovery dark/light
on settled candidate. No native model assertions/SQL output assertions.

Potential follow-up (not reproduced): GC of expired retained stages under new
readonly copied parents may leave empty cleanup directories after payload cleanup;
trace through public retention/recovery before altering eligibility. Broader
source/request lifetime, descriptor/media authority, durable jobs, cleanup/
disposal, Trash/Restore, integrated Downloads, FileManager1/release/workspace
checks/review remain in the full ledger. Never mark full goal complete/pause.

Compile73824 terminal101: missing3 optional-field patterns/one trait;16275
terminal101 after compiling, all5 parents failed on readonly folder install.
Observed EACCES before root Progress when moving555 stage payload across parents.
Linux rename needs owner write for a moved directory's .. update. NewCreatedFolder
journal mode:Option<u32> now captures final source0777 mode; install working
(mode|0700) inside private stage, then restore through pre-opened directory FD
after rename and before append/progress. RecoveryForward restores final mode
before append too; lost created object with missing stage payload retains its
intent for attention instead of guessing uncommitted. UI CreateFolder modeNone
retains prior behavior. Old readers must refuse18. No full phase proof yet.

Focused55895 terminal0:5 parents/1 invoked fixture definition pass in0.72s after
25.62s build. Covers both umasks022/077 private700/readonly555/nested555 Copy,
Recovery, cancel/Continue/separate Undo and private destination parent. Temporary
[directory-permissions] logs still in test and MUST remove after diagnostics.

New public lifetime regression suite11 parents/1 helper started4752; log
directory-permissions-lifetime-red.log. New3 actual SIGKILL parents: during Copy
precommit, postcommit and ChangedCopy Undo review while temporary access is755;
startup must restore555, clean stages, keep recorded prefix/newer data, Undo.
Other3: swapped output restores actual opened original and reconciles after its
return; external mode701 survives completion/KeepFolder; unowned readonly Merge
never auto-chmod. Parent fixture now Cleanup guard repairs private tree only after
all parent assertions, including kills, so failures do not mask observed modes.
No source edits until4752 authoritative terminal. No native/other compiler live.

Predicted gap to verify before fix: Store currently clears mode records after
live error even if a Preparing intent remains blocked by replaced/unavailable
parent. After restoring actual directory mode555 through FD, returning it leaves
staged cleanup unable to write, with its earlier access authority forgotten.
If reproduced, retain permission record until same-lane pending mutation clears;
before reconciliation grant only recorded mode/identity/original-parent matches,
restore mode again even on failure. Do not blanket-chmod replacement folders or
changed external modes. Need test engine reopen with original held away too.

Still need actual readonly cross-filesystem Move retry/Undo, expired prefix Copy
Continue+Undo parent authority, creation commit kill phases, metadata/recovery
ordering, full core/Clippy/build/fmt/schema-upgrade checks and native Recovery/
Retry acceptance on settled candidate. GC readonly-stage tracer remains a possible
next dependency, not reproduced. Whole PRD active, no blocker/agents; preserve
older proof/unrelated Shell research. All earlier handles terminal.

Lifetime4752 terminal101 only wrong enum spelling KeepFolder; corrected UI Keep
folder maps public UndoChoice::KeepItem.22226 terminal101:10 parents pass/1 fails.
All3 crash scopes, external701 edit and unowned readonly Merge pass; replacement
loses its Preparing journal (interrupted list empty) because clean_stage treats
missing stage under replaced parent as cleaned. Probe actualcapturedModeRecord
provides correct physical-original guard before reconciliation.15575 terminal101
after that one-variable prepare_directory_access guard:interrupted now retained,
but returning original555 fails Retry EACCES (exactly line214). Second cause mode
record prematurely deleted while pending journal remained. Restore now retains
record/FD until same-lane pending mutation clears, re-granting only recorded
original/working mode and physical path/parent match before reconciliation.

New317? expected final core baseline304+13 parents=317, helperdefs21 (not run).
Expanded suite4239 terminal101:12 pass/1 fails,2.47s. Replacement/recovery mode
rights now pass; expired original folder-prefix history Continue/Undo passes.
Actual two-device readonly Move retains every source on first failure; after
explicit user chmod of source/root+nested to700, exact Retry Replace succeeds
and removes source tree, preserving captured555 destination modes. Undo of that
new entry fails EACCES at line317. This is not narrowed to private-only folders.

Ranked Undo causes shown:returned-folder install, later child return parent
access, saved-version restoration. Targeted test-only probe8441 running to record
whether original source root exists after failure. Logcross-install-probe. No
source edit while compiler runs. New StageCopy555 issue already traced to Linux
rename requiring owner write; restore_directory still sets555 before rename and
likely needs same durable working-mode/install/final-mode pattern. PendingUndo
already contains RemovedDirectory.mode and stage.payload_identity for startup.
Need apply final mode before state.reversed_with_recovery clears intent.

Potential next child-access dependency: SourceRemoved Undo both returns files
into a recreated readonly original parent AND removes its owned current output
copy from a readonly destination. permit_undo_parent presently covers only its
original recreated parent. May need optional access_parent identity for current
copy parent on SourceRemoved too, captured from same transfer directory plan,
with distinct identity-ref role (CURRENT_PARENT beyond existingPRIMARY0,PARENT1,
STAGE2,BACKUP3). Do not infer ownership from file paths alone. Reproduce after
root install fix if needed. Temporary test logs remain; remove before final.

Cross probe8441 terminal101 confirms source root absent after Undo EACCES. Plain
restore_directory working-mode fix19699 terminal101 still root absent: actual
callsite is moved_directory::prepare_owned -> PendingDirectoryDestination, not
plain restore_directory. Added DirectoryMerge.destination_mode:Option<u32>
(defaultNone old/existingMerge), captured in both new return-root constructors.
Work mode|0700 installs, recover_destination restores recorded final mode before
clearing intent. Generic directory-mode helper now reused by ordinary copy,
plain RemovedDirectory return and destination-frame recovery; read-only555 and
500 installs no longer fail Linux .. update.17128 terminal0 cross-Move case passes.
No SourceRemoved extra permission field/new refs role was needed: paired copy
cleanup already carries access_parent. Earlier speculativeSTAGE2/BACKUP3 roles
were incorrect; identity_refs has only0/1, untouched schema.

Added output-readonly public Move case: successful cross-device Move of700 roots,
then chmod its actual copied output root/nested to555, Undo must return originals
at700 and remove owned output containers.84478 terminal101:13 parents pass/1
fails.6716 targeted probe terminal101:all a,z,nested bytes already returned; old
output nested is empty and root555. Confirms missing parent access for owned
folder cleanup, not file return or wrong recreated mode. Added permission grant
through exact MovedDirectory.created cleanup-owner key, plus only newly prepared
return parents marked destination_modeSome via existing directory_members graph.
No filename/global-inode inference, no chmod of existing unowned Merge targets.
76536 terminal0:all14 parents/1 invoked helper pass in5.15s.

New3 public startup input fixtures (SQL preparation only; no SQL assertions) cover
PendingForward CreatedFolder.modeSome before install, after install before chmod,
and installed root moved away with foreign occupant. Recover appends only actual
install, restores555 before Undo and retains missing-installed-root journal until
original returns. All3 pass. New retention parent101 actual readonly copies then
100 Undo exposes trim running while last loan still active.7013 terminal101:
17 pass/1 retention failure.23746 targeted probe terminal101:all operation IDs
distinct and public interrupted lists empty at each returned Copy, but final
Undo label still present. Confirms ordering, not leaked record or request grouping.
Moved trim to outer run_transfer AFTER with_directory_access; one trim per call.

All temporary [directory-permissions*] logs now removed from Rust tests (diagnostic
artifact logs preserved). Copy/Continue/Recovery modes expanded700/555/500/750,
normal Copy roots include non-UTF8 byte0xff to exercise permission journal raw
paths. Replacement test now drops/reopens engine while original is held away,
checks foreign700/original555 unchanged and pending visible, returns original
then reconciles. cargo fmt --all34407 running; do not edit Rust/compile until
formatter terminal. Next focused18 parents/1 helper, full core expected322 parents
+21 helper definitions, Clippy/default build/fmt/static/native settled source.
Need record ADR0053 and docs. No native session live; all other handles terminal.
Potential GC readonly-stage cleanup issue and original-directory descriptor
authority remain open tracers; not proven solved by these permission cases.

Current formatter34407 TERMINAL0. Final-source focused+core pipeline48177 is
RUNNING; permission18 parents/1 fixture passed (26.73s including build), core
currently progressing through saved-stage tests, no known failure yet. Latest
write_stdin confirms same handle live; do not restart on observation timeout.
No Rust edit while core compiler/test process runs. No native session live.

Retention23746 terminal101 falsifies leaked-record/request-grouping alternatives:
every Copy has distinct result.id, public interrupted_operations empty at returned
boundary; after101 copies+100 Undo label remains. Trim moved after final access
restoration in outer run_transfer, removed duplicate inner trim. Temporary debug
logs removed from Rust; source modes Copy/Continue/Recovery700/555/500/750 and
raw0xff Copy root names pass in focused18. New3 PendingForward.mode input states
before/after/relocated install pass; SQL only prepares input, Engine/FS assertions.
Copy roots through returns/decompUndo preserve recorded final modes; missing
installed roots retain recovery rather than discarding intent. Actual cross
Move cases include readonlysource->explicit permission repair->Retry->both Undo
and writableSource->Move->external chmod output555->Undo all original700 modes.

Native Recovery fixture now uses saved root+nested555, asserts modes after
Cancel/Continue/Undo. Independent exclusive file-arrival thread retries only
FileNotFound/PermissionDenied until the substantial first child's writable
interval; never chmods/overwrites application output. Basic checks19, tiling23,
expiry22 expected; syntax passes. Plan native tiling dark/light+ordinaryRetry
dark/light after settled default build, sequential (tty=True recommended after
last turn's external SIGTERM interruptions; never suppress signals).
ADR0053 draft, context index and README permission paragraph written. ADR status
still implementation/verification in progress. Exact final proof not created.

CRITICAL next dependent ownership tracer identified in source review: normal
DirectoryFinish.destination_identity is currently Identity::read(destination)
after commit_forward. An external root swap between rename and readback can be
adopted as a created parent, incorrectly authorizing permission grants to an
unselected foreign directory. This predates current work but new permission
authority must not rely on it. Full descriptor authority remains unproven.
Need correct public seam: existing transfer_with_check lacks postinstall root
check before capture. Add useful platform DestinationFolder checkpoint at newly
installed/approved root, then regression callback swaps root once it exists
before old late identity read. Before fix should write into foreign root (555
case incorrectly loans access,700 case reproduces without permission loan).
Also test an approved Merge root swapped to foreign700 at same boundary. Do not
claim existing copy-created permission provenance solved until traced/fixed.

Expected fix: capture destination identity from own Stage payload before commit;
for Merge retain approved existing Version.identity, never read/adopt later path.
Capture directory map/ownership before postinstall placement check so a refused
check returns exact typed plan. Preserve original DirectoryFinish across root
reprocessing (including old created/operation/mode and completed_beforeMAX) and
auto-Merge only that captured actual root so Continue doesn't adopt foreign/lose
created-parent authority on readonly root. Pre-enumerate directory before install
to avoid creating empty output/history on initial read failure. Count actual new
root creation once before a postinstall refusal; Directory branch then returns
false so normal completion doesn't double-count. Add postinstall refusal->Continue
readonly root, and initial unreadable source root with no output/new Undo tracer.
These are proposed implementation details, not yet edits/proofs. Revalidate all
against current source after core48177 terminal before changing Rust.

After new epoch fix rerun appropriate focused/full core (necessary due production
change), all-target Clippy/build/fmt. Record schema18/compiled-source102 inputs and
actual tests count (currently322/21 helper definitions expected, not yet counted).
Potential readonly GC-stage cleanup tracer still not reproduced; source/request
lifetimes, full FD/media authority, explicit disposal, jobs, Trash/Restore batches,
Downloads integration, FileManager1, release, full workspace checks and review
stay active. Preserve historical logs and unrelated Shell research; no agents.

Next public cleanup tracer (not yet reproduced, do not claim failure/fix): normal
Copy of a555 source root into a new destination; while the substantial first
child is copying, independently create the later filename at output using
exclusive creation without chmod. Resolve its conflict with explicit Replace
(ordinary Copy, not Recovery, which correctly prohibits Replace). Verify retained
previous bytes and output555 at completion, then expire its history through100+
actual public operations. Inspect public Recovery entries and the private actual
output stage: regular eligible payload and empty container should both clean, or
a visible accurate retained cleanup issue must remain. Repeat with external
replacement/chmod/new stage contents before any authorization change. Current
read-only source review finds expire_retained removes the payload then its stage
via pathname; this alone proves neither failure nor permission authority. A fix
must preserve exact captured created-parent ownership through archive expiry and
restart rather than infer it from a private-looking stage name or chmod every
parent. Preserve existing changed/unavailable/shared-record protections.
