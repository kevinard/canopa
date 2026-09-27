# Canopa panel provider contracts

This crate supplies the two existing provider contracts without a GUI dependency:

- `v1`: the `niribar-v1` framed JSON handshake, declarative controls and typed events.
- `v4`: the `canopa-v4` JSON-lines content and opaque interactions. Existing root
  imports remain aliases for this contract.

Use `canopa-panel-protocol` directly when writing a provider; do not depend on the
shell for its protocol types. Enable the optional `tokio` feature for v1 asynchronous
framing and handshakes or `v4::Frame::read_async`. The default dependency graph
contains only Serde/JSON dependencies. Synchronous v1 callers can use `encode_frame`
and `decode_frame`; v4 forwarders can use `Frame::read` and `Frame::into_bytes`.

A host selects `niribar-v1` (the existing default) or `canopa-v4` explicitly in its
provider configuration. These are separate compatibility contracts, not negotiated
versions of a new protocol. All v4 provider responses carry version 4; v1 establishes
version 1 in its handshake. Existing wire field names and limits are unchanged.

`v4::Frame` validates the complete replacement, including content identifiers and
actions. v1 callers validate `ExternalPanelDocument` before displaying replacements;
`perform_handshake` also validates the initial document. Read errors terminate the
stream unless a caller has a documented resynchronization policy. Read futures must
not be cancelled midway through a frame. Callers own execution, timeouts, queues,
process cleanup, and rejection of stale interactions.

When migrating Rust source from `niribar::external_panel::protocol`, omit the old
`search_query` field from `SelectableList` constructors. It was host-only and never
serialized. The host-only `Canopa` element variant also moves out of the wire enum.
The old module path remains a reexport, but those Rust fields are intentionally
removed. Existing provider wire bytes remain compatible.

Rendering, list search text, prepared images and keyboard focus are host state.
Notification policy and Wi-Fi backend state belong to their respective providers.
See [ADR 0006](../../docs/adr/0006-shared-provider-contracts.md).
