# canopa-panel-protocol

Wire contracts between Canopa Shell and panel providers: the out-of-process
programs that supply the content of rich panels such as Wi-Fi, Sound, Media and
the Notification Center. The Shell renders and owns input; a provider owns its
domain and decides what the panel shows. This crate has no toolkit, Shell or
Notify dependency, so a provider depends on it directly.

| Module | Protocol name | Framing | Used by |
| --- | --- | --- | --- |
| `v5` | `canopa-v5` | JSON lines, provider first, interaction revisions, explicit close | All built-in providers |
| `v1` | `niribar-v1` | Magic + big-endian length + JSON, host handshake first | Declared providers that omit `protocol` |
| `v4` | `canopa-v4` | JSON lines, provider first | `canopa-notify panel-provider` without `--protocol` |

The `v4` types are also re-exported at the crate root.

## Main types

- `v5::Response` (`protocol_version`, `revision`, `content`) and
  `v5::HostMessage` (`event` with the captured revision and an optional
  `activation_token`, or `close`), each with `encode`, `decode` and `read`.
- `v5::Content`: `Form` (the v1 element document) or `Rich` (`v4::PanelContent`).
- `v5::Session`: the current interaction contract. Providers call `replace` with
  `Update::Replace` or `Update::PreserveFormEdits`; hosts call `observe`;
  providers check incoming events with `accepts`.
- `v4::PanelContent`, `PanelSection`, `PanelItem`, `PanelAction`, `PanelControl`,
  `PanelSlider`, `PanelAttribution`, `PanelIcon`, `RichBodySegment`, and the
  `InteractionEvent::Invoke` event. `v4::Frame` validates a frame and keeps its
  original bytes for forwarding.
- `v1::ExternalPanelDocument`, `ExternalPanelElement`, `ProviderEvent`,
  `encode_frame` and `decode_frame`.
- `ProtocolError` for oversized, truncated, invalid or unexpected input.

Every reader and encoder is bounded: v5 responses and v4/v1 frames to 1 MiB, v5
host messages to 64 KiB. Content is validated before use, including whether
every advertised control can be sent back within the host-message limit.

## Features

- default: Serde and serde_json only.
- `tokio`: async readers and writers (`v5::Response::read_async`,
  `v4::Frame::read_async`, `v1::perform_handshake`, `v1::read_frame`,
  `v1::write_frame`). It adds I/O traits, not a runtime.

Callers own deadlines, queues, cancellation and process lifetime. A read that has
consumed part of a frame cannot be resumed after cancellation: keep the read
future alive across unrelated work.

## Documentation

- [Panel protocol architecture](../../docs/architecture/panel-protocol.md):
  contracts, revisions, activation context, compatibility rules and design
  decisions.
- [External panel providers](../../docs/shell/external-panel-providers.md):
  writing and declaring a provider.

Licensed under MIT (`LICENSE`), the option this crate selected from the original
v4 dual license. `LICENSE-MIT` and `LICENSE-APACHE` are those original v4 texts,
kept for attribution; they do not add a license option. See `LICENSES.md` at the
repository root.
