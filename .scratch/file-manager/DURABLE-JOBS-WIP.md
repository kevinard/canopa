# Durable unfinished requests — active implementation

Latest steering:the user rejects GTK's inconsistency with the other Canopa apps.
Root ADR0018 supersedes the GTK choice:use Iced and `canopa-ui`, preserve all
product requirements, and handle the Wayland file-exchange gap through the shared
stack/platform work. The existing GTK adapter is still implemented and its
native acceptance does not prove the replacement. GUI migration is now required
work. Core durability remains reusable and still part of the full objective.

Atomic metadata handoff is now implemented in the core request registry. It
updates the old stopped row and inserts its Queued successor in one transaction,
retaining any unselected plan, CAS revision protection, quotas and finished-row
retention. Source-owner transfer is still absent. First112-source focused
pipeline72666 TERMINAL0:23 tests across job_handoff (3), job_registry (11),
job_checkpoints (4), storage_requirements (5),48.27s. The actual handoff test
reopens between failed Retry and remaining Continue, preserves completed inode
and separate Undo. Budget refusal rolls back both changes. A later small test
edit records Running/Finished around the final Continue as well. Formatter44774
TERMINAL0. Settled112 inputs in job-handoff-settled-source.json; pipeline85966 is
TERMINAL0:handoff3 tests7.71s, core all-target Clippy47.53s, fmt --check10.23s.
Scoped proof:job-handoff-verification.json. No compiler or native session remains
live. There is no new GTK build request; the GUI direction is superseded. Current
binary2a3fa097… remains the older111-input GTK reference foundation.

Current checkpoint: natural-exit missing Retry reproduced in149 and151; original
process exits0 and new process has EMPTY Operations. Same-process control150
passes3 checks, both captures inspected.147/148 were fixture setup failures, kept
separate. Baseline native app4bd28350…; formal evidence job-lifetime-diagnosis.json.

Core version1 exact continuation codec implemented. Four new public tests pass
(raw Unix paths, mode700, preserved completed inode/bytes, separate Undo, foreign
source/destination container refusal and returned-original Retry, incompatible
version/relative path rejection). Existing KeepBoth test now decodes/reopens
before Resume and passes. Existing actual cross-filesystem saved-stage partial
Move decodes/reopens, preserves Recovery context and separate Undo, and passes.
Six focused tests verified; no native queue persistence or bug-fix claim yet.

Pipeline57781 is TERMINAL0: full core349/0/21 helpers (638.78s), Clippy63.27s,
default build80.65s, formatting7.08s. Frozen105 foundation verified in
job-checkpoint-core-proof.json and job-checkpoint-foundation-verification.json.
Binary14098e5106921eb6894be0b64849c6a5c47e4a77f919b4b1c1daaf6fdc29db53
belongs to that foundation, not the later payload edits.

New portable payload serialization is implemented and verified: core
Entry/OperationRequest/TransferRequest/TrashLocation+Item/RecoveryItem+Continuation,
raw optional/vector path adapters and pre-epoch metadata timestamp adapter.
Native Task derives Serde and skips process-local clipboard intent. No queue
save/load yet. Three new public payload tests and an extended directory Recovery
Continue test exercise actual raw-name mutations, Trash restore/Undo, saved-data
copy/Undo, old timestamps and decode/reopen. Pipeline34264 terminal0:3 payload
tests36.29s, Recovery Continue21.69s, default all-target Clippy72.95s.

Placement module moved from desktop/storage.rs to core storage.rs, exposing an
opaque byte-safe Requirements round-trip. Restoring keeps original device UUIDs
and resolved browsing bindings, captures current availability and drops old
process epochs. NoUUID requests refuse restart even when a new observation
reuses exactly the old counter; runtime Retry can still use the same live
observation. Repeated Retry/save never adopts replacement media. Five focused
public tests pass47.62s, including independent source/destination captures.

Core schema20 request registry implemented with short independent SQLite writer,
bounded opaque JSON payloads, monotonic IDs, CAS revisions, six distinct phases,
read-only listing,100 finished rows independently of unfinished records,
1024 unfinished/4MiB payload/64MiB total bounds. Reject stale/cross-history updates,
Running dismissal and Finished resurrection. Does NOT yet lease source data or
bind per-step journal commits. Native save/load remains unimplemented.

