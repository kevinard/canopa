# 0153. Select Qt's portal theme and float portal dialogs in the packaged session

Status: accepted

## Context

Canopa's file chooser, app chooser, screen-sharing picker and the other portal
dialogs reach applications through `xdg-desktop-portal`. GTK applications use the
portal on their own. Qt applications use it only through Qt's `xdgdesktopportal`
platform theme plugin, which Qt selects when `QT_QPA_PLATFORMTHEME` names it. The
packaged session left the variable unset, so Qt applications kept Qt's built-in
file dialog unless the user set it by hand.

Portal dialogs use the Wayland app ID `org.canopa.Portal`. With a parent window
(xdg-foreign) Niri floats them like any parented dialog. Without one, or when the
parent handle has expired, Niri opened them as a new tiled column, which
rearranges the strip for a short-lived dialog. The documentation offered an
optional window rule.

## Decision

- `canopa-session` sets `QT_QPA_PLATFORMTHEME=xdgdesktopportal` in the session
  environment when the variable is empty. A theme the user already chose (for
  example `qt6ct`) is kept, and the session removes only a value it set itself
  when it ends.
- The shipped Niri defaults (`packaging/niri/defaults.kdl`) float
  `org.canopa.Portal` windows, in the same rule as the authentication and
  passphrase prompts.

## Consequences

- Qt applications open Canopa's dialogs without extra setup. Where the plugin is
  not installed, Qt logs a warning and keeps its own dialogs, as before. Apart
  from dialogs, the portal theme wraps Qt's default theme for the desktop; recent
  Qt versions also read the color scheme from Canopa's Settings portal.
- Users who prefer tiled portal dialogs override the rule in `niri-custom.kdl`.
  Sessions other than the packaged one still need the variable and the rule set
  by hand; `docs/portals.md` describes both.
