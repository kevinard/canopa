# 0098: Quick Look dismissal returns to its focused owner

Status: scoped native regression verified, 2026-10-20. Extends the source-bound
Quick Look contract and root ADR0021. Full product acceptance remains incomplete.

The integrated Downloads walkthrough reproduced a focus-return failure twice.
After opening a preview from Downloads and browsing A-project in a second window,
focusing the preview and pressing Escape activated A-project rather than its
Downloads owner. The preview's native transient parent was still correct. Its
close path sent no activation request; the compositor's default next-window
choice did not satisfy the source-bound navigation contract.

Use one explicit dismissal method for Escape, Ctrl+W, default Space, a repeated
Quick Look command and native close requests. Remove the preview's presentation
state, then request its live owner's focus before closing the input window, as
required by ADR0021. Request that focus only if the preview currently owns native
keyboard focus. Browser, Trash and Recovery owners use the existing live-source
lookup. A background close must not redirect current input in another window.

Automatic closure because an owner disappears, stale preparation cleanup and an
already closed native window keep their existing behavior and issue no new focus
request. The existing shared Wayland activation support retains the eligible
input surface through the request. No compositor IPC or shared dependency change
belongs in the application implementation.

The maintained Iced Downloads workflow is the regression seam: two tiled browsers,
the source's floating preview, navigation in the other browser, explicit preview
dismissal, and the exact source becoming focused. A countercase closes an inactive
preview after fresh keyboard input in the destination and requires that destination
to retain focus. The same workflow must continue through actual Move/Undo and
graphical image-action creation; passing only the focus prefix is insufficient.

The complete maintained Downloads workflow now passes fifteen grouped checks in
dark/100% and light/125% on the same Iced candidate. Both appearances verify
active dismissal returning to the exact source and inactive dismissal preserving
the other browser's focus after fresh input. They continue through resized
multi-selection, actual Cut/Paste, shared Undo, a pointer Move/Undo, graphical
action creation, a real thumbnail subprocess, captured output in narrow Operations
and source-bound preview of the generated image. The existing dark preview
workflow also passes seventeen checks. All files, settings, providers and sessions
are private fixtures. These forty-seven checks establish this workflow and
regression; they do not complete the preview or file-manager product requirements.

Default build, all-target Clippy with warnings denied and the unfiltered library
suite pass (77 tests; one fresh-process helper ignored and invoked by its parent).
The application correction changes presentation focus only. Operation, activation
transport and shared dependency implementations are unchanged. Primary review
only. See [the exact evidence](../../../.scratch/file-manager/downloads-iced-verification.json)
and [the requirement ledger](../../../.scratch/file-manager/IMPLEMENTATION.md).