Initial111 application/build inputs in job-registry-compiled-source.json.
Formatter33148 terminal0. Pipeline92335 TERMINAL0 (77.54s):focused job_registry,
job_checkpoints (now actual durable record -> reopen -> exact Resume -> separate
Undo), storage_requirements and history_upgrade. Tests include abrupt process
exit without destructors, real transfer question acceptance, corrupt payload
preservation, budgets and stale revisions. Parent summaries5 history-upgrade,
4 checkpoints,10 registry and5 storage all pass. Then added a public64MiB
aggregate-budget rollback/recovery test and tightened the writer's post-open
history-location check. Formatter35766 terminal0. Final111 frozen inputs in
job-registry-final-compiled-source.json. Pipeline62091 is TERMINAL:core368/0/21
ignored helper definitions across39 targets (818.01s), default all-target
Clippy110.63s pass; desktop build failed101 with LLVM ENOSPC. Its status update
left job-registry-final-state.json empty. The exact handle was missing and host
process inspection confirmed no compiler. Results are preserved in
job-registry-final-recovered-state.json. Removed39 completed Files test binaries
(841818304 bytes) and only the failed native compiler's working generation;
source, application binaries, dependencies, completed caches and evidence remain.
Exact removal is recorded in job-registry-cache-cleanup-executed.json.

Unchanged111 rebuild92901 TERMINAL0:desktop build65.63s, fmt --check5.70s. Core
and Clippy evidence still applies to identical sources. New binary2a3fa0972ad07835f275f1e740ab17218fce9d88ccd87cf0c54ea5d49a389606.
Scoped proof:job-registry-foundation-verification.json. No compiler/native session
remains live at this checkpoint. No native queue integration/source leases/step
checkpoints or missing-Retry fix is claimed. Next implementation:atomic scheduling
handoff for Retry/Continue before source ownership and native wiring.

Next:finish the settled core checks and begin the Iced migration under ADR0018.
Keep durable queue integration on that shared presentation, using exact opaque
plans and original source/destination requirements. Full contract is draft
ADR0056. Persisted cut
intent counters MUST NOT collide with a new process's sequence: discard old
clipboard intent on restore or give it explicit process scope. No-UUID storage
observation epochs also cannot be compared across processes. Persist required
original media/bindings, restore availability conservatively, never adopt new
media. Running/queued interruption and actual committed prefixes require their
own durable phase checkpoints; do not blindly rerun an original batch. Ordinary
saved-file leases and cleanup owners must stay protected while requests depend
on them. Core codec alone does not satisfy these gates.

Retry/Continue handoff needs one transaction that creates the successor Queued
record and removes only the selected plan from its old row. Separate insert and
update calls leave a crash interval with two executable copies of the same plan.
Add the atomic interface with source-owner transfer before native Retry wiring.
Per-step running checkpoints need an associated filesystem journal step and an
exact post-step continuation, including created-directory identities, chosen
targets and completed prefix. A saved original Running payload only gives visible
interruption, not safe replay. Existing Pending journal recovery is the seam;
do not infer committed progress from current path existence or displayed counts.

Next source-lease feedback loop:select an actual retained `previous` file as an
ordinary queued Copy source through the public Recovery entry, admit its durable
request, perform more than100 later mutations, reopen and execute the selected
request. Today request metadata does not protect it from history expiry; first
reproduce that loss before adding ownership. Extend the successful ownership
seam to explicit saved-container movement, Undo consumption, pending-directory
payloads and release/dismissal. Registration must remain possible during a live
question and must serialize against the final expiry owner-check/unlink interval.
Do not archive pending `Record.payload` as ordinary previous data:that field is
currently skipped by Serde, so a naive archived-record lease changes its meaning.

Full goal ACTIVE; this continuation PROGRESS, not a completion or blocker. The
following entries preserve chronology; their live-session statements are historical.

## Chronology

Previous goal turn: progress. Clipboard placement is accepted in ADR0055 and
clipboard-placement-verification.json (final76 native checks/41 inspected captures,
103 frozen application/build inputs, Clippy/build/format; identical core inputs
reuse345/21 evidence). The full PRD stays active. No compiler or native session
was live at this turn's start; host Niri1238 is preserved.

