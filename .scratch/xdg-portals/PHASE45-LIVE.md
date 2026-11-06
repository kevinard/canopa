# Phase45 live checkpoint

TERMINAL FINAL: dark50951/dark06 and light65405/light04 eachTERMINAL0,5topgroups/
24observedterminal. Prepared54224/dark01 TERMINAL0,15groups/85terminal. Validation
66794TERMINAL0,seal13941TERMINAL0. Total133 finalobservedprocessesterminal,no live
Phase45 driver/check. Authoritative CONTINUE.md/currentphase45, acceptance-01.json,
print/cups-acknowledgement-evidence.json and ADR0086. HTTPackfix passes; actual
main-death CUPSjob cleanup stillFAILS (preserved dark04/light02). Fullgoalactive.
All live-handle snapshots below are historical and must not trigger restarts.

CURRENT LIVE FINAL: dark50951/public-printer-dark06 and light65405/light04,
acknowledgement-only native matrix, current validated45worker01 binary.5 top groups
each:4 public (control/lostCreate/lostSend/fresh-after-errors), retained repository
worker regression3subcases. Poll SAME handles, no fixture/Rust edits untilterminal.

VALIDATION66794 TERMINAL0 and positive prepared54224 TERMINAL0: stage45/
prepared-printer-dark01,15groups/85 observed terminal, all PDF/PS/SVG/copy/duplex
jobs9/copies1/content/geometry pass on current binary. No emergencykills.

CRASH GAP established, NOT fixed or accepted: dark04/88575 and light02/78431
TERMINAL1,4 groups then failed actual mid-upload crash cleanup assertion,26/25
observed allterminal/no emergencykills. Parentdeath code in process.rs intentionally
sets KILL, so worker cannot run RAII cancellation after main SIGKILL. Actual
acknowledged CreateJob5 before complete Send HTTP terminator; workerterminal,
mainrestarted, but job5 remainsheld4/job-incoming with no CancelJob/no receiver.
Retained spool/c00005 parsed confirms state4; crash-gap.json each records it.
Unknown Create reply similarly leavesheld4. Do not disable parent-death KILL or
claim global/crash recovery solved. Next goal turn needs durable printer-job
ownership/cleanup policy compatible with existing retirement/resource guarantees.

Earlier post-fix dark03/22340 and light01/33971 TERMINAL1,2groups each/19+20terminal:
fixture incorrectly required physical completion9 before releasing lost reply,
but newcode times out/cancelsjob7 before printeroutput. Correct outcome permits
owned cancellation or at most1exact authorized delivery, while response2/no retry.
dark05/51364 and light03/5692 TERMINAL1,5groups/24+25terminal:all4publiccases pass;
new repeatable fixture wrongly required nonzero helperexit for Failed frame.
Protocol delivery normally exits0, error is inframe; corrected script assertion.
Its earlier exact source snapshot preserved beside both summaries/checksumsmatch.
No production Rust edits since validation. These failed overall receipts remain
failed; current final handles above use corrected fixture. A legacy fresh-after-
restarts label in05/03 was actually fresh-after-errors in ack-only mode; final
source names it accurately and claims no crash/restart acceptance.

All snapshots below predate this current entry.

LATEST: validation66794/worker01 TERMINAL0:fmt3.01s,Clippy116.04s,original-graph
build366.16s,135 Print/12 Lockdown,353 library/55 integration. Source guards match;
phase net growth at validation negative15,175,680bytes. No Rust edits since launch.
Additional stall C shim syntax check passes; new Python regression help/syntax pass.

LIVE native dark22340 (public-printer-dark03) and light (see latest tool handle;
public-printer-light01), exact stage45/worker01 binary.8 top-level groups each:
7 public cases (forward control,lostCreate,lostSend,main death at withheldCreate,
main death BEFORE finalSend chunk,main death at withheld finalSend reply,fresh
postrestart Print), plus retained repository worker regression with3 actual worker
cases and owned-job cleanup. Proxy pins include both new Python scripts; no edits
while live. Poll same handles until terminal. Current relay extends request-end
gating, so red02's exact earlier relay source is preserved beside its summary with
matching checksum, reconstructed losslessly after a snapshot/read race. No data
or binary copies were lost. Red02 remains the strong reproduced failure evidence.

All snapshots below predate this latest entry.

CURRENT: validation66794 LIVE, stage45/worker01, exact same workspace graph/cache.
Native59058/dark01 TERMINAL1,2 groups/21 observed allterminal, no emergencykills.
Native87905/dark02 TERMINAL1 mismatch,4groups/23terminal, no emergencykills.
Both controls pass, lostCreate response2/oneCreate/noSend/jobheld4. REAL BUG:
lost final Send reply (actual scheduler200/IPPsuccess withheld) yields public0
and independently actualworker Created4+Accepted4/exit0. Final native02 stores
all_relay with forwardedfalse and fault names; control forwardedtrue. Actual
receiver/job9 completes once; no retry. PriorAPI success value survives transport
failure in cupsFinishDestDocument. This is not a fixture geometry mismatch.

Rust fix made only AFTER both native handles terminal: cups.rs dynamically loads
public httpGetStatus and requires HTTP200 AND IPPsuccess before Accepted/commit.
No parser/dependency/API or normal routing changes. Associated existing C stall
shim adds forwarding for the newly used public status function. Validation uses
guarded current sources; DO NOT edit Rust/manifest/vendor until terminal.
Next adapt final native driver to current validation, add owned full-main death
at withheld Create/Send acknowledgement and restart checks after the initial
lost-reply regression passes. No new binaries or build graphs.

All notes below predate this current entry.

Previous goal turn44 was PROGRESS: prepared printer fix sealed plus current docs.
Phase44 source and normal binary guards revalidated this turn; guard72645 terminal0.
Goal ACTIVE,18 interfaces/11 packaged/6 other experimental/Print opt-in. No user
blocker or installed changes. Baseline7218839552 free,6GB reserve/3GB growth cap;
no new build, binary copy or native executable. Same current44 binary/test harness.

LIVE native59058, /tmp/canopa-print-45/printer.py, stage45/public-printer-dark01.
Poll SAME handle until terminal. No guarded Rust/manifest/vendor/fixture edits
while live. Fixture pins scripts/desktop-fixture/print-cups-proxy.py; preserve it.
Initial launch exited1 before any child because parent stage directory absent;
fixed mkdir, not a product failure or running process.

New Python Unix relay forwards actual HTTP/IPP between only owned worker and
owned CUPS; arms one dropped scheduler reply AFTER actual accepted operation.
Public frontend/normal --experimental-print/Iced approval unchanged. Initial
cases: positive PDF control, lostCreate reply, lostSendDocument reply withheld
until actual receiver/job9 then discarded. Check response2/no automatic retry,
exactly1Create, owned CancelJob only, unchanged unrelated completed control,
exact PDF bytes/one delivery. Creation job whose ID was withheld is observed
before explicit fixture cleanup; don't claim product can cancel unknown IDs.
No service-death/restart case yet; no source fix chosen before native evidence.

Proxy source primary API semantics checked against OpenPrinting cups v2.4.20
dest-job.c/request.c. Existing code only emits Created after acknowledged positive
ID, final Accepted after Finish acknowledgement, cancels known partial jobs via
worker RAII and parent cleanup, no application resubmission. Unknown Create and
whole-main death policy still need measured evidence. Full goal not complete.
