# Keep body-images unadvertised during partial rollout

Status: implemented
Type: AFK

## Parent

[Body-Embedded Notification Images](../PRD.md)

## What to build

Keep freedesktop `body-images` capability discovery conservative while Canopa Notify can parse, persist, render banners, and export panel body segments before the actual **Notification Center** host is known to render them. The D-Bus API should continue advertising existing supported capabilities, especially `body-markup`, without claiming `body-images` prematurely.

## Acceptance criteria

- [x] `GetCapabilities` continues to advertise `body-markup` when styled text segments render correctly.
- [x] `GetCapabilities` does not advertise `body-images` during this partial rollout.
- [x] Compliance documentation says body images may be decoded and carried before all Notify-owned body-display surfaces render them.
- [x] Capability tests assert both the retained `body-markup` advertisement and the intentionally missing `body-images` advertisement.
- [x] D-Bus notification receipt still accepts body-image markup according to the implemented parsing and degradation rules.

## Blocked by

- [01-parse-persist-and-project-ordered-body-segments.md](01-parse-persist-and-project-ordered-body-segments.md)

## Comments

- Verified capability discipline remains conservative: `body-markup` stays advertised, `body-images` stays unadvertised, and D-Bus `Notify` still accepts body image markup into structured body content. Verified with `cargo test -p canopa-notify`.
