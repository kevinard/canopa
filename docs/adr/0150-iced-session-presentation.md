# 0150. Host Iced session presentation in the native Wayland loops

Status: accepted

## Context

Lock, login, authorization, PIN and wallpaper-picker interfaces shared a raster
presentation with manually dispatched click rectangles. Adding hover, proper
press/release behavior, scrolling and text selection duplicated widget machinery
already available in the rest of Canopa. Their native hosts also own carefully
bounded authentication, session-lock confirmation and suspend-readiness lifetimes.

## Decision

Use Iced widget trees and its standard wgpu compositor with tiny-skia fallback
inside the existing SCTK/calloop hosts. Share the application button treatments
through `canopa-ui`. Keep separate widget state for each output and invalidate
pending interactions when their prompt, generation, controls or enabled state
changes. Retire caches with their output or
dialog. Present directly to native surfaces without GPU readback, and retain each
native role and connection until its graphics surface is destroyed. Cache prepared
background image handles across pointer redraws, consuming the RGB source into a
single shared RGBA allocation. Keep rendering on demand, driven
by native input, model updates and coalesced graphics notifications, without an
animation frame timer. Headless image previews explicitly use tiny-skia. The
wallpaper daemon continues submitting static shared-memory images without
initializing an Iced compositor. Honor Iced’s `ICED_BACKEND` override; default
initialization tries wgpu then tiny-skia. Compositors retain a display-only
connection handle; only graphics surfaces retain native window/lock roles.
Throttle each surface independently with Wayland frame callbacks and disable
renderer vsync waits. Sleeping outputs coalesce changes without blocking other
outputs or authentication. Reconfigure lost/outdated surfaces once; timeouts retry
on a bounded timer and persistent GPU errors downgrade only that output to
software. Contain panics from the pinned graphics backend's constructors and
configuration at that boundary, including surface creation during lock hotplug.
Every native client keeps its event loop and authentication request alive through
graphics initialization/presentation failures. Deferred outputs re-arm their
retry deadline after earlier coalesced wakeups. Presentation errors never release
the compositor's lock.

Keep an owned widget tree per surface, dispatch pointer events directly to it,
and redraw only for widget/model changes. Bound icon, avatar and response-bitmap
caches; keys for response pixels contain mutation/selection revisions, never
response strings. Hosts advance explicit scene revisions after model changes;
clock/weather observation advances them only for changed displayed values.
Presentation never serializes settings or reads weather files. Cache cursor shapes
per pointer, refreshing on enter and shape changes. Strip controls and limit each
public label to 256 characters; only explicitly planned line arrays create multiple
text lines. Fit and clip text to its planned bounds, retain monospace fingerprints
with identity captions, and mirror clock parts in RTL.

Keep response storage in `Secret`. A dedicated Iced input widget handles pointer
selection through grapheme-boundary positions and renders response pixels from a
transient shaping buffer. It does not use Iced's string-valued password widget,
clipboard, keyboard text events or text messages. Each controller validates
selection against its current authentication phase.

Keep the existing ext-session-lock owner and its compositor-confirmed readiness,
PAM worker isolation and authenticated unlock paths. Do not adopt the separate
`iced_sessionlock` event loop or map ordinary window-close actions to unlocking.
The wallpaper daemon and idle supervisor retain their existing responsibilities.

## Consequences

All interactive session interfaces share Iced controls and appearance with the
applications. Native surface and authentication lifecycle tests remain applicable.
The session host must explicitly drive Iced input, redraw and cache retirement;
secret entry remains a dedicated widget because masking a normal text field does
not preserve Canopa's response-storage guarantees.
