# Body-Embedded Notification Images

Status: done

## Problem Statement

Canopa Notify already supports freedesktop notification-level image hints and conservative **Notification Body Markup**, but it does not preserve or render freedesktop `body-images`: images referenced from `<img src="...">` inside the notification body. Senders that rely on body-embedded visual content currently lose that visual content, with only `alt` text retained.

## Solution

Canopa Notify will interpret **Notification Body Markup** into ordered **Notification Body Segments**. Text emphasis, line breaks, and **Body-Embedded Notification Images** will be preserved in sender order while the plain **Notification Body** remains the fallback for text-only surfaces and diagnostics. Body image references will be local-file-only, resource-bounded, retained as references in **Notification History**, rendered by Notify-owned body-display surfaces where supported, and excluded from freedesktop `body-images` advertisement until the full presentation path is available.

## User Stories

1. As an application sender, I want `<img src="file:///...">` inside a notification body to be preserved, so that visual body content is not silently discarded.
2. As an application sender, I want body text and body images to remain in sender order, so that the notification message keeps its intended meaning.
3. As an application sender, I want `alt` text to remain available as a fallback, so that text-only surfaces still show useful content.
4. As a user, I want body-embedded images to render in notification banners, so that image-rich notifications are understandable at a glance.
5. As a user, I want body-embedded images to be available in the **Notification Center**, so that I can inspect image-rich notification history after the banner is gone.
6. As a user, I want body-embedded images to be bounded to the surface layout, so that large images do not overwhelm banners or the **Notification Center**.
7. As a user, I want oversized but safely decodable images to be scaled instead of dropped, so that high-resolution screenshots can still be shown.
8. As a user, I want unsupported or missing body images to degrade to `alt` text, so that notification content remains readable.
9. As a user, I do not want invented placeholder text for images without `alt`, so that text-only output does not claim sender content that was never provided.
10. As a user, I want body image references retained in **Notification History**, so that historical notifications can still render images when the local file remains available.
11. As a user, I want replacement notifications to show only the current revision's body images, so that stale images are not inherited accidentally.
12. As an operator, I want diagnostic JSON to expose structured body content, so that tests and debugging can verify body images without scraping presentation text.
13. As an operator, I want normal logs to avoid body image paths and body snippets, so that private local paths and notification content are not leaked.
14. As a protocol client, I want Canopa Notify to advertise `body-images` only when body images render end to end, so that capability discovery remains truthful.
15. As a maintainer, I want existing `body-markup` behavior preserved through the refactor, so that bold, italic, underline, and line breaks do not regress.
16. As a maintainer, I want the panel protocol to carry generic rich body content, so that Niribar can render body images without learning freedesktop notification markup.
17. As a maintainer, I want the persistence migration to recover body segments from **Raw Notification Body**, so that existing developer databases are upgraded consistently.
18. As a maintainer, I want remote URLs, icon names, relative paths, SVG, and animated image formats out of the first scope, so that the feature stays bounded and safe.

## Implementation Decisions

- Use **Notification Body Segment** as the canonical domain model for interpreted body presentation.
- Replace domain-facing text-only body markup fields with ordered body segments while keeping the plain **Notification Body** fallback.
- Preserve existing text emphasis and line-break behavior as text body segments.
- Treat **Body-Embedded Notification Images** as body content, not notification-level **Notification Images**.
- Resolve body image `src` values only from absolute local paths and `file://` URIs.
- Do not resolve body image `src` values as remote URIs, relative paths, or icon-theme names.
- Ignore body image attributes other than `src` and `alt` in the first scope.
- Let `alt` contribute to the plain **Notification Body** fallback.
- Do not invent placeholder fallback text for a body image without `alt`.
- Let a parseable local body image make an otherwise blank notification meaningful.
- Do not reject a notification because a body image is missing, unsupported, malformed, or unrenderable.
- Retain body image references in **Notification History**, but do not copy image bytes into history.
- Replace the normal persisted interpreted-body column with a body-segments column; unreleased developer databases may be migrated from **Raw Notification Body**.
- Keep replacement behavior revision-local: omitted body images are cleared when a replacement omits them.
- Render common static raster image formats first; vector and animated formats are outside the initial scope.
- Scale safely decodable images to fit each body-display surface rather than rejecting based on natural dimensions alone.
- Keep resource-unsafe files and unsupported formats on the degradation path.
- Advance the shared panel protocol when generic rich body content is exposed.
- Keep freedesktop `body-images` unadvertised until both Notify-owned exported body content and the actual **Notification Center** host can render it.
- Preserve the existing advertised `body-markup` capability by keeping styled text semantics working.

## Testing Decisions

- Test external behavior at the highest available seam instead of parser internals wherever practical.
- Intake tests should cover ordered segment interpretation, local reference acceptance, remote and malformed reference degradation, `alt` fallback, no-op acceptance, and preservation of existing text markup semantics.
- Persistence tests should cover new body segment storage, migration or backfill from **Raw Notification Body**, restart reads, and no raw image payload persistence.
- Daemon receipt and control tests should cover diagnostic JSON exposing structured body content while human-readable output remains plain fallback.
- Presentation tests should cover banner projection and rendering behavior for local raster body images, scaling bounds, missing files, and `alt` fallback.
- Panel provider tests should cover protocol version advancement and rich body content in **Notification Center Projection** output.
- D-Bus capability tests should prove `body-markup` remains advertised and `body-images` remains unadvertised until the complete body-display path is available.
- Redaction tests should ensure normal diagnostics do not include **Raw Notification Body**, body snippets, attribute values, or body image references.

## Out of Scope

- Remote body image fetching.
- Relative body image path resolution.
- Icon-theme lookup for body image `src`.
- SVG, animated, or multi-frame body images.
- Hyperlink activation.
- Interactive body images or opening local image files.
- Sender-controlled width, height, or layout attributes.
- Inline replies.
- Advertising freedesktop `body-images` before Niribar can render the new panel protocol body content.

## Further Notes

ADR 0026 records the ordered body segment and panel protocol decision. The compliance matrix should remain conservative during intermediate implementation: body images may be decoded and carried before they are advertised.

## Completion record

2026-09-16: Ordered body images, fallback and advertisement are verified on banners and the compatible Canopa-v4 Niribar Center. See [desktop acceptance](../../../docs/notify/notification-desktop-acceptance.md) and [release verification](../../../docs/notify/notification-release-verification.md). The v3 references above describe the original delivery stages; v4 preserves those image segments and adds generic links.
