Status: ready-for-agent

# Align Empty and Unavailable Module Semantics

## What to build

Align module rendering and interaction behavior with the accepted distinction between **Empty Module**, **Unavailable Module**, and **Configuration Error**. Empty and unavailable modules render no visible content, leave no gap, have no Tooltip, and are skipped by Keyboard Focus navigation.

## Acceptance criteria

- [ ] A valid Module with a missing runtime dependency becomes an Unavailable Module, not a Configuration Error.
- [ ] A Module with an available data source but no current content becomes an Empty Module.
- [ ] Empty and unavailable Modules render no visible content and leave no gap.
- [ ] Empty and unavailable Modules have no Tooltip and are skipped by Keyboard Focus navigation.
- [ ] Script command failure makes the script unavailable for that refresh rather than keeping stale output.
- [ ] Script command success with empty rendered output becomes an Empty Module.
- [ ] A valid button remains visible; command failure after activation is a failed Module Action, not an Unavailable Module.
- [ ] Tests cover focused-window no-window, tray with zero icons, script empty output, script failure, and missing runtime dependency cases.

## Blocked by

- .scratch/domain-alignment/issues/04-make-bar-and-module-identity-strict.md
