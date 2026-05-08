Status: ready-for-agent

# Target Global Toggle by Focused Output

## What to build

Change **Global Toggle** behavior so `niribar --toggle-expand` targets Bar Surfaces on the currently focused Output when known, falls back to all Bar Surfaces when the focused Output is unknown, and respects Expansion-disabled Bars.

## Acceptance criteria

- [ ] Global Toggle affects only Bar Surfaces on the focused Output when focused Output is known.
- [ ] Global Toggle falls back to all Bar Surfaces when focused Output is unknown.
- [ ] Global Toggle ignores Bar Surfaces whose Bar has Expansion disabled.
- [ ] Explicit expansion changes close affected open Menus before changing Expansion.
- [ ] Passive menu pinning still prevents hover-driven collapse while a Menu is open.
- [ ] Tests cover focused-output targeting, unknown-focused-output fallback, expansion-disabled Bars, and open Menu behavior.

## Blocked by

None - can start immediately
