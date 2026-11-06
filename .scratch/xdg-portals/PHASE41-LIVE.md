# Historical Phase41 live notes — superseded by CONTINUE.md and acceptance-01.json

# Phase41 live continuation

Previous goal turn Phase40 was PROGRESS, sealed. This turn implements full Print
recovery review/export/startup; do not stop at a read-only list. Full goal stays
11 packaged,6 experimental, Print unregistered; ten Files failures unchanged.

CURRENT LIVE: full validator /tmp/canopa-print-41/validate.py, exec session13473,
worker-05.json. Last confirmed live at build after fmt3s/strict workspace Clippy79.03s
passed. Do NOT edit Rust/manifest/vendor until this handle is terminal. Poll the
same handle; timeout is not terminal. No native fixtures are running yet.

Source changes not yet runtime-accepted:
- Recovered separate from Frozen; snapshot selected source via worker, seal and
  SHA-256 verify. Raw/empty displaced bytes use separate PublishRecoveredFile and
  commit_recovery; normal Print remains nonempty/format checked. Reserved2 slots
  enabled for recovery only, normal cap14/max16. Retention records known moved hint.
- Source RecoveryProtection directory identity only FORBIDS destinations; never
  grants write authority. Client/Prepare/Publish reject selected source folder;
  catalog begin also rejects all known retained work folder parent identities.
- inspect allows valid displaced source even if document missing/corrupt. Dismiss
  removes index only after inactive owner/worker proof. All source I/O in workers.
- print_recovery_ui.rs + view.rs + tests.rs: Iced list/status, snapshot before
  embedded Files Picker retaining Reviewed overwrite authority, save/cancel,
  error/notice, OpenFolder/FileManager1 bounded5s, explicit dismissal confirmation.
  Generation/lifetime guards; cannot Back out of active publication and hide result.
  Late/stale messages cannot revive/claim success. Missing metadata not called empty.
- --recover-print public CLI; --print-recovery-store override (also normal backend),
  --print-recovery-dialog private helper. RecoveryStore::discover uses XDG state.
- print/recovery_host.rs: normal main starts background startup discovery AFTER
  D-Bus ready; nonempty => owned Iced helper via same binary/shared config. Shutdown
  bounded15s with continued ownership/reap; wait errors are not death. No Print
  registration. Initial catalog-list errors currently log/skip auto UI; manual CLI
  displays load error. No installed/user-state changes have been made in tests.
- Native recovery fixture augmented for document + opaque/empty displaced export,
  protected-folder rejection, two concurrent publications and index-only dismissal.
  Set CANOPA_PRINT_RECOVERY_EMPTY=1 for empty previous body. Existing producer fixture
  remains real SIGSTOP at Publishing + parent SIGKILL/wait. New regular tests added.

NEXT after worker05 terminal: fix any actual errors, otherwise run actual native
recovery.py both standard/EMPTY; full Portal regressions, existing filesystem/
retirement/renderer/public regressions as appropriate. Validate actual Iced via
/tmp/canopa-print-41/ui.py in host access with tty=true: starts private Weston/Niri
and a stopped producer, then manual Recovery CLI. It prints ready JSON and accepts
JSON line commands op state/capture/click/scroll/keys/kill-producer/launch/check/accept.
Driver is not run yet. Use initial screenshot (functions.view_image) to get actual
button coordinates. Commands logged under target/portal-print-41/recovery-ui-... .
Initial producer stays STOPPED for Busy/disabled checks; kill-producer confirms death,
then Refresh. Home is private runtime/home. Save buttons should yield files
Recovered-interrupted.pdf / Displaced-interrupted.pdf there. Picker default name
prefilled, bottom-right Save copy reuses existing embedded picker. Test cancel,
document/opaque copy, SHA mismatch (tamper preserves mtime, then restore), explicit
remove confirmation/Keep/dismiss, retained bodies untouched. Close manual UI via
Escape on List; launch startup starts actual normal backend with private policy/store;
check backend-ready + actual automatic Iced window. Test dark100/light125. Root window
caption stays 'Recover printed files — Canopa' even in Picker; screenshot/poll needed.
Accept requires named check receipts, then driver tears down owned fixtures; always
observe terminal handle, and fix any Python-driver errors without Rust edits while
validation/native guards live. If UI source fix needed, end driver/check handles first.

Scripts /tmp/canopa-print-41/{recovery,filesystem,retirement,renderer,regressions,public}.py
were cloned from Phase40, stage paths updated, not run yet. They select latest worker
receipt; require full passed worker with portal_test_artifact, not lint-only worker.
New native verifiers memoize binary hashes by dev/inode/size/mtime/ctime. Source guard
and binary pins must be checked after native tests. No extra Rust driver/binary copies.
One NEW tiny C wheel fixture compiled from existing repo scroll.c + Wayland protocol
into stable /tmp/canopa-print-41/tools/scroll (18,008 bytes), SHA
ce47201113dc0c77978d4d47d063b40ccbd080a8a50059636ee0610bf2e31fd0;
cc needs -lwayland-client -lm. Reuse it. click reused from
 target/portal-print-dialog-native-03/file-dark/click. Niri pinned92294c... store.