This phase addresses the review's natural-exit loss of unfinished requests.
Plans and queued tasks currently live in desktop/jobs.rs memory. A completed
batch may retain Retry/Continue while releasing its application hold. The
original PRD requires visible remaining work, exact failed-item Retry and shared
Operations after all browsers close. Durable jobs must retain exact continuations
and original source/destination placement, rather than reconstructing requests
from current paths or repeating completed work. No production fix yet.

New native feedback loop: external URI clipboard of a real directory containing
a17.5 MB regular file and one unsupported FIFO; Paste into a private destination.
Wait for actual staged bytes, close every Files window, prove active lifetime,
observe Completed with errors and natural exit0. Repair only the failed leaf,
reopen Files/Operations in a new PID and use Alt+R. Required outcome: exact failed
leaf copied, prior completed inode/bytes unchanged. No SQL or hidden UI state.

files-job-lifetime.py is dispatched by --transfers-only --transfer-case lifetime.
Native inventory has dark/light1.25 cases. Existing scenario flags now form an
argparse mutually exclusive group, preventing silent contradictory workflows;
the broader optional positional-options smell is not claimed resolved.

Static55518 terminal0: fixture/runner AST, conflicting scenario rejection before
artifact creation, dependent-case rejection, maintained inventory and whitespace.
Producer6 hashes frozen in job-lifetime-repro-source.json. Native147 red now live;
poll its authoritative handle before any compiler/native or Rust edits. Final
app binary remains4bd2835013ee5b069becb0671f31cc192426da9920a8086ff806f996c488e71b.
This is a proposed failing behavioral loop, not a reproduced failure until its
terminal evidence is inspected. Full goal active; no blocker or completion claim.


Native147/35787 terminal1 at expected-exit observation, before semantic reopen
assertion. It confirms all windows absent, natural Files exit0 and notification
Completed with errors:2 completed items/17476000 bytes. Harness wait_for checks
managed process liveness before its predicate, so it rejected the expected exit.
Removed that expected process from the liveness set around this boundary only,
restoring cleanup ownership if it stays live/fails. Native148 will reach the
actual reopened Retry assertion. Updated6 producer hashes recorded separately;
no application fix or genuine semantic red claimed yet. Rust candidate unchanged.


Native148/72667 terminal1 in clipboard readiness before any transfer:wl-paste
ran before the asynchronous wl-copy owner published and returned1. Matching the
existing source-isolation fixture, readiness now treats that transient no-data
result as false and continues the same bounded wait. This is fixture setup,
not a semantic failure or evidence for a production fix. Third6 producer hashes
saved separately. Native149 reruns the sharpened loop; app source unchanged.


Native149/9819 terminal1 at the actual reopened Retry assertion. OriginalPID875963
exits0; newPID875996 shows EMPTY Operations (capture inspected), despite one
Completed with errors notification and intact completed bytes SHA2fe7cf73… .
Alt+R never creates the repaired child. This is the first semantic red, unlike
147/148 fixture failures. Ranked3 hypotheses shown before probes:lost in-memory
plan (same-process should work); repaired-source plan itself invalid (both should
fail); Operations/window/keyboard focus fault (same-process should also fail).
Added lifetime-live control:keep one unrelated Browser, close the transfer origin,
open a new destination window in the same PID and invoke the same Operations
Retry. Exact source repair, bytes/inode and focus observations are retained. Probe6
hashes frozen separately. Next150 control,151 repeated full restart, sequentially
on unchanged app binary4bd28350… . No production edit or narrowed goal claim.


Native150/79326 control terminal0:3 checks/2 captures inspected. Kept unrelated
Browser retains PID876223; closing origin and opening new destination/Operations
still exposes Retry. Repaired FIFO copies successfully; completed prefix keeps
same inode/hash. Focus explicitly true. This falsifies repaired-source and
same-process window/shortcut explanations; Queue memory lifetime is supported.
Native151 repeats full natural-exit/restart on current probe producer, unchanged
binary, to confirm semantic red independently. No app code changed yet.


Native151/90350 terminal1 repeats the exact restarted Operations missing-Retry
failure after clean exit0; all native sessions now terminal. Supported cause:
retry/continuation tasks exist only in the original Queue memory. Native150 same
source repair/new-window/control succeeds. No production probe logs/env gates.
Draft ADR0056 records the full durable queued/running/failed/remaining contract,
original media/alias requirements, no automatic command replay, crash-phase
ownership, raw bytes and write-failure preservation. Scope is not reduced to a
process hold or path-only retry.

