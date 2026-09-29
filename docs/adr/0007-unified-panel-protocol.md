# 0007: Unified panel protocol with interaction revisions

Status: accepted for provider convergence, integration phase 6 (2026-09-27).

`canopa-v5` is the common protocol for the built-in Wi-Fi and Notification Center
providers. It uses provider-first JSON lines, complete content replacements,
explicit interaction revisions, typed input and an explicit close message. The
shared `canopa-panel-protocol::v5` module owns validation, bounded codecs and the
current interaction contract. Its default dependency graph remains Serde/JSON only;
Tokio I/O is opt-in. No dependency version or vendor patch changes are needed.

A provider chooses one of two host-rendered content modes: `Form` for selection,
custom values, text/password inputs and statuses; `Rich` for sections, controls,
items, metadata, styled text, links, local images and opaque actions. Both use the
same envelope, framing, revision rules and close lifecycle. They reuse the frozen
v1/v4 semantic primitives rather than copying them into another schema. Neither
mode carries executable commands, toolkit widgets or host focus/search/image state.
Mixing form fields into rich sections is deferred until an actual consumer needs it.

[ADR 0010](0010-rich-item-source-attribution.md) adds optional source attribution
to rich items while preserving existing item decoding and interaction behavior.

Every response has `protocol_version: 5`, a nonzero `revision`, and `content`.
Responses fit in 1 MiB including the newline. Host events carry the same version,
the captured revision, and a typed `interaction`; close carries just the version.
Host frames fit in 64 KiB including the newline. Readers distinguish EOF from
truncation and read at most one byte beyond their limit. Encoders stop at the limit
instead of building an unbounded serialized frame. Content identifiers, labels,
actions and list entries are validated, with at most 4,096 form elements and 8,192
form elements/entries combined; rich-content limits remain those of v4.

A revision advances when the meaning of the interaction changes. Providers use
`Session::replace` with `Update::Replace` for a new view or operation. Wi-Fi may
request `PreserveFormEdits` after an input edit or background synchronization. The
shared session retains the revision only when the complete form is identical apart
from text-input values and input errors. Changes to status text, IDs, labels,
secret flags, ordering, buttons or list entries advance it. The provider is
responsible for keeping the domain operation unchanged when requesting preservation.
Rich replacements always advance the revision. The host rejects decreasing
revisions and incompatible reuse; providers reject stale revisions, wrong control
kinds, disabled/unknown list entries, disallowed custom values and unpublished rich
actions. The most recent complete contract is the only retained interaction state.

This distinction keeps buffered password edits followed by submission valid: input
echoes share the prompt revision, but starting a connection or returning a failed
attempt retires it. The shell includes its session identity and the wire revision
in form-control IDs, retaining widget identity for compatible echoes and rejecting
late events from replaced or closed panels. Rich controls retain their existing
host generation IDs and opaque domain values. Notify's daemon-instance,
notification-revision and per-view page guards remain authoritative as well.

The process adapters keep existing deadlines, bounded queues, backend cleanup,
image preparation, accessibility and placement. Wi-Fi validates queued events when
they are consumed, after any progress update that may have retired their revision.
Notify wraps its private v4 daemon stream in v5 at the provider process; it retains
legacy socket clients and per-view paging. A v5 close or stdin EOF closes only that
view's socket and wakes its output loop. The shell retains bounded process-exit
cleanup if a provider is stalled. Stream read futures must still be retained across
unrelated updates; cancellation after reading a prefix is not resumable.

Both built-ins explicitly pass `--protocol canopa-v5`. User-declared providers
continue to override the built-in command and protocol together. Omitted protocol
in a user declaration still means `niribar-v1`. `niribar-wifi-provider` without the
new flag remains v1; `canopa-notify panel-provider` remains v4. Explicit v1/v4 host
adapters and their wire contracts remain available. There is no protocol guessing,
configuration rewrite, or forced migration of external providers.

This extends [ADR 0006](0006-shared-provider-contracts.md) and Niribar ADR 0011.
A first portal interface and its request lifecycle remain separate work.

See [validation evidence](../unified-panel-protocol-validation.md).
