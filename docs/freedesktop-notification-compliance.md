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
| `NotificationClosed` | Implemented | Emits reasons for expired, dismissed, sender-closed, and undefined terminal states. New no-op and policy-dropped requests return a nonzero ID and then emit reason `4`; a dropped replacement preserves the existing notification lifecycle. |
| `ActionInvoked` | Implemented | Preserves opaque, case-sensitive sender action keys. |
| `ActivationToken` | Implemented | Emits before `ActionInvoked` when a user-originated notification interaction can obtain a Wayland activation token. |

## Specification-defined capabilities

| Capability | Status | Notes |
| --- | --- | --- |
| `actions` | Advertised | Named actions and `default` activation are supported. |
| `action-icons` | Advertised | `action-icons=true` maps action keys to static icon-name references; banners render resolved local raster icons when available. |
| `body` | Advertised | Body text is shown with a plain-text fallback. |
| `body-hyperlinks` | Not implemented | Link text is preserved, but targets are not retained or actionable. |
| `body-images` | Not advertised | Body-embedded image markup may be decoded and carried before all Notify-owned body-display surfaces render it. |
| `body-markup` | Advertised | Bold, italic, underline, tolerated line breaks, and safe text fallback are supported. |
| `icon-multi` | Not implemented | Animated or multi-frame icons are not supported. |
| `icon-static` | Advertised | Raw image data, local static raster files, and static raster icon-theme names are supported. |
| `persistence` | Not advertised | Notify persists local history, but does not yet promise freedesktop persistence semantics to senders. |
| `sound` | Runtime advertised | Advertised when graphical notification sound support is enabled and the audio backend initialized successfully. Not advertised in headless mode or when sound is disabled. |

This table is the complete capability vocabulary defined by Desktop Notifications Specification 1.3. Runtime `GetCapabilities` results are tested as exact sets so compatibility or Canopa extensions cannot silently become specification claims.

## Specification-defined hints

| Hint | Status | Notes |
| --- | --- | --- |
| `action-icons` | Implemented | When true, each action key is also treated as a static icon name for presentation. |
| `category` | Implemented | Stored as notification metadata. |
| `desktop-entry` | Implemented | Preferred source for App Identity. |
| `image-data` | Implemented | Raw static image payloads are validated and rendered at runtime without persisting payload bytes. |
| `image_data`, `icon_data` | Compatibility aliases | Deprecated spellings remain decoded for older senders. |
| `image-path` | Implemented | Local static raster paths are rendered when resolvable. |
| `image_path` | Compatibility alias | The deprecated spelling remains decoded for older senders. |
| `resident` | Implemented | Action invocation keeps resident notifications active. |
| `sound-file`, `sound-name` | Implemented | Shown notifications play requested local files or themed sound names, with `sound-file` taking precedence and default sound fallback when explicit sounds are unavailable. |
| `suppress-sound` | Implemented | Suppresses default, named, and file-backed notification sounds for the request. |
| `transient` | Implemented | New transient notifications retain runtime presentation, actions, timeouts, sound, and sender lifecycle while remaining absent from persisted Notification History and the first Notification Center. A thread's initial request fixes retention across replacements. |
| `urgency` | Implemented | Maps `0`, `1`, and `2` into low, normal, and critical behavior. |
| `x`, `y` | Decoded, ignored | Wayland banner placement is daemon-owned; sender coordinates are not honored. |

## Compatibility extensions

These names are widely implemented interoperability surfaces, but they are not capabilities or hints defined by Desktop Notifications Specification 1.3.

| Extension | Kind | Status | Compatibility reason |
| --- | --- | --- | --- |
| `activation-token` | Capability | Runtime advertised | Existing Wayland clients probe this unnamespaced name. It is advertised only when the daemon can request tokens. The standard `ActivationToken` signal remains independently usable and is not made standard by this capability name. |
| `inline-reply` | Capability | Not advertised | Portal reply purposes require a keyboard-interactive response UI and are not a freedesktop 1.3 capability. |
| `value` | Hint | Implemented | The de-facto progress hint is retained because common clients send it; Notify stores and renders it as Notification Progress. |

## Canopa extensions

Canopa Notify currently advertises no Canopa-specific capabilities and decodes no Canopa-specific hints. New Canopa-only protocol names must use the `x-canopa-` namespace rather than adding another unnamespaced extension.

## Client portability guidance

- Portable freedesktop 1.3 clients should depend only on the D-Bus interface, capabilities, and hints in the specification-defined sections above.
- Clients may subscribe to the standard `ActivationToken` signal without treating the `activation-token` compatibility capability as part of freedesktop 1.3.
- Clients using `activation-token` or `value` must tolerate servers that omit or ignore them.
- `inline-reply` is portal-specific behavior, not a portable `org.freedesktop.Notifications` capability.
- Canopa-specific behavior, when introduced, will be identifiable by its `x-canopa-` name.

## Compatibility rules

- `GetCapabilities` tests must assert the exact specification-defined and extension sets for headless, graphical, sound-enabled, and activation-token-capable runtimes.
- Adding a new advertised capability requires a D-Bus-level test and at least one presentation or lifecycle test for the behavior.
- Activation-token behavior should be tested through a replaceable daemon activation-token provider, with compositor-specific integration kept outside lifecycle tests.
- Banner interactions first try a Wayland `xdg_activation_v1` token using the click's input serial and banner surface; if unavailable or empty, sender-visible publication falls back to the daemon activation-token provider.
- `activation-token` capability advertisement follows activation-token provider availability; normal `--headless` startup does not install a provider.
- `activation-token` capability advertisement is stable for the daemon lifetime; individual token requests remain best-effort.
- `sound` capability advertisement follows runtime sound support; normal `--headless` startup does not enable sound, and valid configuration reloads may change sound capability for future requests.
- Sound playback is a best-effort presentation side effect. Playback failure must not change the sender-visible `Notify` result or the persisted delivery outcome.
- The initial activation-token generation budget is a fixed 100 ms daemon-side timeout.
- Empty activation-token provider results are treated as unavailable tokens; `ActionInvoked` still proceeds without an `ActivationToken` signal.
- Decoding a hint without acting on it must be documented here as `Decoded, neutral` or `Decoded, ignored`.