Added first public core checkpoint contract test:failed directory Copy with raw
Unix root/leaf names, mode700 and completed sibling; store opaque continuation,
close/reopen engine, repair leaf, decode/resume exact plan, preserve completed
inode/bytes/mode, separate Undo of retry and completed prefix. Serde traits are
not implemented yet; expected compile-red precedes codec foundation. Formatter
running; no native/compiler overlap. Queue persistence remains unimplemented.


Core checkpoint red4283 terminal101:Serialize/Deserialize absent on public opaque
TransferContinuation (5.76s). Added private version1 checkpoint representation:
arrays for pending entries, directory frames and saved receipts preserve every
raw path through existing path_serde; no JSON object path keys. Transfers expose
Serde without exposing private owner fields. Decode rejects unsupported versions,
nonabsolute locations/invalid entry targets, duplicate directory/saved keys and
invalid folder modes. Exact order, DirectoryFinish identities/mode/cleanup owner,
and Receipt(id,ordinal,physical identity) remain intact. No queue persistence yet.
Formatter56910 terminal0; first-green core check now running. Application default
binary is still prior4bd28350… until a later build. No native/compiler overlap.


First-green42656 terminal0:public raw-name/mode/separate-Undo checkpoint test
passes (31.53s incl30.95s rebuild; test0.25s). Added public replacements of source
and destination containers after decoding, refusal with foreign data preserved,
then return original and Retry. Added version/relative-location rejection.
Existing KeepBoth continuation test now serializes, drops/reopens engine and
resumes decoded plan, verifying exact chosen folder mapping without new conflict.
No job persistence claim. Formatting now runs before these focused checks.


Guard suite51499 terminal0:4 public checkpoint tests pass11.94s including build;
KeepBoth serialized/reopened test passes26.67s inclbuild. Source and destination
foreign-container refusals preserve independent bytes; returning originals allows
Retry. Version2/relative targets are rejected. Added serialized checkpoint to the
existing actual cross-filesystem saved-stage partial Move/Resume/Undo test so
Receipt ownership and Recovery context must survive decoding/reopen. Formatter
now runs; still no queue persistence or user-visible bug-fix claim. Native149/151
remain genuine missing-Retry reds;150 same-process control green.


Additional next-implementation constraints:runtime cut intent numbers are process
scoped; restoring an old persisted token must not clear a new process's newer
Copy/Cut with a reused number. Serialized Task transfer tokens should restore
without clipboard authority. Storage required roots need an explicit process
scope/conservative restoration for no-UUID epochs. Core job ownership and retained
source leases must account for history expiry and saved-stage relocation, rather
than treating plan serialization as a data lease. Queue hydration must reuse
normal rows/focus/layout and never auto-run uncertain commands or whole batches.


Prior turn classification:progress (codec+tests+diagnosis). Pipeline57781 now
TERMINAL0:core349 passed/0 failed/21 invoked helper definitions across36 targets,
638.78s; Clippy63.27s; desktop build80.65s; fmt7.08s. Frozen105/binary14098e5106921eb6894be0b64849c6a5c47e4a77f919b4b1c1daaf6fdc29db53.
Proof:job-checkpoint-core-proof.json and job-checkpoint-foundation-verification.json.
This proves core codec foundation, NOT queue persistence. No native/compilers live
before new edits. Formal native reds149/151 remain the user-visible baseline.

Started portable queue payload support:byte-safe path optional/sequence adapters,
metadata timestamp codec preserving pre-1970 values (standard serde SystemTime
rejects those; exact installed source verified), Serde for Entry/OperationRequest/
TransferRequest/TrashLocation+Item/RecoveryItem+Continuation. Native Task can now
serialize its portable fields; transfer clipboard intent is skipped so restored
requests cannot control a new process's clipboard. No save/load integration yet.
New public payload tests exercise decoded create/rename/copy with raw paths,
actual volume Trash restore/Undo and Recovery copy/Undo with raw names and old
file timestamps. Existing directory Recovery Continue now decodes after engine
reopen. Formatter now running; wait its handle before checks. Source105 manifest
is historical foundation after these new edits; default binary14098… remains that
candidate until next build. Full goal active/progress.
