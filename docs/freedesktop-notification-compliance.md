# Freedesktop notification compliance

Source: Desktop Notifications Specification 1.3, latest upstream publication dated 2024-08-18.

Canopa Notify should advertise a freedesktop capability only when the sender-visible D-Bus behavior and Notify-owned presentation behavior are both implemented. Unsupported hints are ignored or decoded as request facts according to the spec's optional hint model.

## D-Bus interface

| Spec surface | Status | Notes |
| --- | --- | --- |
| `org.freedesktop.Notifications.Notify` | Implemented | Maps request values through Notification Server Request decoding into Notification Intake. |
| `CloseNotification` | Implemented | Closes active notifications and emits `NotificationClosed` with reason `3`. |
| `GetCapabilities` | Implemented | Advertises only the capabilities listed below. |
| `GetServerInformation` | Implemented | Reports spec version `1.3`. |
| `NotificationClosed` | Implemented | Emits reasons for expired, dismissed, closed, and unknown terminal states. |
| `ActionInvoked` | Implemented | Preserves opaque, case-sensitive sender action keys. |
| `ActivationToken` | Planned | Requires Wayland activation-token integration before advertising `activation-token`. |

## Capabilities

| Capability | Status | Notes |
| --- | --- | --- |
| `actions` | Advertised | Named actions and `default` activation are supported. |
| `action-icons` | Advertised | `action-icons=true` maps action keys to static icon-name references; banners render resolved local raster icons when available. |
| `body` | Advertised | Body text is shown with a plain-text fallback. |
| `body-markup` | Advertised | Bold, italic, underline, tolerated line breaks, and safe text fallback are supported. |
| `icon-static` | Advertised | Raw image data, local static raster files, and static raster icon-theme names are supported. |
| `body-hyperlinks` | Not implemented | Link text is preserved, but targets are not retained or actionable. |
| `body-images` | Not implemented | Body-embedded image markup may contribute alt text only. |
| `icon-multi` | Not implemented | Animated or multi-frame icons are not supported. |
| `persistence` | Not advertised | Notify persists local history, but does not yet promise freedesktop persistence semantics to senders. |
| `sound` | Not advertised | Sound hints are decoded but no playback adapter is present. |
| `activation-token` | Not advertised | See `ActivationToken` above. |
| `inline-reply` | Not implemented | Requires keyboard-interactive reply UI and sender reply signaling semantics. |

## Standard hints

| Hint | Status | Notes |
| --- | --- | --- |
| `action-icons` | Implemented | When true, each action key is also treated as a static icon name for presentation. |
| `category` | Implemented | Stored as notification metadata. |
| `desktop-entry` | Implemented | Preferred source for App Identity. |
| `image-data`, `image_data`, `icon_data` | Implemented | Raw static image payloads are validated and rendered at runtime without persisting payload bytes. |
| `image-path`, `image_path` | Implemented | Local static raster paths are rendered when resolvable. |
| `resident` | Implemented | Action invocation keeps resident notifications active. |
| `sound-file`, `sound-name` | Decoded, neutral | Request facts are decoded, but sound is not played until a sound adapter exists. |
| `suppress-sound` | Decoded, neutral | Stored on request input for future sound adapter behavior. |
| `transient` | Decoded, neutral | Does not yet change notification history retention. |
| `urgency` | Implemented | Maps `0`, `1`, and `2` into low, normal, and critical behavior. |
| `value` | Implemented | Stored and rendered as notification progress. |
| `x`, `y` | Decoded, ignored | Wayland banner placement is daemon-owned; sender coordinates are not honored. |

## Compatibility rules

- `GetCapabilities` tests must assert both advertised and intentionally unadvertised capabilities.
- Adding a new advertised capability requires a D-Bus-level test and at least one presentation or lifecycle test for the behavior.
- Decoding a hint without acting on it must be documented here as `Decoded, neutral` or `Decoded, ignored`.
