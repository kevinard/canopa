# Settings window layout

Status: implemented as a section of Personalization; per-application widths not done.

Personalization gains a "Window layout" group, applied to Niri at once like the
existing compositor motion switch, not through the page's Save footer:

- Space around windows: None, Small, Medium, Large (`gaps` 0, 6, 12, 20).
- Center the focused column: Never, When it does not fit, Always.
- New window width: one third, half, two thirds, full width
  (`default-column-width`).
- "Use the Canopa layout" removes the override.

Settings writes a marked `layout` block at the end of `niri.kdl`, before the
reduced-motion block (which must stay last), and recognises it only when it is
exactly one it could have written; a hand-edited block disables the controls
with an explanation. Saving validates with Niri, reloads it and restores the
previous file if the reload fails, through the same `save_main` path as the
motion switch. Like the motion switch it is refused while the Shortcuts page has
unsaved edits.

Why a section and not a sidebar page: a new page renumbers Ctrl+N for every
later page and the page registry's public ids; three controls do not justify it
yet. Per-application widths need a list editor of window rules and would make a
page worthwhile.

## Validation

- `cargo test -p canopa-settings`: all pass, including round trips with and
  without the motion block, refusal of an edited block, parsing every layout
  Settings can write, and a check that the shipped values match
  `packaging/niri/defaults.kdl`.
- Private Weston/Niri session including the packaged defaults followed by a
  written block (gaps 0, one third): a new window's tile was 426 × 800 on a
  1280 × 800 output, so the block overrides the shipped `layout`.
- The Settings window itself was not exercised.

## Comments
