# Advertise body-images after Notification Center rendering

Status: ready-for-human
Type: HITL

## Parent

[Body-Embedded Notification Images](../PRD.md)

## What to build

After the Niribar-hosted **Notification Center** can consume panel protocol v3 rich body content and render **Body-Embedded Notification Images**, update Canopa Notify's freedesktop capability advertisement and compliance status to claim `body-images`. This slice requires human verification of the external Notification Center host behavior before the capability is exposed to senders.

## Acceptance criteria

- [ ] Niribar or the active Notification Center host is verified to render panel protocol v3 body image segments.
- [ ] `GetCapabilities` advertises `body-images` only after the body-display path is verified end to end.
- [ ] Compliance documentation marks `body-images` as advertised or implemented with notes matching the final behavior.
- [ ] D-Bus-level capability tests cover the newly advertised `body-images` capability.
- [ ] Presentation tests or integration evidence cover both banner and Notification Center body image rendering.

## Blocked by

- [02-render-body-embedded-images-in-banners.md](02-render-body-embedded-images-in-banners.md)
- [03-export-rich-body-content-through-panel-protocol-v3.md](03-export-rich-body-content-through-panel-protocol-v3.md)
- External Niribar or Notification Center host support for panel protocol v3 rich body content
