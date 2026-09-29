# Niribar menu and panel UX exploration

Status: shared direction approved; production implementation and native validation complete.
See `PRD.md` for the accepted implementation scope.

The user wants to improve niribar's look and feel, with menus and panels (layout,
opening and closing) as the first focus. These are interactive design proposals,
not changes to the running application. All preview content is sample data.

## Findings before this implementation

- `PanelAnimationConfig` defaults to 180 ms opening and 120 ms closing, and the KDL
  parser accepts both. No runtime code consumes these values. The tick handler in
  `app/animation.rs` currently advances tooltip dwell only.
- `Foreground::close_active_panel` immediately emits `RemoveWindow` and clears the
  panel runtime. A visible closing transition needs explicit presentation lifetime
  management; changing durations alone will not animate anything.
- `render_panel_menu_model` uses different resting backgrounds, borders, and radii
  for drawer and automatic presentation. Rows have an 8 px gap. Shared visual
  metrics exist, but local values remain throughout the panel renderers.
- The inspected September 23 audit screenshot shows a calendar card inside a tall
  panel with considerable unused space. It is historical evidence, not a capture
  of the user's current desktop. Current code supports both automatic sizing and
  drawers, including titled drawer chrome.

## User decision: one product, two presentations

Both drawer and popover are supported forms. The user wants them to feel like one
product. They are complementary presentations of the same panel design, rather
than competing visual directions. The earlier three-way exploration is superseded.

| Shared in both forms | Adapted to presentation |
| --- | --- |
| Palette, text hierarchy, icons and control radii | Attached drawer or detached popover frame |
| Header, close control, search, content padding and row density | Edge-filling or bounded content height |
| Hover, selection, keyboard focus and disabled treatment | Anchor position and output-boundary handling |
| Loading, error and empty-state language | Outer corners and shadow appropriate to attachment |
| Opening/closing timing, easing and reduced-motion behavior | Direction of travel follows the originating edge |

The preview now has **Drawer** and **Popover** presentations with shared markup,
styles and one motion configuration: 180 ms opening, 120 ms closing, 12 px slide
with opacity, no scale effect. This is a proposed starting point. Presentation
selectors affect only outer geometry. All content uses the same visual rules.

Both keep the bar as the content selector; no second drawer navigation is added.
Applications, Wi-Fi, sound, search, selection and volume state carry across the
comparison. This shared preview state is for evaluating the design, not a proposed
change to production session lifetimes. Escape clears a search before closing;
closing makes controls inert immediately and returns focus to the originating module.
The browser's reduced-motion preference suppresses animations. The host's design
controls adjust the shared motion configuration for both presentations.

The compact Applications popover remains an illustrative sizing proposal: current
automatic Panel Menus fill the edge span. Adopting shared styling does not by itself
authorize changing that sizing policy. Existing automatic module detail panels
already size to content. Preserve the supported sizing and drawer overlay/push-bar
contracts when implementing the shared treatment.

## Approved first implementation slice

Apply shared panel chrome, menu rows, spacing, and focus treatment to both drawer
and popover in the same implementation slice, then add bounded opening/closing
motion. Presentation-specific code should place and size the shared content rather
than choose its typography, row styles or controls. Keep theme roles shared with
Notify; keep shell lifecycle and interaction state in niribar.

Respect ADR 0008: Foreground continues to own identity, replacement, and stale
close handling. Closing should end logical interaction/provider lifetime promptly;
any retained visual used for the exit animation must be inert. Replacement or rapid
reopening must not let an old completion remove the new foreground. No resize or
exclusive-zone changes per animation frame, and no permanent idle animation tick.

Before shipping: verify all four edges, drawer overlay/push-bar modes, rapid
toggle/replacement, Escape/focus behavior, reduced motion, output removal, provider
cleanup, and scrolling with the chosen row geometry. Preserve list virtualization.
The parser already accepts zero durations. The approved implementation documents
zero as disabling the corresponding transition and corrects the diagnostic help.

The inline preview source is `menu-panel-directions.html`; `preview.html` is the
exported browser version. It is not a Rust UI or a performance test, and cannot
establish native Wayland animation smoothness.

## Initial exploration validation

- Rendered in local headless Chromium at desktop width and inspected the three
  directions in narrow columns (approximately 310 px each).
- Exercised application search, first-Escape query clearing, immediate input
  deactivation on close, rapid close/reopen, and switching to Wi-Fi and sound.
- Checked JavaScript syntax and repository whitespace. No production source changed;
  Cargo validation is not applicable to this HTML design exploration.

## Shared presentation revision validation

- Inspected both presentations with identical Wi-Fi content in Chromium.
- Verified matching computed header/content padding, row padding/radius, heading
  size and close-control width across drawer and popover.
- Verified synchronized search and selection, first-Escape clearing, immediate
  deactivation on close, and visibility/interactivity after rapid reopening.
- Regenerated `preview.html` at the existing browser path and checked JavaScript
  syntax. These checks cover the preview; native rendering is still unmodified.

## Production implementation

Shared chrome and spacing are implemented in the Rust UI. Per-Bar animation
settings now control directional slide/fade, with Foreground retaining only an
inert outgoing presentation after logical close. See ADR 0014 for the lifetime,
input mapping, capture bound and provider replacement decisions. Browser artifacts
remain design evidence; production validation is recorded in `PRD.md`.
