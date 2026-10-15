# Module Content Rotation and Focused Window Icons Design

**Date:** 2026-06-08
**Author:** Kevin

## Summary

Add shared **Content Rotation** for modules that support **Format Templates**, and improve `focused-window` so it can show the focused application's icon and a rotated full title in vertical bars.

`content-rotation` is a module-level setting. It changes the presentation direction of formatted module content without changing the **Bar** **Orientation**, **Edge**, placement, interaction behavior, or tooltip behavior.

## Goals

- Let formatted module content rotate on vertical bars.
- Make `focused-window` default to rotated icon-plus-title content on vertical bars.
- Resolve `focused-window` `{icon}` from the focused **Compositor Window** application identity when possible.
- Keep **Format Template** placement as the control for whether an icon is visible.
- Preserve per-segment format styles when content is rotated.
- Keep per-output focused-window behavior: each **Bar Surface** shows the active window for its **Output**.

## Non-Goals

- Do not rotate tooltips.
- Do not rotate built-in fallback rendering for modules that do not have a configured or default **Format Template**.
- Do not change **Unavailable Module** or **Empty Module** behavior; they render no visible content, leave no gap, and have nothing to rotate.
- Do not make non-focused-window `{icon}` placeholders image-capable.
- Do not add scrolling or marquee behavior for long rotated content.
- Do not log per-window application icon resolution failures.
- Do not add boolean rotation aliases.

## Config Syntax

Every non-tray module that supports `format`, `expanded-format`, and `horizontal-format` also accepts:

```kdl
content-rotation "none"
```

Accepted values:

- `"none"`
- `"auto"`
- `"clockwise"`
- `"counter-clockwise"`

There are no aliases. In particular, `"counterclockwise"` is invalid.

`tray` does not support `content-rotation`.

In Rust config structs, `content_rotation` is a sibling of `format` on each
formatted module config. The shared enum belongs near `FormatConfig` because it
describes formatted content presentation after template selection.

## Shared Rotation Semantics

The shared default is:

```kdl
content-rotation "none"
```

Outside `focused-window`, this feature is inert by default. Existing module
output stays unchanged unless the user configures `content-rotation` and the
bar render path selects a **Format Template**.

`auto` is edge-aware:

- left **Edge**: `counter-clockwise`
- right **Edge**: `clockwise`
- top/bottom **Edge**: no rotation

Explicit `clockwise` and `counter-clockwise` are absolute instructions and apply on horizontal bars too. `none` always disables rotation.

Template selection remains based on **Orientation** before rotation is applied:

- vertical bars use `format`
- horizontal bars use `horizontal-format`, falling back through existing format rules

Then `content-rotation` applies to the resulting formatted content group.
Rotation does not introduce a new render mode and does not change the existing
format fallback rules. The goal is only to rotate the result those rules already
produce.

The bar render path should pass enough context for modules to resolve `auto`,
preferably the **Edge** or a small bar-render context that includes **Edge** and
**Orientation**, rather than only the current horizontal boolean.

## Format Template Interaction

`content-rotation` affects **Format Template** output only. For modules other than `focused-window`, if no relevant format template is configured, the built-in fallback rendering is not rotated.

Rotated content preserves normal template behavior:

- placeholder substitution
- multiple segments
- newlines inside text segments
- segment spacing
- per-segment color, size, and weight
- state-specific segment styles

The formatted segment group rotates as one visual unit. Individual segments, including icons, are not kept upright independently.

Rotated content may request a larger slot along the bar's long axis. On a vertical bar, the unrotated formatted width becomes the rotated height. The cross-axis remains constrained by the bar content width. If content still exceeds available zone space, it is clipped at the zone boundary.

Pointer hit boxes use the final visible module rectangle. Keyboard navigation
order, module ordering, and zone ordering do not change.

Implementation should centralize rotated formatted content in a small shared
helper or widget. Module code should pass resolved segments, icon metadata,
rotation direction, and style into that helper instead of hand-rolling rotation
geometry in each module.

## Focused Window Defaults

`focused-window` has kind-level defaults, independent of the configured module name:

```kdl
format "{icon}" "{title}"
content-rotation "auto"
fallback-icon "□"
```

The bundled default config should explicitly match these Rust defaults.
`focused-window` does not define default `expanded-format` or
`horizontal-format`; existing format fallback rules apply from the default
`format`.

When there is no active/focused **Compositor Window** for a **Bar Surface**'s **Output**, `focused-window` preserves existing behavior: render nothing and reserve no gap.
An active **Compositor Window** with no title is still visible state. It may
render its **Application Icon** with an empty title segment.
Missing and empty-string titles are both treated as an empty title value when
a window is present.

The default tooltip remains `"{title}"`.
For an active window with no title, the default tooltip renders no content and
therefore should not show. A custom tooltip such as `"{app-id}"` may show if it
renders non-empty text.

## Focused Window Placeholders

`focused-window` supports:

- `{icon}`
- `{value}`: title
- `{title}`: title
- `{app-id}`: compositor-provided application identity, or an empty string

`{icon}` uses the resolved **Application Icon** when it can be rendered as an image. It uses `fallback-icon` as text when image rendering is not available or not appropriate for the template shape.

