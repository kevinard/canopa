# Icon controls and a footer for provider panels

Date: 2026-11-28
Status: accepted

Provider panels (the notification Center first) drew their view and settings
actions as labeled chips in the heading and their content actions as a toolbar
under it. Two rows of controls sat above the first notification, and the header
read as crowded beside the other panels, whose headings hold a title and round
icon buttons only.

## Decision

- `PanelAction` gains an optional `symbol`: a symbolic icon name such as
  `preferences-system-symbolic`. It is a hint like `secondary` or `primary`:
  older hosts ignore it and it serializes only when set. A name is ASCII letters,
  digits, `-`, `_` and `.`, never a path; the host draws only symbols it ships,
  so a provider cannot make the shell load an arbitrary image.
- Canopa Shell draws a secondary action whose symbol it knows as a 32-pixel
  round icon button beside Close, accented while pressed, with the action's label
  as its tooltip and accessible name. An unknown symbol falls back to the chip.
- Content actions move to a footer below the list: everyday ones lead and
  destructive ones sit apart at its end. The list takes the height the footer
  leaves.
- The notification daemon marks **Group by app** `view-group-symbolic` and
  **Settings** `preferences-system-symbolic`.

## Consequences

- The Center has one heading row (title, two icon buttons, Close) and its bulk
  actions (**Mark all read**, **Clear all**) at the end of the list, where they
  act on what was just read.
- A provider that wants an icon the shell does not know keeps its labeled chip;
  adding a symbol is a shell change, not a protocol one.
- Keyboard focus order is unchanged: affordances follow the document, and the
  icon buttons keep their actions' semantic nodes.
