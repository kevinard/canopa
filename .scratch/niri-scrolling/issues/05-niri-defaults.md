# Niri defaults pass

Status: implemented.

`packaging/niri/defaults.kdl` now adds:

- `preset-column-widths` ⅓, ½, ⅔ with `Mod+R` / `Mod+Shift+R`.
- `Mod+Ctrl+F` expand-column-to-available-width; `Mod+C` center-column.
- `Mod+BracketLeft` / `Mod+BracketRight` consume-or-expel-window-left/right.
- `Mod+W` toggle-column-tabbed-display.
- `Mod+Home` / `Mod+End` focus-column-first/last, with `Shift` to move the column.
- `Mod+WheelScrollDown/Up` changes workspace (150 ms cooldown) and
  `Mod+WheelScrollLeft/Right` or `Mod+Shift+WheelScrollDown/Up` changes column,
  following Niri's own default configuration rather than the first draft of
  this issue (wheel to columns), so Niri users find the same bindings.
- `Mod+1…9` focus-workspace and `Mod+Shift+1…9` move-column-to-workspace.
- The top-left hot corner opens the overview.
- Pinentry, Keyring and Polkit prompts open floating. Niri already floats
  parented dialogs such as Quick Look, so they need no rule.
- The Shell wallpaper is placed within the overview backdrop.

Already on in the pinned Niri without configuration: touchpad gestures and the
recent-windows switcher (Alt+Tab).

Not done: background blur. Niri 26.04 can blur layer surfaces with a
`background-effect { blur true; }` layer rule, but Canopa's theme roles are
opaque, so the blur would not show; add the rule with a translucent surface
theme.

Settings: the shortcut editor accepts Niri's wheel, touchpad-scroll and mouse
button triggers (before, any such binding made the editor refuse to save), and
labels the new bindings in English and French, including numbered workspace
bindings ("Go to workspace 3").

## Validation

- `/usr/lib/canopa/niri validate -c packaging/niri/defaults.kdl` (the pinned
  Canopa Niri 26.04): valid.
- `cargo test -p canopa-settings`: all pass, including a new pointer-trigger
  test. One fixture that simulated a package update moving close-window to
  `Mod+W` now uses `Mod+Y`, since `Mod+W` is taken.
- Not checked in a live session.

## Comments
