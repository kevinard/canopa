# Filesystem attention checkpoint

Goal turn: progress. ADR0083 is scoped-verified and preserved; the entire PRD
remains active. All compiler/native/archive/cache handles TERMINAL. Both18-path
code reviews11 and both3-doc reviews12 report0/all pinned hashes match.

Final source12:572 inputs. Archive SHA82a01872e1170e3bd545db0a1b61ef4df6ea462bcb26c7fcb68e2c27d7f1d315.
Application 88a116cdffee04832092c4dca7bf566e846ac1a032fc637a83922242476c43e1/119965208B;
verified gzip canopa-files-88a116cd.gz/28866968B,
SHA820e053339a27665389daac3856ed96006e48249b02f856f07280de3a2c4fcbb. Preservation90156 exit0 verifies all archive
members, decoded binary, current source hashes and application inode content.
Source12 changes3 docs only from11. Application built10; production unchanged11/12.
Headless09 production/fixtures unchanged10/11/12 (only test additions/cleanup).
See operation-attention-verification.json and operation-attention-preservation-12.json.

OperationJobs now publishes semantic attention under the snapshot lock. A unique
observer claims notices before exposure; latest unread state perjob survives
100-row trimming. At most1024 outstanding operation identities and17 handed opaque
receipts; hold stays until explicit acknowledgement. Resolved/retried unread
attention is retired, but handed generations keep independent receipts. No old
history announcements or replay. OperationAttention owns standard Notify replies,
bounded100 perjob errors+1 outside, and frontend wakeups beforeIced inLocalOperations.
Iced now retains refresh/drafts/typed question presentation, with no notification
Task/generation-dedup/delivery hold. Exit reads live backend holds.

Evidence: newinterface compilationred51945/101; initialgreen91117/0/1. Stronger
handoff red96547/101 reproduces early hold release; green17821/0/1 retains receipt
untilack. Capacity first50950/101 had2 pass then generic10s wait timeout after
1024 queued jobs; fixture pacing each completion retains1024 unread receipts,
no performance/product fix claim. Core8978/0 passes19 parents and its crash-child
check; savefailure41656/0 adds1, with actual history-path loss, onlyunsaved holds,
restored RetrySave without replay. Host newinterface red67006/101; green74079/0/1.
Broader85336/0 passes61 parents plus normal/killed private helper checks. Total81
backend parents. FinalClippy94174/0 only3 existing warninggroups; default build/fmt
pass. Its wrong --lib iced_desktop filter ran0/47, explicitly excluded. Correct
unfiltered --lib41656/0 passes47; changedtest lint/fmt pass.

Private-bus84605/0 passes8 checks: action20-command burst/17 actual deliveries/3
busy refusals; filesystem3 jobs+decision produce4 actual standard notifications.
Held Notify reply protects even directcore Shutdown; exact answered Undo preserves
later file bytes then resumes queued creation. Both build JSONs enforce onlytheir
background feature and noIced artifacts, noDISPLAY/Wayland/compositor initialized.
GUI20946/0 passes39: operationsdark12/light12, actionslight15. Both operation logs
empty; actionlog only4 expected missingGIOremote-monitor messages. Representative
darksourceclosure/lightquestion/runningaction captures visually inspected.

Cachecleanup5128 removed4 completed regenerable library/core test ELFs145784544B
under Cargo artifact flock+lockf, preserved appab3caff0 at that time and all libs,
native helpers, sources/evidence. Do not blindly delete canopa_files-*; application
aliases share that prefix. Every source/fixture change was frozen while validation
ran. Future mutations may proceed now that all exact handles are terminal.

Next actual work: choose resident process layout/local primary activation in an
integration ADR, then implement host/CLI/session activation into the SAME runtime.
Remaining native clipboard/platform coordination is still frontend-owned; account
for it rather than creating another history writer or reconstructing opaque source
plans from snapshots. LocalOperations/local_volumes are currently desktop-feature
modules even though OperationAttention itself builds withoutIced. A display-free
core workflow test is not a public daemon. No --daemon flag exists. Measure idle/
warm performance; preserve original source/media/journal/restart and safe Retry/
Continue gates as independent unfinished work. Startup timeout remains open.
Full PRD/workspace/release/packaging and other migration gates remain active.
See NEXT-EXECUTION-NOTES.md. Old operation-startup finalizer remains invalid.
