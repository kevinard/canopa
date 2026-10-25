# 0052: Keep Operations usable in narrow tiles

Status: accepted. Native verification is recorded in the implementation ledger.
The full product brief remains in progress.

## Context

The user chose automatic adaptation for tiling while keeping the same navigation
and keyboard behavior. The shared Operations window must meet that expectation
alongside the Browser and floating Quick Look.

A native Recovery workflow with an unbroken long filename reproduced a 987-pixel
minimum when the compositor requested 360 pixels. Changing only text wrapping
reduced that minimum to 456 pixels. The remaining minimum came from the horizontal
Continue and Undo controls.

## Decision

Operation headings, status, details and the window hint wrap at word boundaries
or characters when needed. This changes display layout only; filenames and paths
keep their original filesystem bytes.

The existing action buttons flow onto additional rows as space shrinks. They keep
the same callbacks, order and availability. Hidden buttons do not reserve empty
cells, and flow wrappers add no extra keyboard focus stops. Buttons use the
shared Canopa appearance. Wider windows place actions together again.

Alt+R still retries failed items, Alt+C continues remaining work, and the Review
button gains Alt+V for reopening a deferred decision. These mnemonics belong to
Operations at every width. This change introduces no separate narrow navigation
mode or operation semantics.

Recovery metadata identifies directories as Folder, symbolic links as Link with
their existing size, and special files by kind. Regular files keep their size.
The directory entry's inode size is not a recursive content size and is no
longer presented as one. All metadata comes from the prepared snapshot; rendering
adds no filesystem reads on the main thread.

Selectable error text uses the shared AccentSurface and text colors. Native
Retry captures exposed an inherited GTK text selection with poor contrast in
dark mode, including after the selected label lost focus. The scoped selection
rule keeps selected paths and error explanations readable in both schemes.

## Verification and limits

The native tracer resizes Operations 360→640→360, checks actual compositor sizes
and retained window focus, and captures Continue alongside Undo. It defers a
conflict and reopens it with Alt+V, copies the exact long filename with Keep both,
uses floating Quick Look from a 360-pixel Browser, and reverses the completed
parts while preserving newer work. The ordinary cross-filesystem failed-directory
tracer also uses actual narrow Operations and Alt+R, including temporary source
unavailability and clipboard ownership.

The width failure observations are retained as native 114 and 115. Native 118
passes the functional Retry workflow but exposes the selected-text contrast
defect. Native 116/117 verify the layout, long filenames and Review in dark and
light at 1.25× scale, with 20 checks each. After adding the selection-color rule,
native 119/122 verify Retry, Undo, clipboard ownership and readable selected text
on the final binary, with 22 checks each. All 46 captures from these four passing
workflows were inspected. The later color rule changes no layout or operation
logic. Clippy, the desktop build, formatting, Python syntax, native inventory,
whitespace and packaged documentation checks pass. Default selected-text contrast
is 8.51:1 dark and 11.00:1 light.

Light attempts 120/121 received SIGTERM and remain failed artifacts. The unchanged
122 case passed with its runner attached to a terminal; the signal's source
remains unknown. No termination handler was bypassed or failure suppressed.
Exact candidate hashes, results and inspected captures are recorded in
`.scratch/file-manager/operations-tiling-verification.json`.
The engine, history schema and filesystem tests are unchanged by this decision;
the prior 304-test core result applies only to the byte-identical non-desktop
inputs listed in the source-reuse record. This is scoped tiling verification,
not completion of the broader Recovery or file-manager requirements.
