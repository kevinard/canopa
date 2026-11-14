# Screen-reader layout announcements

Status: implemented in the Shell; the tabbed-display part needs Niri.

When keyboard focus moves to another column, another window of a stacked
column, or another workspace, the Shell announces the new place politely, for
example "Column 2 of 4, window 1 of 2" and, on a workspace change, ", workspace
3". Window titles are left to the application's own focus announcement, so they
are not read twice. Nothing is announced when focus stays in place, when a
floating window is focused, or when a Shell surface takes the keyboard.

The announcer is a small AT-SPI application named "Window layout", sharing the
announcer the on-screen display already used (moved to
`crates/canopa-shell/src/app/announcer.rs`). AccessKit sends nothing while no
assistive technology listens.

Still open: "tabbed" cannot be said because Niri's IPC does not report a
column's display mode. That needs a field on Niri's IPC window layout or
workspace, in the fork.

## Validation

- `cargo test -p canopa-shell`: a new test checks the announcements for moves
  between columns and within a stacked column, and that repeats and Shell focus
  say nothing. The OSD tests still pass after the announcer move.
- Not checked with Orca in a live session.

## Comments
