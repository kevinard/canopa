# 0006: Share provider contracts before changing the wire protocol

Status: accepted for the provider foundation slice of integration phase 6 (2026-09-27).

Both existing panel contracts belong to `canopa-panel-protocol`: framed JSON
`v1` for Wi-Fi and JSON-lines `v4` for Notify. Providers can depend on this crate
without depending on Niribar, Iced, Wayland or an application runtime. Existing
root Rust imports still refer to v4. Niribar's former public protocol path reexports
v1 for callers migrating their imports. Rust callers constructing v1 lists must
remove the old host-only `search_query` field; the non-wire `Canopa` variant is
also removed. This preserves wire compatibility, not those host-coupled Rust fields.

This is source and mechanism consolidation, not the versioned successor described
in the integration proposal. Existing provider commands, configuration defaults,
handshakes, JSON fields, size limits and close behavior remain unchanged. A provider
must still select the contract explicitly; the host does not guess it.

| Contract | `v1` | `v4` |
| --- | --- | --- |
| Start | Host-first handshake with session and bar/output origin | Provider-first complete replacement |
| Framing | Existing magic, big-endian u64 payload length, JSON | One newline-terminated JSON response |
| Limit | 1 MiB JSON payload | 1 MiB including newline |
| Content | Text, button, searchable selection, custom value, secret input, status | Sections, controls, items, metadata, styled/rich text, local images, opaque actions |
| Input | Typed element/list/input events | Opaque target, action key and value |
| Close | Typed close event, then bounded process cleanup | Close stdin, then bounded process cleanup |

The shared crate owns wire data, content validation and bounded codecs. Its default
normal dependency graph is Serde/JSON only. Tokio readers/writers are behind the
explicit `tokio` feature; the feature adds I/O traits, not a runtime or process
manager. The runtime, deadlines, backpressure and cancellation remain with callers.
In particular, callers must retain an in-progress read across unrelated updates:
these stream reads are not cancellation safe after consuming a prefix.

The synchronous Notify forwarder and asynchronous shell adapter use the same v4
frame validation. A validated frame can yield either its original bytes, for
forwarding without wire rewriting, or its content, for rendering. EOF, incomplete
frames, unsupported versions, malformed JSON and invalid content remain distinct
outcomes. The read takes at most the byte limit plus one overflow probe.

Niribar now owns a non-serializable prepared document: either v1 elements or a v4
snapshot with decoded images and generation-scoped interactions. Its list search
text lives alongside that document as local host state. These two host-only fields
were previously embedded as Serde-skipped members of v1 wire types. A v1 provider
still cannot serialize a v4 snapshot, and provider-supplied search text still has
no effect. Text-input values remain wire data because Wi-Fi consumes them.
Replacement content resets the local list query as before; input echo and retained
widget identity keep their existing behavior.

The Wi-Fi implementation imports the canonical v1 types directly. Its backend
selection, ordered credential edits, in-flight operation and cleanup stay in the
provider. Notify retains its daemon-instance, notification-revision and per-view
page guards. Niribar retains its stale-session/generation checks, rendering,
accessibility, keyboard focus, image admission and process cleanup.

The extracted v1 code retains Niribar's MIT attribution. The combined crate uses
MIT, selecting the existing v4 code's MIT option; dependency notices are updated.
No vendor fork or upstream dependency version changes are needed.

A successor remains a separate compatibility change. Before migrating built-ins,
it must cover both rows of the content table, define revision handling for buffered
credential edits, and pass Wi-Fi and Center fixtures while both existing adapters
remain available. Portal request lifetimes are also separate from bar panels.

See [validation](../shared-provider-contracts-validation.md).
