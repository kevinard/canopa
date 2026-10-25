# Live transfer continuation and original selected sources

The full PRD goal remains active. The previous turn made concrete FileManager1
progress; this turn reproduces and fixes selected-source loss through the public
transfer engine. It does not complete application Retry/Continue or restart.

## Current code and evidence

Source01 has only the new replacement regression relative to FileManager1's
final source22. Red02 exits101: Copy Continue copies the unrelated replacement,
completed1 where the test requires0. All files are private temporary fixtures.

TransferContinuation now retains the original ObservedTransfer in memory.
Initial cancellation, repeated destination pauses and remaining/failed outcomes
share those selected pins; resume never captures replacement sources. Mutated
kind/destination are refused. Portable encoding remains unchanged and decoding
has no process observations.

Source04 passes the new tests but exposes three old directory-test failures in
Core05: two already completed postorder roots and a selected FIFO replaced by a
different file. Source07 recognizes prior traversal only when DirectoryFinish
matches the held selected pin. The selected-file test now parks/restores the
actual original parent, retaining all unselected-parent and Undo assertions.

Core08 exits0 across transfers, directory Retry/Continue, checkpoint and handoff
targets. The three new tests pass, including Copy/Move replacement refusal,
original-object return, separate completed-prefix Undo, a raw dangling link
through cancellation/unavailable placement, and refusal of changed intent.
This focused pass did not yet establish broader validation; the results below
supersede that intermediate checkpoint.

Full core build10 exits0 in6m54. Full run11 uses private HOME/XDG directories,
no display/system bus and a private session bus without service directories.
It exits101 at clipboard_capacity: review-only saved results retained the old
source observation after clipboard replacement. The application now explicitly
converts its stopped outcomes to saved plans, retiring process pins there.
Public live engine continuations still retain observations. The existing
clipboard test remains unchanged; the failed run and private-runner receipt
are retained. The unchanged focused parent and its actual limit128 subprocess
pass in attempt14 after the saved-plan conversion.

## Broader verification and native diagnosis

Build15 exits0. Full private core16 exits0 in486.86s: 65 targets, 552 parent
tests and the custom device-runtime harness. Its 36 ignored entries include
parent-invoked helpers and fixture-specific cases; do not count all as executed.
The bus has no activation directories and HOME/XDG are private. Default Iced
library18 passes78 with1ignored; all-target Clippy19 with warnings denied and
default build20 pass. Source13 contains the final compiled Rust/test changes;
source17 adds only desktop-lib/private-audio options to the general runner.

The current executable is133410896B, SHA256
6e74275b879764150fa3ea9d75411586b005a0822991013811a5dd7ce08ae219.
Preservation21 verifies all701 source17 inputs and its decoded gzip. All previous
source/app/probe archives, including candidate5e8, remain preserved.

Native22 fails: dark first-window startup times out with zero windows and empty
Files stderr; light reaches a retained transfer question but later activates
Cancel. Control23 runs the preserved old5e8 executable and reproduces that same
Review failure. Diagnostic25 changes only captures: after closing Details the
question remains pending; after four Tabs Cancel is outlined. Recovery is an
enabled keyboard stop omitted by the old fixture. Source26 changes three Review
traversals4->5 and older-job Details5->6, with diagnostic captures retained under
useful names. The GUI, timeouts and filesystem assertions are unchanged.

Native27 passes standard transfers in both appearances,6 checks each. Native28
passes clipboard capacity in both appearances,3 checks each, with the existing
limit128. Native29 passes parent closure/reopening2 and minimum-inspector1 in
each appearance. All eight cases and24 checks pass on candidate6e7 with pinned
Niri63e, private files, buses and GIO providers. Final summaries include the
unchanged compositor checksum. All runs are terminal. Representative Review,
clipboard replacement/Undo, retained-question, old-result and compact-inspector
frames were inspected. The initial dark first-window timeout remains an open
reliability issue; the keyboard correction does not resolve it.

Primary standards/spec review of the full delta from FileManager1 source22 has
no unresolved scoped findings; no independent/delegated review is claimed.
Final source30, scope/diff, preservation and continuation-verification.json bind
the results to the executable and retain every unsuccessful attempt. Post-build
changes affect documentation and the native fixture only; compiled Rust remains
unchanged from source13. Full workspace and release gates have not been run for
this slice.

## Remaining work

Application Retry/Continue needs an original core-owned plan, original placement
and request revision through atomic handoff, with lifetime holds, save-failure
ordering, cancellation and clipboard completion. Iced should render those core
actions and outcomes. Neither a retention lease nor a decoded payload supplies
execution authority. Restored rows remain review-only until original source,
stable media and request/journal proof permit exact continuation.

Do not replace durable continuation with permanent refusal or fresh selection
silently. Keep the full intended workflow active, including completed-prefix
Undo, isolated failures, both plan orders and truthful interruption recovery.
Descriptor-relative access, stale-plan replay, accessibility, performance, full
workspace/native/optimized/hosted release, compositor distribution and GTK
retirement also remain open. Preserve every earlier source/app/probe archive.
