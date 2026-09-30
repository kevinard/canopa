# Canopa panel provider contracts

This crate supplies versioned provider contracts without a GUI dependency:

- `v5`: the common built-in provider contract: form/rich content, interaction
  revisions, bounded JSON lines and explicit close. See [ADR 0007](../../docs/adr/0007-unified-panel-protocol.md).
- `v1`: the `niribar-v1` framed JSON handshake, declarative controls and typed events.
- `v4`: the `canopa-v4` JSON-lines content and opaque interactions. Existing root
  imports remain aliases for this contract.

Use `canopa-panel-protocol` directly when writing a provider; do not depend on the
shell for its protocol types. Enable the optional `tokio` feature for asynchronous
v1 framing and handshakes, `v4::Frame::read_async`, or v5 response/message readers.
The default dependency graph
contains only Serde/JSON dependencies. Synchronous v1 callers can use `encode_frame`
and `decode_frame`; v4 forwarders can use `Frame::read` and `Frame::into_bytes`.

A host selects `canopa-v5`, `niribar-v1` (the configured-provider default), or
`canopa-v4` explicitly in its provider configuration. Existing v1/v4 contracts
remain available alongside the explicitly selected v5 successor; hosts never guess
a protocol. All v4 provider responses carry version 4; v1 establishes version 1 in
its handshake. Existing wire field names and limits are unchanged.

Rich items may include optional `attribution` with a source label and local/theme
icon reference. Older readers ignore this field; omission preserves the original
item wire shape. Rust constructors without source identity use `attribution: None`.
Sections may include an optional `icon` of the same type beside their title;
omission preserves their wire shape too. Rust constructors use `icon: None`
when unused.
See [ADR 0010](../../docs/adr/0010-rich-item-source-attribution.md).

`v4::Frame` validates the complete replacement, including content identifiers and
actions. v1 callers validate `ExternalPanelDocument` before displaying replacements;
`perform_handshake` also validates the initial document. Read errors terminate the
stream unless a caller has a documented resynchronization policy. Read futures must
not be cancelled midway through a frame. Callers own execution, timeouts, queues,
process cleanup, and applying the interaction contract. The v5 `Session` shares
revision and advertised-control validation between host and provider; application
domain guards remain caller-owned.

When migrating Rust source from `niribar::external_panel::protocol`, omit the old
`search_query` field from `SelectableList` constructors. It was host-only and never
serialized. The host-only `Canopa` element variant also moves out of the wire enum.
The old module path remains a reexport, but those Rust fields are intentionally
removed. Existing provider wire bytes remain compatible.

Rendering, list search text, prepared images and keyboard focus are host state.
Notification policy and Wi-Fi backend state belong to their respective providers.
See [ADR 0006](../../docs/adr/0006-shared-provider-contracts.md).

Rich items may include an optional `slider` with `key`, `label`, `min`, `max`,
`step`, `value` and `unit`. Numeric fields are bounded unsigned integers. A committed
value uses the existing item `Invoke` and canonical decimal string; v5 validates
the declared range/grid and revision. Constructors without a slider use
`slider: None`, which preserves the existing wire shape. See
[ADR 0012](../../docs/adr/0012-rich-panel-sliders.md).
