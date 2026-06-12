# External Panel Providers

An **External Panel Provider** is an out-of-process program that owns domain logic for a rich panel while Niribar owns the visible panel window, placement, styling, focus, sizing, scrolling, and input routing.

V1 providers are panel-only. They cannot create provider-owned menus or tooltips, and provider content cannot ask Niribar to run configured commands.

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

Open a provider from any panel action with `open-provider`:

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

The built-in `network` module has an optional default provider name of `"wifi"`. If a provider named `"wifi"` is declared and the network module has no configured click action, clicking the module opens that provider.

Configured click actions always win:

```kdl
modules {
    module "network" {
        actions {
            click { command "nm-connection-editor" }
        }
    }
}
```

Override or disable the built-in provider name with `provider`:

```kdl
modules {
    module "network" {
        provider "my-wifi-provider"
        // provider #false
    }
}
```

Missing optional default providers are quiet. Explicit `open-provider "name"` references must point at a declared provider and launch failures are rendered as provider failure content in the panel.

## Transport

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

Invalid JSON, oversized frames, incompatible versions, unexpected messages, provider exits, and invalid content models produce provider failure content and tracing diagnostics.

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

V1 primitives are:

- `text`: static text.
- `button`: interactive button with opaque `id` and visible `label`.
- `selectable-list`: host-rendered list with opaque list `id` and entries. Niribar owns local selectable search and ranking.
- `text-input`: input with opaque `id`, visible `label`, current `value`, optional `secret`, and optional `error`.
- `status`: title, message, and `info` or `error` severity.

Interactive element IDs are opaque provider-owned strings. Niribar only uses them to route typed events and rejects duplicate or empty interactive IDs.

## Events

Niribar sends typed events to the provider. Providers should answer interaction events with a full `content` message.

```json
{
  "type": "event",
  "payload": {
    "type": "button-activated",
    "element_id": "connect"
  }
}
```

The Rust protocol currently serializes provider events as enum variants with these fields:

- `button-activated`: `element_id`.
- `selectable-list-entry-activated`: `element_id`, `entry_id`.
- `text-input-changed`: `element_id`, `value`.
- `text-input-submitted`: `element_id`, `value`.
- `close`: no fields.

Secret text input values are provider-owned form data. Niribar does not log typed event payload values in provider diagnostics.
