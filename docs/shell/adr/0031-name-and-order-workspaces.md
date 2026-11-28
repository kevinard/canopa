# Name and order workspaces from the bar and the overview

Date: 2026-11-28
Status: accepted

Workspaces are shown by name in the bar's pills, their hover cards and, with the
fork's `overview { workspace-labels; }`, above each workspace in the overview.
Until now a name could only come from Niri's configuration, and the order only
from keyboard bindings.

The overview briefly presented workspaces as cards (`workspace-cards`: wallpaper
inside rounded workspaces, outlines, window name pills, a screen-filling zoom).
Reviewed against Niri's own overview, it added little beyond the names, and the
window pills repeated the titlebars. The shipped defaults keep Niri's overview and
turn on only the labels; `workspace-cards` stays in the fork as an option.

## Decision

- A workspace pill's right-click menu, a Shell menu like the taskbar's, offers
  **Rename…**, **Move up** and **Move down**. Moves use Niri's
  `MoveWorkspaceToIndex` on the workspace's id.
- **Rename…** opens the Shell's own dmenu prompt with free text; a named
  workspace also offers **Use the numbered name**. The answer becomes Niri's
  `SetWorkspaceName` or `UnsetWorkspaceName`.
- In the overview, the fork turns a workspace label into a control: dragged, it
  moves the workspace; clicked without dragging, it sends the momentary
  `OverviewWorkspaceLabelClicked` IPC event, which the Shell answers with the same
  rename prompt. The compositor does not edit text itself.

## Consequences

- Names set this way are Niri's runtime names: they last until Niri restarts,
  and a named workspace stays even when empty. Persisting them is left to Niri's
  configuration.
- A name another workspace already uses is ignored by Niri.
- Other IPC clients can ignore the new event; `niri msg event-stream` prints it.
