# Show a real minimal Notification Banner

Status: implemented
Type: AFK

## Parent

[Canopa Notify PRD](../PRD.md)

## What to build

Render the first real daemon-owned Notification Banner for an eligible Active Notification. The banner should use iced/layer-shell, display minimal readable notification content, avoid stealing keyboard focus, appear on the focused output when possible with fallback behavior, and support basic activation/dismissal.

## Acceptance criteria

- [x] Receiving an allowed notification creates a visible minimal Notification Banner outside Headless Mode.
- [x] The banner presents Notification Summary and plain Notification Body text when available.
- [x] Banner placement uses the focused output when available and a configured/default output fallback otherwise.
- [x] The banner does not steal keyboard focus.
- [x] Activating a banner invokes the Default Notification Action when present and otherwise dismisses the notification.
- [x] Tests or smoke checks cover presentation coordinator behavior in Headless Mode, with a graphical smoke path documented for real iced/layer-shell rendering.

## Blocked by

- [02-receive-store-default-allowed-notification.md](02-receive-store-default-allowed-notification.md)

## Comments

### 2026-06-19 Diagnose

The issue was marked implemented, but the code only maintained `presented` and `visible_banner` SQLite state. There was no daemon presentation boundary, no non-headless daemon options in service mode, and no iced/layer-shell surface code.

This diagnosis added a tested presentation coordinator seam and fixed service-mode option selection so non-headless notifications are delivered to a `BannerPresenter`. The remaining work for this issue is still the real Wayland/iced layer-shell renderer that turns `BannerView` updates into visible, non-focus-stealing Notification Banner surfaces.

### 2026-06-19 Implementation

Implemented a real iced/layer-shell `IcedLayerShellBannerPresenter` for service mode. It runs a same-process layer-shell UI thread, renders minimal readable banner content from `BannerView`, uses active/focused-output layer-shell placement with fallback behavior, sets keyboard interactivity to none, and forwards banner activation/dismissal events back to the daemon.

Added tests for the banner surface contract, iced/layer-shell settings mapping, Headless Mode presentation behavior, and banner activation behavior. Verified with `rtk proxy cargo test -p canopa-notify`. The real graphical smoke path remains documented in `docs/service-mode.md`.