Validation history all terminal before worker05: worker01/session49888 failed types
DismissedRecovery String reference + nonexistent Action::Primary; worker02/session50944
failed moved Operation match. Fixed matches!(&operation,..) and shared Action::Normal
with focused flag. worker03/session50033 lint-only PASS158.05s all targets; no build.
worker04/session83982 Clippy group scoped-stopped at39.02s to apply a failed textual
edit before compilation; terminal-15, no build. Full worker05 is the only live handle.

Storage baseline7,328,976,896bytes free; 6GB reserve/3GB net cap. Same root target,
workspace/default/all-target/offline/incremental/jobs1. Do not switch graph, create
fresh target, clear caches, copy binaries, or ask user for more disk unnecessarily.
Host access require_escalated is needed for private D-Bus/socket native fixtures and
validator tests. Direct harness CWD crates/canopa-portal. Preserve previous phase40
acceptance as historical; new binary/source pins will supersede. No Phase41 acceptance
or ADR yet. Update roadmap/PRINT/CONTINUE after actual verification, full goal active.

## Live update after build/native checks

worker05/session13473 TERMINAL0: fmt3s/Clippy79.03/build721.28,131 Print/11ignored
and12 Lockdown PASS. Full Portal/session42429 TERMINAL0:349 library/19ignored,
55 integration/2ignored. Native CLI recovery-01/session93279 and recovery-02 EMPTY/
session97319 both TERMINAL0,18 observed each, actual verified document/opaque-or-empty
export/protection/dismissal assertions pass. FS13644 TERMINAL0,6groups18 observed;
retirement62899 TERMINAL0,3groups11 observed. Renderer54445 unexpectedly terminal143
at svg-to-pdf despite successful child; host scoped scan found no surviving native-01
renderer fixtures before rerun. native-01 summary marked interrupted. Rerun89153
TERMINAL0 native-02 PASS24groups149 observed. No Rust edits after worker05 started.
Public3759 and40499 TERMINAL0 PASS9 files-dark27 observed and9 printer-light59 observed.

Iced dark session25848 TERMINAL0 PASS recovery-ui-dark-01,10 observed terminal.
Actual Busy disabled copy, confirmed producer SIGKILL + refresh, picker cancel,
document36bytes and opaque14bytes export to private HOME, source bodies intact,
manual close, actual normal backend ready THEN automatic recovery popup, Keep vs
explicit index-only dismissal, source intact/catalogempty. Screenshots/commands in
receipt. verified-destinations.json preserves hashes before temporary HOME cleanup.
Native hash-probe.json uses actual normal worker to reject tampered document with
same size+mtime (failed completion check, exit0/EventFailed), restores source.
Earlier UI hash attempt conservatively failed with generic worker-stopped and its
screenshot caught Preparing before error; do not claim that screenshot proves
SHA rejection. Direct worker probe + regular test prove exact checksum failure.

ONLY CURRENT LIVE native/check: UI light session31045, recovery-ui-light-01, started
with CANOPA_PRINT_RECOVERY_EMPTY=1. Private HOME /tmp/canopa-print-recovery-1h193qwu/home.
Window logical860x640 at20,10.4, source controls same logical coords: doc470,312;
raw480,354; picker Save814,620; Remove575,547; confirm Keep93,191/Remove222,191.
Both doc36 and empty displaced0 copies passed, retained source+entry still present.
Currently checking overwrite consent: after copies, external owned fixture wrote
21-byte 'reviewed target bytes' to Recovered-interrupted.pdf and created hardlink
reviewed-target-link.dat. overwrite-before.json records old inode/home. Clicked doc
then picker Save; capture overwrite-consent.png queued in session31045. Read next
output/view screenshot to locate actual Replace button (likely200,580). Confirm and
verify target restored36byte expected PDF, old hardlink still21 bytes and same inode.
Then persist verified-destinations.json before driver cleanup. Close manual UI via
Escape/List, launch startup, check backend-ready, then explicit dismissal and
retained-intact; accept with required check names as dark. Driver has20min deadline
from startup; proceed promptly. Functions.write_stdin can batch JSON lines, ttytrue.
Don't edit Rust while live. No external user blocker/approval pending.

Acceptance/ADR0082 not sealed yet; ADR file drafted with status under validation.
Need final seal (adapt phase40 script for2 CLI recovery receipts+2 UI receipts,
new fields/no wrong phase40 concurrency-only scope), authoritative host no-scope
process audit including interrupted native01 recorded PIDs, full source/binary
pins, storage6GB reserve/3GB phasecap (baseline7,328,976,896). Currentfree~6.97GB.
No new Rust driver/binary copies. Tiny C scroll18,008 bytes was compiled but not
used yet; can archive it losslessly by checksum after UI closes, keep source receipt.
Need docs ROADMAP/PRINT/CONTEXT-MAP/CONTINUE updated to actual verified phase41.
Full goal remains active11pkg6experimentalPrintunregistered, broader gates unchanged.
