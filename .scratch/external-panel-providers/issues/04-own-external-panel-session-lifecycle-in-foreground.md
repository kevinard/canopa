Status: ready-for-agent

# Own External Panel Session Lifecycle In Foreground

## Parent

[PRD: External Panel Provider Platform](../PRD.md)

## What to build

Make **External Panel Sessions** obey Niribar's one-active-**Foreground** model. Opening or replacing interactive overlays must close the current provider session, clean up provider process I/O, and prevent stale provider messages from mutating a newer panel.

This slice proves that provider panels are normal foreground panels rather than a parallel overlay lifecycle.

## User stories covered

6, 7, 29, 30, 31, 32, 41

## Acceptance criteria

- [ ] Niribar has at most one active **External Panel Session** at a time.
- [ ] Opening another provider panel closes the previous provider session.
- [ ] Opening a normal **Panel** or **Menu** closes any active provider session.
- [ ] Closing the hosting **Panel** ends the active provider session.
- [ ] Provider stdin/stdout work is cleaned up when the provider session ends.
- [ ] Provider messages from an ended or replaced session are ignored.
- [ ] Provider panels still use existing **Panel Origin**, placement, and stale panel close behavior.
- [ ] Tests cover replacement, close, cleanup, and stale-message behavior through **Foreground** or **Panel Runtime** seams.

## Blocked by

- [02 - Render Static External Panel Content After Handshake](02-render-static-external-panel-content-after-handshake.md)
