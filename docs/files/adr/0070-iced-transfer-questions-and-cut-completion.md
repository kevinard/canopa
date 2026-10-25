# 0070: Iced transfer questions and Cut completion

Status: implementation in progress, 2026-10-18. Continues ADR0067/0068/0069
and the entire PRD. This does not accept durable restart, complete clipboard,
device, source-descriptor or Recovery ownership.

Undo and transfers share one source-bound Iced question window. The service's
question kind and generation determine the reply; closing or escaping sends
the corresponding Cancel. Automatic presentation is limited to the focused
source Browser or Operations, while manual Operations review remains available.
Use captured file sizes and dates, permitted Replace/Merge/Keep both/Skip
choices, default Cancel, optional batch choices and original-destination Retry.
The floating window stays comfortable above a narrow Browser and uses the
shared Canopa controls and focus treatment.

Apply to remaining conflicts remembers three independent groups: two folders,
two regular files or links, and mixed or unsupported item kinds. A remembered
file Replace never answers a file/folder collision whose safe choices are
Cancel, Keep both and Skip. Ask for a new decision for that group. Its choice
does not erase the earlier file choice; later ordinary file conflicts retain
their own reviewed policy. This applies equally to Copy and Move.

Every new live question generation sends one desktop attention notification,
including when its source is in the background. Track this separately from
whether the question window has been presented. Notification completion uses
the existing application hold and error reporting; it does not activate or
answer the question. Repeated service updates must not repeat the same notice.

Cut completion must follow the operation's confirmed result. Failed, skipped
and remaining work keeps the corresponding original top-level Cut references.
Uncertain results retain the selection for review. A completed Move may update
only its original ownership token; another Copy/Cut or external owner wins.
For the same token, apply remainders by intersecting the currently served file
set. Multiple admitted Moves can finish from an earlier selection; their partial
results must only remove files, regardless of response order. A late result
cannot add a previously consumed file again. An empty intersection destroys only
that original source. A new explicit Copy/Cut creates a new intent.

The transfer engine must also report original source folder remainders when
new work arrives during a decomposed Move or Merge. Use the captured directory
scope identity to distinguish that original folder from an unrelated folder
later created at the same pathname. Supply these descriptive facts in the
transfer result and shared report. The Iced thread must not infer completion by
probing arbitrary current paths. Remainders do not grant filesystem authority,
reconstruct a Retry plan or make a restored operation executable.

Verification uses the established public transfer engine/service and actual
private files, including arrivals during Merge and a recreated source folder.
Native acceptance extends real Iced keyboard/menu commands through actual
copies, moves, conflict choices, completion notifications and shared Undo in
private Niri, with both themes and narrow/wide tiles. Preserve the separate
retained-source, journal, media and descriptor gates before claiming completion.

Captured facts in transfer and Undo questions use the same keyboard scrolling as
result inspection: Home/End, Up/Down and Page Up/Down. Scrolling leaves action
focus unchanged, including default Cancel. Choices and the optional batch control
remain outside the facts viewport, so reviewing metadata at the minimum size
never activates or silently changes a decision. This follows the native compact
folder-conflict failure: End previously left the incoming and existing metadata
unreachable without a pointer.

Question facts wrap long paths between characters when necessary. Their vertical
scrollbar reserves layout space, so it cannot obscure the reviewed path or date.

An explicit successful answer, including Cancel, requests focus for the surviving
Browser or Operations parent before retiring the question. This uses the shared
ordinary-window focus mechanism in root ADR0021 while the actual input source
still exists. The native three-window regression reproduced cancellation leaving
the wrong Browser focused. Automatic retirement or parent disappearance still
dismisses auxiliary windows without issuing an unrelated focus request.

Once a live transfer has observed its destination folder's identity, Retry must
retain that identity through missing folders and unrelated replacements at the
same pathname. Restoring the original folder allows the transfer to continue;
using another folder requires cancelling and starting a new request. A transfer
that has never observed a destination still accepts its first reviewed folder.
This protects the completed prefix across live questions; it does not establish
queued, restored, removable-media or descriptor authority.

Common folder Copy conflicts now have native acceptance in both maintained
themes. A floating question above a narrow Browser exposes both captured folders
through all six scrolling keys at 320×240 while preserving action focus. Keep
both copies a real nested tree and shared Undo removes only that copy. Default
Cancel and explicit Skip leave both original trees intact.

Batch acceptance merges two existing destination folders, independently applying
file Keep both, Replace or Skip. The original destination-folder identities and
unrelated files remain intact; Undo restores previous payloads or removes only
completed copies. These payload checks leave engine-owned retained directories
uninterpreted. They do not establish retained-container authority or cleanup.
Actual Undo questions also pass minimum-size Home/End review before the existing
Keep item and queued-original-volume workflows in both themes.

The earlier parent-focus and live Retry checkpoint passes 420 core tests, 51
Iced library tests and 42 declared native checks across eight dark/light cases.
The public service regression covers Copy and Move through missing and unrelated
replacement folders, original-folder restoration and shared Undo. Native Copy
acceptance checks those same filesystem results and successive typed questions.
It uses ordinary private directory relocation, not actual device removal.

Both themes also pass the three-window parent-focus regression, common folder
conflicts and existing name/Undo/Operations workflows. All-target Clippy with
-D clippy::all passes with three existing unused-helper warnings. A test-only
lint cleanup is separately verified by all 19 transfer-service tests, already
included in the core count.

The subsequent batch-choice checkpoint passes all 44 transfer engine/service
tests and 36 declared native checks across eight dark/light cases. A genuine
native and public-service regression first reproduced ordinary-file Replace
turning two mixed collisions into failures without asking. Copy and Move now
ask a fresh mixed question, keep its Keep both policy independent from later
file Replace, and reverse the actual results through shared Undo. Both mixed
type orientations preserve the existing destination kinds and bytes.

Ordinary folder Move acceptance also covers Keep both, Cancel, Skip and two-
folder Merge batches with file Keep both, Replace or Skip. Completed Move leaves
preserve their identities through Undo; skipped files stay at source, and empty
source folders are removed only for completed Merge work. Destination folder
identities and unrelated files remain intact. Copy regressions pass separately.
The Iced presentation is unchanged; all-target Clippy passes with the same three
existing unused-helper warnings. The full core, Iced library, workspace and
release gates have not been rerun for this batch-only engine change. Additional
typed questions, retained-source leases, safe Retry/Continue and original-media/
descriptor/restart authority remain open.