## Application Icon Resolution

`focused-window` state should preserve both:

- whether an active **Compositor Window** is present
- title, which may be missing even when a window is present
- raw `app_id: Option<String>`
- resolved icon information for rendering

Application icon resolution uses a two-step lookup:

1. Match `app_id` against discovered desktop entries using conservative normalization:
   exact desktop id (`firefox.desktop`), exact desktop id stem (`firefox`), then reverse-DNS suffix (`org.mozilla.firefox` to `firefox.desktop`). If none matches, accept a unique ASCII case-insensitive match against those identifiers, covering XWayland classes such as `Chatgpt` with `chatgpt.desktop`. Ambiguous case-insensitive matches are rejected.
2. If no desktop entry matches, try `app_id` directly through the icon theme resolver.

Resolution does not fuzzy match display names, comments, or keywords.
Desktop-entry discovery respects the same current-desktop filtering used by
installed-app menus. The direct icon-theme fallback still runs when no filtered
desktop entry matches.

If resolution fails, rendering falls back to `fallback-icon`.

Resolution should be lazy and process-lifetime cached. The desktop-entry index
is built on first focused-window icon lookup, not at startup, and ordinary view
rendering must not repeatedly scan desktop entries or icon theme paths.
Cache invalidation for desktop file or icon theme changes is out of scope;
changes can take effect after Niribar restarts.

Failed resolution is not logged by default. If diagnostics are added later, they should avoid per-window user content and follow ADR-0006's runtime diagnostics policy.

## Image Icon Rendering

Image-capable `{icon}` is scoped to `focused-window`.

For `focused-window`, a real image/SVG **Application Icon** renders only when `{icon}` is its own format segment:

```kdl
format "{icon}" "{title}"
```

An own icon segment means the segment template is exactly `{icon}` with no
surrounding whitespace or literal text. Styled `segment "{icon}"` follows the
same rule.

If `{icon}` appears inside mixed text, the segment remains text-only:

```kdl
format "{icon}: {title}"
format " {icon} " "{title}"
```

In mixed text and tooltips, `{icon}` renders as `fallback-icon`.

Other modules continue to render `{icon}` as text/glyph content.

When a focused-window `{icon}` segment renders as an image, the segment `size`
style controls the image size when present. Otherwise the image uses the render
mode's normal icon/text fallback size. Segment color and weight styles do not
recolor bitmap or SVG application icons.

## Per-Output Behavior

The current per-output focused-window behavior remains:

- For a **Bar Surface** with an **Output**, show that output's active **Compositor Window**.
- For the globally focused output, this is the globally focused window.
- For an output-less surface, fall back to the globally focused window.

This applies to title, `app-id`, and **Application Icon**.

## Workspace Items

`content-rotation` configured on the `workspaces` parent module is inherited by generated workspace item modules.

Example:

```kdl
module "workspaces" {
    format "{icon}" "{label}"
    content-rotation "auto"
}
```

Each runtime workspace item applies that rotation policy to its formatted bar content.

## Tooltips

Tooltips never rotate.

Tooltip templates use the same placeholder values as the module, but remain text-only. For `focused-window`, tooltip `{icon}` uses `fallback-icon`, not an image.

## Panels

Panels never rotate module content because of `content-rotation`. The setting
applies only to modules rendered on a **Bar Surface**, not to
`render_panel_with_style` output or panel module layouts.

## Default Config Documentation

The default config should:

- document `content-rotation` once near the shared format-template comments
- explicitly show the focused-window default `format`, `content-rotation`, and `fallback-icon`
- avoid repeating full rotation explanations in every module block

## Testing

Parser tests:

- parse `content-rotation` for every non-tray formatted module
- reject `content-rotation` for tray
- reject unknown rotation values with a diagnostic at `modules[N].<type>.content-rotation`
  whose help lists `none`, `auto`, `clockwise`, and `counter-clockwise`
- reject aliases such as `"counterclockwise"`
- parse focused-window `fallback-icon`
- parse focused-window `{app-id}` in format and tooltip templates

Config/default tests:

- focused-window defaults to `format "{icon}" "{title}"`
- focused-window defaults to `content-rotation "auto"`
- focused-window defaults to fallback icon `"□"`
- focused-window rejects an empty `fallback-icon`
- custom modules with `type "focused-window"` receive the same defaults
- bundled default config matches Rust defaults

Rendering/model tests:

- `auto` maps left edge to counter-clockwise and right edge to clockwise
- `auto` is no rotation on top/bottom edges
- explicit rotation applies on horizontal bars
- rotation is applied after format mode selection
- rotated content preserves styled segments
- non-focused-window built-in fallback rendering is not rotated without a format template
- workspace items inherit parent `content-rotation`
- no active window still renders no focused-window module content

Focused-window icon tests:

- state carries title, app id, and resolved icon information
- per-output title/app-id/icon selection follows existing per-output title behavior
- `{app-id}` renders empty when missing
- own-segment `{icon}` can render an image icon when resolved
- mixed-text `{icon}` renders `fallback-icon`
- tooltip `{icon}` renders `fallback-icon`
- unresolved application icon uses `fallback-icon`
