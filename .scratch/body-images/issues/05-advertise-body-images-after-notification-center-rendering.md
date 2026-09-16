# Advertise body-images after Notification Center rendering

Status: done
Type: HITL

## Parent

[Body-Embedded Notification Images](../PRD.md)

## What to build

After the Niribar-hosted **Notification Center** can consume panel protocol v4 rich body content and render **Body-Embedded Notification Images**, update Canopa Notify's freedesktop capability advertisement and compliance status to claim `body-images`. The original ticket requested human verification before advertisement. The authorized completion implementation supplies captured real-host, native AT-SPI and Orca evidence before enabling the capability; no separate human sign-off is claimed.

## Acceptance criteria

- [x] Niribar or the active Notification Center host is verified to render panel protocol v4 body image segments.
- [x] `GetCapabilities` advertises `body-images` only after the body-display path is verified end to end.
- [x] Compliance documentation marks `body-images` as advertised or implemented with notes matching the final behavior.
- [x] D-Bus-level capability tests cover the newly advertised `body-images` capability.
- [x] Presentation tests or integration evidence cover both banner and Notification Center body image rendering.

## Blocked by

- [02-render-body-embedded-images-in-banners.md](02-render-body-embedded-images-in-banners.md)
- [03-export-rich-body-content-through-panel-protocol-v3.md](03-export-rich-body-content-through-panel-protocol-v3.md)
- External Niribar or Notification Center host support for panel protocol v4 rich body content

## Comments

2026-09-16: Implemented in the paired Notify/Niribar completion changes. [Desktop acceptance](../../../docs/notification-desktop-acceptance.md) records real pixels, input, portal opening and Orca speech; [release verification](../../../docs/notification-release-verification.md) records the full/core/host suites and exact capability checks. Current host transport is Canopa v4. The compatible host source is applied; the live desktop configuration/service was not changed.
