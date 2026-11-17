# External panel providers

An **External Panel Provider** is an out-of-process program that owns the domain
logic of a rich panel. Canopa Shell owns the visible panel window, placement,
styling, focus, sizing, scrolling and input routing. Providers send declarative
content only: they cannot create menus or tooltips, run configured commands or
supply widgets or markup.

This page is the reference for configuring and writing providers. The Shell side
is described in [architecture/shell.md](../architecture/shell.md#panel-providers)
and the wire contracts in [architecture/panel-protocol.md](../architecture/panel-protocol.md).
Use the `canopa-panel-protocol` crate (`crates/canopa-panel-protocol`) rather than
reimplementing frame encoding, message types or validation.

## Lifecycle

- The Shell starts the provider when its panel opens and talks to it over stdin
  and stdout.
- Content replacements redraw the existing panel window, including background
  updates and local search or text edits. They never close and reopen the panel.
  A replacement control cannot inherit a held click or keyboard focus.
- Clicking the module again dismisses its provider panel, also while the provider
  is still opening; a late reply cannot reopen it. Activating another module or bar
  opens that origin's panel.
- On close, the Shell sends a close message, waits a bounded grace and then kills
  the provider's process group: 250 ms in general, 3 s for `bluetooth` and 35 s for
  `wifi`. Closing a panel does not roll back work already handed to a backend.
- Invalid JSON, oversized frames, incompatible versions, unexpected messages,
  invalid content and provider exits show provider failure content in the panel
  and are logged. Invalid output after the session starts ends the session rather
  than leaving stale content interactive.
- The Shell keeps the last 4 KiB of the provider's stderr for diagnostics and never
  shows it in the panel. Providers must not write secrets (such as network
  credentials) to stderr. The Shell does not log typed input values.

## Configuration

Declare providers by name in the `shell` namespace with the usual command grammar:

```kdl
panel-providers {
    provider "my-vpn" {
        protocol "canopa-v5"
        command { argv "my-vpn-provider" "--protocol" "canopa-v5"; }
    }
}
```

`protocol` is `niribar-v1` (the default when omitted), `canopa-v4` or `canopa-v5`;
anything else is a configuration error. The Shell never guesses a protocol from
the command. Commands obey `command-policy`: shell-form commands are rejected when
shell commands are disabled, and `argv` arguments are not expanded (no
`$XDG_RUNTIME_DIR`).

Open a provider from any action with `open-provider "NAME"`:

```kdl
modules {
    module "network" {
        actions {
            click { open-provider "wifi" }
        }
    }
}
```

References must name a declared or built-in provider; unknown names are
configuration errors. If a valid provider cannot be found or started at runtime,
the panel shows provider failure content.

### Built-in providers

| Name | Command | Opened by default from |
| --- | --- | --- |
| `wifi` | `canopa-wifi-provider --protocol canopa-v5` | `network` module |
| `audio` | `canopa-audio-provider --protocol canopa-v5` | `volume` module, Sound card |
| `media` | `canopa-media-provider --protocol canopa-v5` | `media` module |
| `bluetooth` | `canopa-shell --hardware-panel bluetooth` | Quick settings `bluetooth` |
| `power` | `canopa-shell --hardware-panel power` | Quick settings `power-and-brightness` |
| `notifications` | `canopa-notify panel-provider --protocol canopa-v5` | `notifications` module |

Built-in names are global and use `canopa-v5`. Companion binaries are found next to
the running `canopa-shell` first, then on `PATH`. A declared provider with the same
name overrides the built-in one, including its protocol.

Configured actions always win over built-in defaults. For the `network` module,
`provider` selects or disables its default click:

```kdl
modules {
    module "network" {
        // omitted or "wifi": the built-in Wi-Fi provider (unless "wifi" is declared)
        provider "my-wifi-provider"
        // provider #false disables the default click action
    }
}
```

## Panel size

Provider popovers default to 360 × 600 logical pixels and use the available bar
height when it is smaller. `panel { height ...; max-height ...; }` on the hosting
bar controls the size. Content updates keep the viewport, including the change
from loading to populated content. Drawers fill their edge as usual. The
Notification Center is 480 logical pixels wide and fills the available height.

## Choosing a protocol

| Protocol | Framing | Content | Use |
| --- | --- | --- | --- |
| `canopa-v5` | JSON lines, provider first | Form or rich, with interaction revisions and explicit close | New providers; all built-ins |
| `canopa-v4` | JSON lines, provider first | Rich only; no close message | Existing providers |
| `niribar-v1` | Length-framed JSON, host handshake first | Form only | Existing providers; default when `protocol` is omitted |

Form content is the v1 element set (text, button, selectable list, text input,
status). Rich content is the v4 model (sections, items, controls, actions, styled
and rich bodies, images, sliders). v5 carries either.

## canopa-v5

The provider writes first; there is no host handshake. Every message is one JSON
object followed by a newline. Provider responses may be at most 1 MiB and host
messages 64 KiB, including the newline.

Provider response:

```json
{"protocol_version":5,"revision":1,"content":{"kind":"form","body":{"elements":[
  {"type":"status","title":"Wi-Fi","message":"Choose a network","severity":"info"},
  {"type":"button","id":"refresh","label":"Scan"}
]}}}
```

For rich content use `"kind":"rich"` with a v4 `PanelContent` as `body`.

- `revision` is nonzero and never decreases. Increase it for every new interaction
  contract. Repeating the previous revision is allowed only for a compatible form
  update that changes input values or errors, such as a password field edit; any
  other change must advance it (`Session::replace` with `Update::PreserveFormEdits`
  does this check).
- Responses may be sent at any time, not only in reply to an event.

Host messages:

```json
{"type":"event","protocol_version":5,"revision":1,"interaction":{"kind":"form","event":{"type":"button-activated","element_id":"refresh"}}}
{"type":"close","protocol_version":5}
```

- An event echoes the revision it was produced from and names an advertised
  control. Providers should reject events for other revisions (`Session::accepts`).
- A rich action with `"request_activation_token": true` may receive an event with
  an extra `activation_token` (a Wayland `xdg_activation_v1` token from the click,
  at most 4,096 bytes). The Shell sends the event without a token when none can be
  obtained. Pass the token to whatever you launch or activate.
- After `close`, exit promptly.

## Form content and events

Form content (v1 and v5 form) is a list of elements:

| Element | Fields |
| --- | --- |
| `text` | `text` |
| `button` | `id`, `label` |
| `selectable-list` | `id`, `entries` (`id`, `label`, optional `description`, `disabled`, `active`, `status`), optional `allow_custom` |
| `text-input` | `id`, `label`, `value`, optional `secret`, optional `error` |
| `status` | `title`, `message`, `severity` (`info` or `error`) |

Interactive IDs are opaque provider strings; the Shell rejects empty or duplicate
ones. The Shell owns list search and ranking. When `allow_custom` is true and the
typed text matches no entry label exactly, the Shell offers a custom-value row;
activating it sends the typed value, not an entry ID.

Events (`type` field):

| Event | Fields |
| --- | --- |
| `button-activated` | `element_id` |
| `selectable-list-entry-activated` | `element_id`, `entry_id` |
| `selectable-list-custom-value-submitted` | `element_id`, `value` |
| `text-input-changed` | `element_id`, `value` |
| `text-input-submitted` | `element_id`, `value` |
| `close` | none (v1 only; v5 uses the `close` message) |

Secret input values are provider-owned form data.

## Rich content

Rich content (`PanelContent`) has a `title`, `sections` (each with `id`, `title`,
optional `icon`, `items` and `controls`), content-level `actions` and an optional
`error`. An item has `id`, `title`, optional `attribution` (source label and icon),
`body`, `styled_body` or `rich_body`, `metadata`, `actions` and an optional
`slider`. Interactions are `invoke` events with a `target` (`content`, `section` or
`item`), the `action_key` and the action's opaque `value`, returned unchanged.

How the Shell renders it:

- Grouped sections, controls (radio choices), metadata, all actions, bold, italic
  and underline, and ordered local body images. Images are decoded off the UI
  thread: at most 16 unique images per snapshot, 8 MiB per file, 2048 × 2048 source
  pixels, two decoding jobs at once, shown at most 512 × 512. Failed images show
  their alternative text. Nothing is fetched from the network.
- Icons are a local absolute path or theme names (`{"kind":"theme","names":[...],"theme":...,"scale":1..8}`).
- Action hints, ignored by hosts that do not know them: `"destructive": true` for
  an action that removes or discards something, and `"pressed": true|false` for a
  view or setting switch. Content-level actions render as compact chips,
  destructive ones in the danger tone and switches with an on/off switch. On an
  attributed item, a destructive item action becomes the card's round close
  control, and a presence status with a single other action becomes an unread dot
  that performs it.
- A `slider` (`key`, `label`, `min`, `max`, `step`, `value`, `unit`) commits its
  value through the item's `invoke` as a decimal string.
- Body links are `link` segments carrying a `uri` and an opaque action. The URI is
  accessibility metadata only; activation sends the action to the provider, which
  decides how to open it.
- Keyboard: arrows and Tab select an action, Enter activates it, Page Up, Page Down,
  Home and End scroll, Escape closes. The selected action and its item context stay
  visible above the scrolling content. A replacement clears keyboard selection and
  invalidates old actions. Native AT-SPI exposes names, button and link roles,
  selection, focus and actions.

## canopa-v4

The provider writes `{"type":"replace_content","protocol_version":4,"content":{...}}`
lines (at most 1 MiB each) without waiting for the host. Host events are the
`InteractionEvent` JSON shape. There is no close message: the host closes stdin and
applies the exit grace. v4 cannot carry activation tokens.

## niribar-v1

Every message is a frame:

1. ASCII magic `NIRIBAR_EXTERNAL_PANEL_JSON_V1\n`;
2. an eight-byte unsigned big-endian payload length;
3. the UTF-8 JSON payload, at most 1 MiB.

The host sends a handshake first:

```json
{"type":"handshake","payload":{"protocol_version":1,"session_id":"external-panel-1","provider_name":"wifi","origin":{"bar_name":"main","output_name":"DP-1","origin_kind":"module"}}}
```

The provider replies with its version and initial content, then sends full
replacements whenever its state changes:

```json
{"type":"handshake","protocol_version":1,"content":{"elements":[{"type":"text","text":"Wi-Fi ready"}]}}
{"type":"content","content":{"elements":[...]}}
```

Host events are `{"type":"event","payload":{"type":"button-activated","element_id":"connect"}}`.
Events are fire-and-forget; the host does not wait for a reply.

## Writing and testing a provider

- Keep an explicit provider-owned state machine and render content from it.
  Backend events and host events are inputs; backend objects should not leak into
  content.
- Send semantic state only (active, disabled, known, connecting, failed, secured);
  the Shell owns icons, styling and layout.
- Map backend errors to user-facing categories in content and keep raw errors in
  logs.
- Test the state machine against backend fakes and the protocol through subprocess
  tests; normal tests should not need real system services.

## Built-in Wi-Fi provider

`canopa-wifi-provider` (`crates/canopa-shell/src/wifi_provider.rs`) is a form-content
provider and a reference implementation. Without arguments it speaks
`canopa-v5`, as the Shell runs it; `--protocol niribar-v1` keeps the legacy wire
for existing declarations.

- **Backend.** It shares the network bundle in `canopa-services` with the Shell's
  status module: NetworkManager when present, otherwise iwd for control (with
  networkd for status). It follows D-Bus owner changes, including a daemon that
  appears later. `CANOPA_WIFI_PROVIDER_BACKEND=networkmanager|iwd|none` restricts
  the choice. All backend calls use zbus D-Bus clients, not `nmcli` or `iwctl`.
- **Content.** Initial content is sent immediately (scanning state, the active
  network if known); one scan starts when the panel opens and the Scan button
  repeats it. Backend changes while open send full replacements. Access points with
  the same SSID and security collapse into one row. Rows are ordered active first,
  then known networks, then by signal strength, then by label.
- **Joining.** Known and open networks connect immediately. A secured network
  without saved credentials shows a secret `text-input` prompt; rejected
  credentials keep the prompt with an inline error and a cleared field. New
  credentials are saved through the backend's normal profile or known-network
  mechanism. Repeated activation cannot start a duplicate attempt.
- **Hidden networks.** A custom list value that matches no row label exactly
  (case-sensitive, trimmed, not blank) starts a hidden personal network join.
  Enterprise (EAP) setup is not supported.
- **Radio and saved networks.** When the backend manages the radio, the panel
  offers Turn off or Turn on, Disconnect for the active network and Forget for the
  active network or a known network that just failed.
- **Authorization.** The provider shows an authorization-required status and relies
  on the session's polkit agent; it does not prompt itself.
- **Privacy.** Credentials never appear in diagnostics; SSIDs are logged only at
  debug level.
- **Shutdown.** A pending NetworkManager profile creation is bounded to 30 s and
  rollback calls to 2 s, which is why the Shell allows 35 s before killing the
  process. Cleanup is best effort if the daemon stops responding.

Wire references: [NetworkManager D-Bus API](https://networkmanager.dev/docs/api/latest/gdbus-org.freedesktop.NetworkManager.html),
[iwd station API](https://kernel.googlesource.com/pub/scm/network/wireless/iwd/%2B/master/doc/station-api.txt),
[iwd agent API](https://kernel.googlesource.com/pub/scm/network/wireless/iwd/%2B/master/doc/agent-api.txt).

## Notification Center provider

The `notifications` built-in is `canopa-notify panel-provider --protocol canopa-v5`.
It connects to the Notify daemon's private control socket (default
`$XDG_RUNTIME_DIR/canopa-notify/control.sock`) and streams rich content; when the
daemon is unavailable it shows an error. The design is in
[architecture/notify.md](../architecture/notify.md#notification-center-provider).

No declaration is needed for the standard socket. To use another socket, override
the built-in with an absolute path (argv does not expand variables):

```kdl
panel-providers {
    provider "notifications" {
        protocol "canopa-v5"
        command { argv "canopa-notify" "panel-provider" "--protocol" "canopa-v5" "--socket" "/run/user/1000/canopa-notify/control.sock"; }
    }
}
```

`canopa-notify panel-provider` speaks `canopa-v5` when the flag is omitted;
`--protocol canopa-v4` remains for existing hosts. Set the flag and `protocol`
together: a declaration without `protocol` still means `niribar-v1`, which no
built-in provider speaks by default. Only `canopa-v5` carries activation tokens
for Center actions.

Behaviour a host can rely on:

- The reading view is one list, newest first, with a Group by app switch, unread
  dots, item actions (Open, Dismiss, Mark as read, Remove, Reply, sender actions)
  and Mark all read, Clear all (with Undo) and Do Not Disturb choices. A Settings
  view holds app and category delivery choices (Show banners, Save quietly, Block)
  and Delete history; Back returns to the same page.
- Live transient notifications appear in date order with retained ones and
  disappear when they end; opening the Center does not start banner timeouts.
- Content is paginated with First, Previous and Next page controls. Pages keep
  complete bodies and every action; frames are at most 1 MiB. Each open panel has
  its own position.
- Every action value is bound to the displayed revision and daemon instance. The
  daemon rejects actions for replaced content or a previous daemon, even if the
  action key is reused. Return values unchanged.
- Link URIs are metadata for assistive technology; Notify opens links itself
  through the OpenURI portal when the link action is invoked.
- Provider input lines may be up to 65,518 bytes. Each interaction has a 5 s
  deadline and is never retried; a timeout does not prove the action was
  rejected. Invalid daemon output becomes a small visible error.
- At most eight Center panels can be open at once; another gets a visible limit
  error.
