# External Panel Providers

An **External Panel Provider** is an out-of-process program that owns domain logic for a rich panel while Niribar owns the visible panel window, placement, styling, focus, sizing, scrolling, and input routing.

V1 providers are panel-only. They cannot create provider-owned menus or tooltips, and provider content cannot ask Niribar to run configured commands.

Content replacements redraw the existing panel window, including background
network updates and local search or text edits. They do not close and reopen
the panel. Input state stays with the same control identity; a replacement
button or text field cannot inherit a held click or keyboard focus.

The Wi-Fi provider requirements in this document define protocol version `1`; earlier experimental request/response-only behavior is not a separate compatibility target.

## Configuration

Declare globally named **Configured Panel Providers** with the same command grammar used elsewhere:

```kdl
panel-providers {
    provider "wifi" {
        command {
            argv "niribar-wifi-provider"
        }
    }
}
```

Open a provider from any panel action with `open-provider`.

Clicking the same module again dismisses its provider panel. This also cancels a
provider that is still opening; a late handshake cannot reopen the dismissed
panel. Activating a different module or a different bar surface opens that
origin's panel. This repeat-click behavior also applies to module panel and menu
actions. Direct panel actions retain their explicit open/toggle semantics.

For example:

```kdl
modules {
    module "network" {
        actions {
            click { open-provider "wifi" }
        }
    }
}
```

Provider commands obey `command-policy`. Shell-form commands are rejected when shell commands are disabled.

## Network Default Provider

The built-in `network` module has an optional default provider name of `"wifi"`. If the network module has no configured click action, clicking the module opens the shipped Wi-Fi provider. A declared provider named `"wifi"` overrides the shipped provider.

The shipped Wi-Fi provider is a separate Cargo binary, `niribar-wifi-provider`, launched through the same framed-JSON external-provider path as user-configured providers. Zero-config launch looks for a `niribar-wifi-provider` sibling of the running `niribar` executable first, then falls back to resolving `niribar-wifi-provider` from `PATH`.

Protocol and content-model types should live in shared library code used by `niribar` and shipped provider binaries. Provider binaries must not duplicate the frame encoding, JSON message enums, or content validation rules.

Provider tests should use backend trait fakes for provider state-machine behavior, protocol subprocess tests for framed JSON integration, and resolver tests for built-in provider name precedence and launch lookup. Normal tests should not require real NetworkManager or iwd services.

The built-in Wi-Fi provider should keep an explicit provider-owned state machine and render `ExternalPanelDocument` from that state. Backend events and host events are inputs to the state machine; backend-specific objects should not leak into content rendering.

Implementation should upgrade the shared provider platform before adding Wi-Fi backend logic. The platform work includes shared protocol/library code, built-in provider resolution, asynchronous provider output reading, fire-and-forget events, custom list values, graceful close, and bounded stderr capture.

Provider state and rendering should use **Wi-Fi Network** concepts. Backend identifiers such as SSID, BSSID, access-point paths, and device paths remain backend adapter details unless a later user workflow needs to expose them.

The Wi-Fi provider sends semantic content only. It may mark entries active, disabled, known, connecting, failed, open, or secured through existing content fields, but Niribar owns styling, icons, focus, scrolling, sizing, spacing, and layout.

Configured click actions always win:

```kdl
modules {
    module "network" {
        actions {
            click { launch "nm-connection-editor" }
        }
    }
}
```

Override or disable the built-in provider name with `provider`:

```kdl
modules {
    module "network" {
        // omitted: use the built-in "wifi" provider
        // provider "wifi"
        provider "my-wifi-provider"
        // provider #false
    }
}
```

For the network module, omitted `provider` and `provider "wifi"` both use the built-in Wi-Fi provider unless user configuration declares a provider named `"wifi"`. Any other provider name must be declared in `panel-providers`. `provider #false` disables the default provider action.

Explicit `open-provider "name"` references must point at a declared or built-in provider and launch failures are rendered as provider failure content in the panel. Built-in provider names are global **Panel Provider Names**, so `open-provider "wifi"` resolves to the built-in Wi-Fi provider from any action unless user configuration declares a provider named `"wifi"`. User-declared providers take precedence over built-in providers with the same name.

