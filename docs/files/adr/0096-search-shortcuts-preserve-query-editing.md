# 0096: Search shortcuts preserve query editing

Status: scoped native behavior verified, 2026-10-20. Extends the Iced search
interface and ADR0095. Primary review only; full product acceptance stays open.

## Problem and decision

The recursive-search drag workflow exposed an actual input-routing defect. With
the query field focused, configured Alt+S inserted `s` into `needle`, producing
`needles`, instead of toggling Include subfolders. An application listener after
the native text input cannot prevent text that the input has already inserted.
The earlier full-search fixture moved focus into results before using that
shortcut, so it did not cover this case.

Handle configured search-local commands before the query's native text input:
Include subfolders, Stop, More results, Show in folder and unreadable-location
details. These commands act on the existing search session and must leave the
query's native focus, selection and caret alone. Do not repair focus afterward
or select the query again. Scope this precedence to the search controls; keep
ordinary editing and other browser commands on their existing routing.

The private focus frame reuses its existing shortcut interception with an empty
traversal order and an explicit preserve-focus policy. Existing form frames keep
their focus-clearing behavior. Native Tab traversal remains available in the
search controls. Configuration still owns the bindings; this introduces no new
shortcut or search command.

This is presentation behavior in the Iced adapter. Search requests, workers,
results, cancellation and filesystem operation authority remain on their existing
interfaces. No backend or shared dependency change is needed.

## Verification and limits

The original focused-query failure is preserved in choices Native04. On the
corrected executable, the actual drag workflow toggles scope with the query
focused, appends `x`, removes it with Backspace, and moves the real nested match
through the shared transfer and Undo workflow in both appearances. This proves
continued editing at the existing end caret; it is not an exhaustive input-method
or selection-range acceptance test.

The maintained full filename-search workflow is now registered in the native
matrix for dark/100% and light/125%. Its sixteen checks per appearance pass:
current and recursive filename matching, exact launches and raw parent bytes,
hidden/unreadable locations, superseded requests, independent tabs, restored
selection/scroll, and Stop/More across 50,005 actual matching files. Representative
rendered results were inspected. All-target Files Clippy with warnings denied,
the default build and 77 unfiltered library tests pass. One ignored helper is
invoked by its fresh-process parent; it is not another parent test.

Exact attempts, source/executable archives and remaining gates are in
[the choices evidence](../../../.scratch/file-manager/dnd-choices-verification.json).
Accessibility, broader editing acceptance, production performance and the full
Files brief remain open. This decision changes neither resident hosting nor its
same-process frontend crash limitation.
