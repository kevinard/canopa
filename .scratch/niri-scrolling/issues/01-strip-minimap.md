# Strip minimap

Status: implemented; native acceptance in the live session pending.

A `minimap` bar module draws the scrolling strip of the workspace active on the
bar's output (the focused workspace when the surface has no output name).

- Columns come from `pos_in_scrolling_layout`, left to right; a column's tiles
  split its thickness by their heights (tabbed columns therefore show as equal
  splits, since Niri does not report the display mode).
- Widths are to scale: the output's logical height, from Niri's `Outputs`
  request, is drawn at `size`; before that arrives, the tallest column stands in.
  The strip shrinks to fit `max-length`.
- The focused window is accented, the active column is brighter, urgent windows
  use the warning color. An empty workspace shows a screen outline.
- Click focuses a window (`FocusWindow`); scrolling moves focus to the next or
  previous column (`FocusColumnRight`/`Left`), which acts on the focused
  workspace, not necessarily the bar's output.
- Accessible description: "Window strip, column 2 of 4, <title>", plus the
  number of floating windows, which are not drawn.
- Configuration: `size` (8–64, default 18), `max-length` (default 240, 0 = no
  limit). Settings lists it among the widgets.

Shell changes that came with it: the Niri state now applies
`WindowLayoutsChanged` and `WindowUrgencyChanged` and keeps output sizes
(fetched on connect, configuration reload and output changes). Scrolling over
any module now accumulates smooth (touchpad) scrolling into 20-pixel steps and
ignores the zero-length event that ends a gesture; before, every touchpad event
was a step and the end of each gesture counted as a step down (this also
affected volume).

Not done: framing the visible area needs issue 07.

## Validation

- `cargo clippy -p canopa-shell --all-targets` and `cargo fmt --check` are clean.
- `cargo test -p canopa-shell`: all pass. One AT-SPI quick-settings test and
  three launcher process/qalc tests timed out under full-suite load in separate
  runs and passed when rerun alone.
- Settings widget tests pass.
- Private Weston/Niri session (1280×800, five GTK windows, one stacked column,
  one at 80% width), with a top and a left bar: the columns measured 13, 13, 22
  and 13 px for tiles of 602, 602, 970 and 602 px. The strip is centered
  across both bars and follows focus. Clicks on a column and on the upper and
  lower tiles of the stacked column made those windows active; 24 px of touchpad
  scrolling moved exactly one column each way. Evidence:
  `.scratch/artifacts/niri-scrolling/minimap-2` and `minimap-3`.
- After a click the bar keeps keyboard focus, as with
  the taskbar (Shell ADR 0026): the window is active but not keyboard-focused.
- The scroll fixture `scripts/desktop-fixture/scroll.c` needs `-lm`, which
  `validate-desktop.py`'s `build_pointer("scroll")` does not pass; the check
  compiled it separately.

## Comments