Configuration validation accepts provider references that resolve to either a user-declared provider or a built-in provider name. Unknown provider names remain configuration errors.

If a built-in provider reference validates but the shipped provider binary cannot be found or launched at runtime, Niribar renders provider failure content in the hosting panel.

The built-in Wi-Fi provider detects its own control backend when it starts. It uses NetworkManager when available, falls back to iwd Wi-Fi control when NetworkManager is unavailable, and renders no-control status content when neither backend is available. Niribar does not pass its app-owned network status backend choice through the V1 provider protocol.

The built-in Wi-Fi provider talks to supported control backends through structured zbus D-Bus clients, not by shelling out to tools such as `nmcli` or `iwctl`.

Implementation should prioritize the iwd path first so the provider works on systems that use systemd-networkd plus iwd, then add NetworkManager support while preserving the same runtime backend selection policy. The complete built-in Wi-Fi provider must support both the NetworkManager path and the iwd fallback path.

The built-in Wi-Fi provider should complete the handshake promptly with initial content instead of waiting for a scan. Initial content may show scanning status, the **Active Wi-Fi Network** when the backend can report one, and an empty or disabled network list. Scan results arrive as full content replacements.

The built-in Wi-Fi provider starts one **Wi-Fi Scan** when the panel opens and exposes a manual refresh action in its content. It does not need a background polling policy for the first version.

When multiple physical access points advertise the same user-facing network, the provider renders one **Wi-Fi Network** row and chooses the backend target internally.

Wi-Fi list rows should use existing entry metadata for signal strength, security state, active state, known state, connecting state, and failures. V1 does not add provider-supplied icons or custom row widgets for Wi-Fi rows.

Wi-Fi rows should be ordered with the **Active Wi-Fi Network** first, then **Known Wi-Fi Networks**, then stronger signal strength, with stable label ordering as the final tie-breaker.

When multiple Wi-Fi devices are present, the built-in Wi-Fi provider selects one device automatically for V1. It should prefer the active connected device, then an available managed Wi-Fi device. V1 does not expose Wi-Fi device selection in **External Panel Content**.

The built-in Wi-Fi provider supports **Hidden Wi-Fi Networks** by rendering its network list as an **External Selectable List** that allows an **External Custom List Value**. When the typed value does not exactly match a discovered row label, submitting it starts the hidden-network join flow instead of pretending the value is a provider-supplied entry. This exact-match check is case-sensitive even though **Selectable Search** matching may be case-insensitive. Blank or whitespace-only custom values are not valid hidden-network names; V1 trims submitted custom values before starting the hidden-network flow. V1 supports personal secured hidden networks through a **Network Credential Prompt**; enterprise/EAP hidden-network setup is out of scope.

When the user selects a **Known Wi-Fi Network** or an open **Wi-Fi Network**, the provider may start a **Wi-Fi Connection Attempt** immediately. When the user selects a secured **Wi-Fi Network** without saved connection material, the provider should replace its content with a **Network Credential Prompt** that uses secret `text-input` content and submits the credential back to the provider.

Credentials submitted for new secured networks should be saved through the active backend's normal profile or known-network mechanism so future joins treat the network as a **Known Wi-Fi Network**.

The provider owns the user-facing credential prompt for both NetworkManager and iwd flows. Backend-specific agent or request-input mechanisms are adapter details and should be hidden behind the same **Network Credential Prompt** behavior when possible.

Provider diagnostics must never include **Network Credentials**. Panel-visible errors may include the selected **Wi-Fi Network** label, because the user is acting on that network. Runtime logs should avoid SSIDs at normal levels and include them only in debug or trace diagnostics when needed.

Backend errors should be mapped into user-facing categories in panel content, such as no supported backend, scan failed, connection failed, credential rejected, unsupported network type, and authorization required. Raw backend errors belong in diagnostics, not verbatim panel content.

V1 does not implement authorization or polkit prompts inside **External Panel Content**. If backend authorization is required, the provider should surface an authorization-required status while relying on the system's normal authorization agent when available.

