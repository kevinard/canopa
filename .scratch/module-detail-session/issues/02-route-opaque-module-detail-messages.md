Status: ready-for-human

# Route Opaque Module Detail Messages

## Parent

.scratch/module-detail-session/PRD.md

## What to build

Route opaque **Module Detail Session** messages from rendered **Module Detail Content** back to the active originating **Module**. The app should not expose or match on clock/network/audio-specific detail event variants. It should only validate that the message targets the active **Panel Origin**, pass the message to the owning **Module**, and handle returned effects according to ownership.

This slice should introduce the effect contract needed by module-owned detail behavior: module-owned effects remain with the owning **Module** or its runtime path, while app-owned effects are returned to the app for shared infrastructure.

## Acceptance criteria

- [x] Detail messages are opaque at the app seam and are routed back to the active originating **Module**.
- [x] Detail messages whose target is not the active **Panel Origin** are ignored.
- [x] Detail messages after panel close or replacement do not mutate stale **Module Detail Sessions**.
- [x] The detail update result can distinguish app-owned effects from module-owned effects.
- [x] Tests cover successful routing, stale message dropping, and returned effect classification without introducing module-specific app-level detail enums.

## Blocked by

- .scratch/module-detail-session/issues/01-add-module-detail-session-seam.md
