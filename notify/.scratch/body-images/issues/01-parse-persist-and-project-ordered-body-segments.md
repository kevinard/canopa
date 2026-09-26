# Parse, persist, and project ordered Notification Body Segments

Status: implemented
Type: AFK

## Parent

[Body-Embedded Notification Images](../PRD.md)

## What to build

Introduce ordered **Notification Body Segments** as the interpreted body content shape for accepted notifications. Existing text emphasis and line breaks should become text segments, body-embedded image markup should become image segments with local references and fallback text, the plain **Notification Body** should remain the text fallback, and persisted history should store the new segment shape instead of the old text-only interpreted body shape.

## Acceptance criteria

- [x] Existing **Notification Body Markup** emphasis and line-break behavior is preserved through the new segment model.
- [x] `<img>` body markup with an absolute local path or `file://` URI creates an ordered **Body-Embedded Notification Image** segment.
- [x] Remote URIs, relative paths, icon names, and unsupported image references do not create image segments.
- [x] `alt` text contributes to the plain **Notification Body** fallback in sender order.
- [x] A body image without `alt` does not add invented placeholder text to the plain fallback.
- [x] A parseable local body image can make an otherwise blank notification accepted instead of no-op dropped.
- [x] Multiple text and image segments retain sender order.
- [x] **Notification History** persists body image references but not image bytes.
- [x] Restarting the daemon reads persisted body segments back into active or historical notifications.
- [x] Migration or startup repair derives body segments from **Raw Notification Body** for existing unreleased developer databases.
- [x] Diagnostic JSON can expose structured body content while human-readable control output remains plain fallback.
- [x] Normal diagnostics do not include raw body text, body snippets, body attribute values, or body image references.

## Blocked by

None - can start immediately

## Comments

- Implemented ordered `NotificationBodyContent` segments across intake, history persistence, banner projection, and control projections. Verified with `cargo test -p canopa-notify`.