The built-in Wi-Fi provider keeps the panel open after a **Wi-Fi Connection Attempt** starts or succeeds. It reports connecting, active, failed, or credential-error state through full content replacements; Escape or an explicit close action closes the panel.

When a credential-backed connection attempt fails because the credential is rejected, the provider should keep the **Network Credential Prompt** visible, show an inline error, keep the selected network context, and clear the credential field.

While a **Wi-Fi Connection Attempt** is active, the provider should prevent duplicate connection attempts from repeated row activation or repeated credential submission. Refresh may remain available when the backend supports concurrent scan and connect operations; otherwise the provider may disable refresh until the attempt completes.

While the **External Panel Session** is open, the built-in Wi-Fi provider should subscribe to backend changes that affect visible Wi-Fi state and send full content replacements when relevant state changes outside a direct user-triggered operation.

V1 does not support disconnecting from or forgetting saved networks from the built-in Wi-Fi provider. It focuses on scan, refresh, join, credential retry, active-state display, and provider failure handling.

## Transport

### Canopa Notify (JSON-lines v4)

Select `protocol "canopa-v4"` inside the `provider` declaration to host Canopa Notify. The default remains `niribar-v1`; unknown names are rejected at configuration load. The two versions name different transports, not successive versions of the Wi-Fi protocol.

```kdl
panel-providers {
    provider "notifications" {
        protocol "canopa-v4"
        command "canopa-notify panel-provider --socket \"$XDG_RUNTIME_DIR/canopa-notify/control.sock\""
    }
}
```

This shell-form example obeys `command-policy`. With shell commands disabled, use `command { argv "canopa-notify" "panel-provider" "--socket" "/absolute/runtime/path/control.sock"; }` instead. Add `click { open-provider "notifications"; }` under a module's `actions` to open it.

Canopa providers send complete `replace_content` JSON lines without waiting for a host handshake. Every message must carry `protocol_version: 4`, terminate with a newline and fit within 1 MiB. The shared crate validates unique section/item identifiers and unambiguous action/control values. Host events use the shared `InteractionEvent` JSON shape and preserve opaque keys and values.

The host renders grouped sections, controls, metadata, all actions, bold/italic/underline and ordered local body images. Image decoding runs off the UI thread: at most 16 unique images per snapshot, 8 MiB encoded bytes per file, 2048×2048 source dimensions, and two concurrent decoding jobs; displayed pixels are reduced to at most 512×512. Invalid/unavailable images retain alternative text. No network fetching or arbitrary markup execution occurs.

Arrow keys select an action, Enter activates it, and Escape closes the panel. The selected action and its item/control context remain visible above the scrolling content. A replacement clears keyboard selection and invalidates old actions. Pointer interaction remains available for every action/control option. Body links are underlined controls with opaque provider actions. Their URI stays in the provider. Native AT-SPI exposes names, button/link roles, selection, focus and actions; updates retain the panel window and retire stale controls. Page Up/Page Down and Home/End read the content without activating a control. The Notify completion evidence records the supported Orca/keyboard acceptance.

See [ADR 0011](adr/0011-explicit-canopa-panel-protocol-adapter.md) and `vendor/canopa-panel-protocol/README.md` for the source-of-truth and drift-check rules.

### Niribar framed JSON v1

Niribar launches the provider on demand and communicates over standard input and standard output. Every message is a framed JSON payload:

1. ASCII magic: `NIRIBAR_EXTERNAL_PANEL_JSON_V1\n`
2. Eight-byte unsigned big-endian payload length.
3. UTF-8 JSON payload.

The maximum payload size is 1 MiB.

## Handshake

Niribar sends a host handshake first:

```json
{
  "type": "handshake",
  "payload": {
    "protocol_version": 1,
    "session_id": "external-panel-1",
    "provider_name": "wifi",
    "origin": {
      "bar_name": "main",
      "output_name": "DP-1",
      "origin_kind": "module"
    }
  }
}
```

The provider must reply with compatible protocol version and initial content:

```json
{
  "type": "handshake",
  "protocol_version": 1,
  "content": {
    "elements": [
      { "type": "text", "text": "Wi-Fi ready" }
    ]
  }
}
```

