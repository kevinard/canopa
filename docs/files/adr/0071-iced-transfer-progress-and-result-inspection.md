# 0071: Iced transfer progress and result inspection

Status: implementation in progress, 2026-10-18. Continues the entire PRD and
ADR0067–0070. This does not accept Retry/Continue or restored execution.

Operations shows the worker's current path, confirmed item count, streamed byte
count and verification stage while a transfer is active or waiting. The worker
does not provide a reliable recursive total, so the interface must not invent a
percentage. Rendering reads the coalesced immutable snapshot and performs no
filesystem probes.

An explicit Details action opens one floating inspector attached to Operations.
Alt+D opens details for the newest transfer with progress or recorded results on
the currently visible page.
The inspector stays bound to that operation ID while other jobs arrive. It can
show live progress before a result exists, then its recorded result. It never
adopts another job when the original disappears. Closing Operations closes its
inspector; closing the inspector does not cancel or answer the operation.

Recorded results separate completed destinations, failures with source/destination
and explanation, skipped items, remaining work, and original source folders to
review. Each section has bounded pages of twenty entries and stable keyboard
controls. Sections and buttons wrap in narrow windows; long path display may
wrap between characters. Paths retain their original bytes until presentation.
Recorded paths describe the stopped result and grant no new execution authority.

Page commands carry the inspector, job and selected section, rejecting stale
commands after a section or operation change. Selecting a section or another
operation resets the page. Previous/Next and item ranges stay outside the
scrolling facts. At either end, keyboard focus moves to the available page
direction. Home/End reach the beginning/end of the current page; Up/Down scroll
by a line and Page Up/Down use the actual viewport height with a small overlap.

At a viewport height below 400 logical pixels, keep section choices in one
horizontally scrollable row with shorter labels. The report heading retains the
full section name. Tab keeps the same five section stops and reveals a focused
choice horizontally. The normal-height view continues to wrap its section
buttons. This leaves room for recorded facts, paging and Close at the declared
minimum size. Both modes retain the same scrollable wrapping-row widget tree,
changing its width constraint. The controls' actual widget state preserves
section focus through compact→normal→compact resizing; IDs alone do not preserve
focus when a subtree is replaced.
Compact margins and a section scrollbar with reserved layout space leave more
room for facts and keep the focused control's outline unobscured.

Native parent-close acceptance confirms the normal Close→Closed path already
dismisses the inspector and question without answering the waiting transfer.
Closing the inspector or its parent remains distinct from explicit cancellation.
Reopening Operations can review the retained question and reverse only the
confirmed prefix after explicit cancellation.

The established native acceptance seam uses real private files: one confirmed
copy, one unsupported FIFO, a later filename conflict, and unstarted work. Close
the source Browser while the operation waits, inspect its live progress beside
narrow Operations, then inspect the stopped report and reverse its confirmed
portion through shared Undo. Add long-report paging and both themes. Keep all
existing source/journal/descriptor, activation and full product gates open.

Returning focus when the inspector closes uses the shared Wayland focus mechanism
in root ADR0021, requesting focus while the input source still exists. Its native
three-window regression remains required before accepting the inspector.

Focused progress and lifetime verification now passes six native scenarios with
eighteen declared checks across dark/100% and light/125%. They cover real partial
Undo, original job binding, keyboard scrolling, both page directions, section
reset, explicit job changes in the same inspector and the normal native parent-
close path with an unanswered question. Minimum-size captures show scrollable
failure facts, visible paging and Close, horizontally revealed section focus and
focus retention through shrinking, expanding and shrinking again. The compact
regression first reproduced clipped facts; a separate regression reproduced
lost section focus when replacing the widget subtree. Both are fixed.

All 51 Iced library tests, all-target Clippy with -D clippy::all and both review
axes pass for this scope. Three existing unused-helper warnings and the full
workspace gate remain open. Remaining typed transfer questions, retained-source
leases, safe Retry/Continue, restart and storage authority, real drag and drop,
activation and the broader product migration still require their own acceptance.
