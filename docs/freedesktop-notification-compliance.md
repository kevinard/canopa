# Freedesktop notification compliance

Source: Desktop Notifications Specification 1.3, latest upstream publication dated 2024-08-18.

Canopa Notify should advertise a freedesktop capability only when the sender-visible D-Bus behavior and Notify-owned presentation behavior are both implemented. Unsupported hints are ignored or decoded as request facts according to the spec's optional hint model.

Completion is still in progress. The following status tables describe implemented behavior, not a claim that every selected presentation requirement has passed. The [execution tracker](../.scratch/freedesktop-completion-review/IMPLEMENTATION.md) records remaining work; the [graphical checklist](notification-center-verification.md) has not yet been executed for the Niribar port.

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

## Requirement-to-evidence matrix

The protocol baseline is the versioned [Desktop Notifications 1.3 protocol](https://specifications.freedesktop.org/notification/1.3/protocol.html); image layout follows [icons and images](https://specifications.freedesktop.org/notification/1.3/icons-and-images.html). Automated executable tests run on a private bus through `bash scripts/test.sh`.

| Requirement | Obligation | Implementation owner | Evidence / remaining verification |
| --- | --- | --- | --- |
| Four D-Bus methods, reply types and server information | Core protocol | D-Bus adapter | `tests/dbus_receipt.rs` exercises Notify/Close; binary tests check exact GetCapabilities sets. Dedicated introspection/GetServerInformation assertions remain open in W01. |
| Nonzero live IDs, replacement and invalidated closures | Core protocol | D-Bus registry + Lifecycle | `dev_dbus_closed_wire_id_cannot_replace_an_unrelated_history_thread`, `concurrent_fixed_wire_token_requests_revise_one_live_thread`, `tests/replacements.rs`. Typed registry extraction and wrap/retirement coverage remain W03. |
| Required closure/action signal delivery and ordering | Core protocol / advertised actions | Runtime + sender publisher | Binary `dbus_bulk_clear_publishes_all_130_closures`, `notification_server_emits_activation_token_before_action_invoked`; D-Bus action-before-close test. Transport failure and renderer recovery remain W05. |
| Expiration, resident and transient semantics | Protocol with explicit local timeout policy | Lifecycle + Runtime | `tests/lifecycle.rs`, `tests/presentation.rs`, `tests/control_api.rs`; display-based timing and transient-center exclusion documented below. |
| Static raw images and legacy spellings | Advertised static profile | Request decoder + Intake | `tests/notification_receipt.rs`, decoder tests: signed channel validation, omitted final-row padding, and source precedence. Theme/file safety work remains W09. |
| Actions, markup and body images on real surfaces | Optional capabilities, required when advertised | Banner presenter + Niribar host | Banner presentation tests and Niribar `canopa_provider`/`provider_protocol` tests; host v3 port now renders sections, controls, styled bodies and bounded images. All-transient action reachability and actual graphical evidence remain W10/W11. |
| Hyperlinks and persistence | Optional; not advertised | Intake, presentation, retention | Explicitly unfinished W12/W13; retaining closed history alone does not satisfy persistence. |
| Durable paths, installed configuration and reload | Local product/operational contract | Startup + Configuration + Control | `tests/control_spine.rs`, `installed_startup_loads_xdg_configuration_before_accepting_notifications`, failed-install sound test; socket partial-clear regression verifies committed counts. |
| Atomic storage and safe upgrades | Local reliability contract | Notification History adapter | Twelve SQLite adapter tests, future-schema rejection and partial-clear history tests. Interrupted upgrades, large backfills and additional startup failure fixtures remain W02/W06. |

The static profile excludes `icon-multi`; Wayland placement may ignore `x`/`y`. No native portal-v2 backend is claimed. See the existing [portal interoperability notes](xdg-notification-portal-v2.md) for negotiated bridge limitations.

## Expiration semantics

[ADR 0028](adr/0028-notification-timeouts-begin-with-banner-display.md) defines the implemented behavior.

- Positive and server-default expiration values begin counting only when the current **Notification Thread** revision first becomes visible in a **Notification Banner**. A zero value never expires.
- Overflow notifications, **Silenced Notifications**, requests accepted under **Do Not Disturb**, and **Headless Mode** requests that never display a banner remain active without automatic expiration. This deliberately permits a finite **Sender Notification Lifecycle** to remain open until user or sender action.
- Once a **Notification Banner** has displayed, its deadline continues if a critical notification displaces it. Notify does not later promote an overflow notification merely because a banner slot opens.
- A replacement that revises a **Notification Thread** cancels the prior revision's timeout and begins a fresh timeout only if the replacement revision becomes visible. A dropped replacement preserves the existing thread and timeout.
- When a displayed notification reaches its deadline, Notify invalidates the active sender ID and emits `NotificationClosed` with reason `1` while retaining an **Expired Notification** in **Notification History**.

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
| `category` | Implemented | Valid standard and vendor-prefixed values are normalized into Notification Category metadata and may drive category-specific policy; malformed values are ignored. |
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
