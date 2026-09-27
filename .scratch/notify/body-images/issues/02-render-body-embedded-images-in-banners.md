# Render Body-Embedded Notification Images in banners

Status: implemented
Type: AFK

## Parent

[Body-Embedded Notification Images](../PRD.md)

## What to build

Render **Body-Embedded Notification Images** as ordered body content in **Notification Banners** while preserving the existing notification-level visual area for **Notification App Icon** and **Notification Image**. Banner rendering should load supported local static raster files, scale them to a presentation-owned bound, and fall back to the segment's alternative text when the image cannot be rendered.

## Acceptance criteria

- [x] A banner can render ordered text and body image segments from one notification body.
- [x] Body-embedded images do not participate in notification-level visual priority and do not replace the app icon or notification image slot.
- [x] Supported local PNG and JPEG body images render from absolute paths and `file://` URIs.
- [x] Safely decodable oversized natural dimensions are scaled to fit the banner body area.
- [x] Resource-unsafe files, missing files, unsupported formats, and malformed image references degrade to `alt` text when available.
- [x] A rendered body image segment does not also show its own `alt` text in the banner.
- [x] Body image-only notifications without `alt` can still produce a visible image banner when the local file renders.
- [x] Replacement notifications clear previous body image segments when the replacement body omits them.
- [x] Existing banner tests for styled body text, notification-level images, and app icons continue to pass.

## Blocked by

- [01-parse-persist-and-project-ordered-body-segments.md](01-parse-persist-and-project-ordered-body-segments.md)

## Comments

- Implemented banner-rendered body segments with bounded body image rendering and `alt` fallback for missing or resource-unsafe local files. Verified with `cargo test -p canopa-notify`.
