# Export rich body content through panel protocol v3

Status: implemented
Type: AFK

## Parent

[Body-Embedded Notification Images](../PRD.md)

## What to build

Advance the shared panel protocol to carry generic rich body content for **Notification Center Projection** items. The panel provider should emit ordered body segments, including styled text and body-embedded local image references, while preserving the plain body fallback for consumers that need text.

## Acceptance criteria

- [x] The shared panel protocol version advances for the rich body content shape.
- [x] Panel item output can represent ordered styled text and body image segments without exposing raw freedesktop markup.
- [x] Panel provider output includes body image references for notification center items when accepted notifications contain them.
- [x] Panel provider output preserves the plain **Notification Body** fallback.
- [x] Existing notification center action, app permission, active/history grouping, and live subscription behavior still works.
- [x] Panel protocol tests cover serialization of rich body content and explicit protocol versioning.
- [x] Panel provider tests cover snapshot or subscription output for a notification containing text, emphasis, and a body image.

## Blocked by

- [01-parse-persist-and-project-ordered-body-segments.md](01-parse-persist-and-project-ordered-body-segments.md)

## Comments

- Implemented panel protocol v3 `rich_body` item content with ordered styled text and local image segments, and mapped Notify notification center projections into it. Verified with `cargo test -p canopa-panel-protocol` and `cargo test -p canopa-notify`.