Invalid JSON, oversized frames, incompatible versions, unexpected messages, provider exits, and invalid content models produce provider failure content and tracing diagnostics. Invalid post-handshake provider output fails the active session instead of leaving stale content interactive.

Niribar should capture a bounded tail of provider stderr for diagnostics and tracing, but should not display raw stderr verbatim in panel content by default. Providers must not write **Network Credentials** to stderr.

## Content

Providers send full content replacements, not patches:

```json
{
  "type": "content",
  "content": {
    "elements": [
      { "type": "text", "text": "Choose a network" },
      {
        "type": "selectable-list",
        "id": "networks",
        "allow_custom": true,
        "entries": [
          { "id": "home", "label": "Home Wi-Fi", "active": true },
          { "id": "guest", "label": "Guest", "status": "open" }
        ]
      },
      {
        "type": "text-input",
        "id": "password",
        "label": "Password",
        "value": "",
        "secret": true
      },
      { "type": "button", "id": "connect", "label": "Connect" }
    ]
  }
}
```

After handshake, providers may send `content` messages whenever provider-owned state changes. Content replacements do not have to be direct responses to host events.

V1 primitives are:

- `text`: static text.
- `button`: interactive button with opaque `id` and visible `label`.
- `selectable-list`: host-rendered list with opaque list `id`, entries, and optional `allow_custom`. Niribar owns local selectable search and ranking.
- `text-input`: input with opaque `id`, visible `label`, current `value`, optional `secret`, and optional `error`.
- `status`: title, message, and `info` or `error` severity.

When `allow_custom` is true and search text has no exact entry-label match, Niribar may render a distinct custom-value affordance for the typed text. Activating that affordance sends a custom-value event carrying the list element ID and typed value, not a provider-supplied entry ID.

Interactive element IDs are opaque provider-owned strings. Niribar only uses them to route typed events and rejects duplicate or empty interactive IDs.

## Events

Niribar sends typed events to the provider. Host events are fire-and-forget: providers may answer interaction events with a full `content` message, and may also send later content replacements from backend subscriptions or long-running operations, but Niribar does not require an immediate acknowledgement.

When a panel-hosted provider session closes, Niribar should send a best-effort `close` event and give the provider a short bounded window to exit before terminating the process.

Closing the panel ends the **External Panel Session** but does not imply rollback of a **Wi-Fi Connection Attempt** already handed to the backend. The provider should only cancel backend work on close when the backend exposes a safe explicit cancellation operation.

```json
{
  "type": "event",
  "payload": {
    "type": "button-activated",
    "element_id": "connect"
  }
}
```

The Rust protocol serializes provider events as enum variants with these fields:

- `button-activated`: `element_id`.
- `selectable-list-entry-activated`: `element_id`, `entry_id`.
- `selectable-list-custom-value-submitted`: `element_id`, `value`.
- `text-input-changed`: `element_id`, `value`.
- `text-input-submitted`: `element_id`, `value`.
- `close`: no fields.

Secret text input values are provider-owned form data. Niribar does not log typed event payload values in provider diagnostics.


## Shutdown and Wi-Fi validation (2026-09-23)

Panels disappear immediately on close. Generic providers retain a 250 ms exit
grace. The `wifi` provider name has a 35-second process-cleanup allowance: a
pending NetworkManager profile-creation call is bounded to 30 seconds and
rollback calls to two seconds. This lets a delayed creation reply be consumed
and its temporary activation/profile retired before the host kills the process
group. Successful NetworkManager profiles are saved after activation; iwd owns
its usual known-network persistence. Cleanup is best effort if the daemon stops
responding or the entire bar is forcibly terminated.

Adapter tests use private D-Bus services, including delayed operations and
cancellation. Real radio, policy-agent and credential persistence behavior still
needs the [desktop acceptance matrix](manual-validation.md).

Wire references: [NetworkManager manager API](https://networkmanager.dev/docs/api/latest/gdbus-org.freedesktop.NetworkManager.html),
[iwd station API](https://kernel.googlesource.com/pub/scm/network/wireless/iwd/%2B/master/doc/station-api.txt),
[iwd credential agent API](https://kernel.googlesource.com/pub/scm/network/wireless/iwd/%2B/master/doc/agent-api.txt).
