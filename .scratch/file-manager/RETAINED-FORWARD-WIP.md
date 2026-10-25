# Retained payload mutation ordering

Full PRD and implementation goal remain active. Previous goal turns made concrete
progress and verified waits on the exact live compiler/native handles. This
checkpoint is verified scoped progress, not full file-manager completion.

## Immediate state

All known compiler/native/formatter/read handles terminal: full core39112 exit0
(442 tests), Iced library34611 exit0 (51 tests), Clippy33728 exit0, build23395 exit0,
fmt58415 exit0, native44018 exit0 (10 checks in2 cases), integrity54060 exit0
and final source28 integrity96291 exit0.
Source/binary archive69830 and source27 archive96585 also terminal0.
Final documentation snapshot source28 has534 inputs, with only3 MD changes from
build/native source27. Archive93063 terminal0; all534 members/final inputs
verified. No compiler/native/archive/formatter/read handles live. Both code-review
axes on source27 have0 unresolved scoped findings; both source28 documentation
reviews also report0 findings. No new compiler/native invocation is needed for this MD-only delta.

## Verified candidate

- Default Iced/canopa-ui executable: c2b9e792,
  sha c2b9e792a2d6e97ce80589c2ae095652e9c298e38af4d232b2597db4c726aa39,
  118802464B, no GTK widget links.
- Preserved canopa-files-c2b9e792.gz,28688061B,
  sha dc8648f297147243e10331f9f48924686582c665a62ae1700b3179b2f45dff65;
  decoded hash verified, binary+archive unchanged after native.
- Source27 tar.gz,1481641B,
  sha b9cf494252c81fa79ec325e0272434afd316812b60d1f3b65210f84fb2eb7377;
  all534 members/current inputs verified before documentation delta.
- Native output retained-forward-native-green-01. Dark100:5 checks,212.3983s;
  light125:5 checks,239.3143s. Both files.log empty, both actionable Rename
  refusal notifications verified. Dark expiry/light release captures inspected.
  Actual Copy survives100 real folder operations, pastes old bytes, then releases
  expired data after foreign clipboard supersession. Baseline2108 on bf3 genuinely
  consumed the clipboard-owned retained file through Rename.
- Earlier full core53410 source23 passed440; final39112 source27 passed442.
  Source27 focused71804 passed27 parents+1 ignored child helper.

## Implementation and decisions

ADR0073 shared journal predicate guards consuming forward/reverse paths and
source/destination parents of saved-stage migration. Canonicalize affected parents
before walking physical ancestors, preserving final-link semantics and failing
closed on unreadable parents. Include RemovedDirectory source and moved-current
Undo endpoint. Whole-container Rename records shared Movement::capture parent
facts and validates saved namespace before begin/mutation. Real lookalike folders
remain ordinary data. Acquisition and journal admission use the same transaction
ordering. Retention grants no execution bypass or saved-plan replay authority.

All genuine reds, corrected invalid oracles, exact handles/manifests and final
runtime evidence are in retained-forward-verification.json. Source27 scoped
review fixed deep aliases, empty directory forward removal, enclosing-container
migration and empty directory Undo. Standards duplication was removed using
shared Movement::capture. No remaining scoped finding. Cleanup02 removed46
completed regenerable core integration executables under the Cargo lock, freeing
1065108280B; all those core tests had passed before removal.

## Next actions

This protection checkpoint is closed and preserved, with final source28 archive,
integrity and both documentation reviews verified. Keep the whole PRD open.
Proceed with queued source identity and core request/journal execution ownership,
using the concrete
public reproduction in NEXT-EXECUTION-NOTES.md. The codebase-design skill was read
and applied to planning the core module interface; no implementation agents or
unapproved test seams. Existing implement/tdd/code-review skills remain active.

Safe Retry/Continue, source/request/journal/descriptor/original-media/restart
execution authority; complete typed questions, DnD, activation, Iced Trash/Recovery/
devices/media/Downloads; accessibility/performance/shared-stack/workspace/release
and final review/commit all remain active. No full requirement checkbox complete.

No source edits with compiler/native/archive handles live; no compiler/native
overlap. Cargo offline -j1. Real private filesystem fixtures only; native only
approved isolated harness with escalation+tty and unique output directory.
Workspace root commands; .git read-only, preserve all existing unrelated dirty
work including shell research. Disk space uses safe regenerable Cargo cleanup,
not deletion of proofs or user data. Root/Files contexts and instructions apply.

Final source28 archive: retained-forward-source-28.tar.gz,1482323B,
sha adf32ec32d185b345ab20125abe357fc19e107652b613498dadf4a402b322346; all534 members verified.

Final integrity: retained-forward-final-integrity.json verifies all534 final
inputs,322 unchanged vendor inputs,9 changed files without trailing whitespace,
only3 post-build MD changes, exact executable and all3 archive hashes unchanged.
All handles terminal; source may be edited for the next genuine implementation
step after inspecting current state. Full goal remains active.
