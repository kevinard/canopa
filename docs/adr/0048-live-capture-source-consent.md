# 0048: Live monitor identity through source consent

Status: portal implementation with scoped native acceptance, 2026-10-31.
The compositor extension is a prepared draft, not a canonical recipe or installed
provider. ScreenCast and RemoteDesktop remain unadvertised.

A connector name can survive the removal and replacement of an Output object.
Comparing that name after consent cannot distinguish the displayed source from
its replacement. PipeWire numeric node IDs identify producers at a later stage
and are also reusable. Neither is a live monitor identity.

Keep the existing unique compositor owner and IPC peer-PID binding. For the
prepared CanopaSourceVersion=1 extension, read MonitorInfo on that exact owner
before showing the Iced source catalog. Carry the returned nonzero process-local
Output identity alongside its connector through catalog refresh, selection,
post-consent revalidation and the separate recording worker. It is not a hardware
identifier, persistence token or display label. A new identity clears an existing
selection even when connector, title and model are unchanged. Review refresh also
requires the originally pinned unique owner, not just the same process ID.

The Monitor value preserves the legacy connector-string wire representation for
view-only capture on providers without the extension. Identity-bearing values
use a structured private representation and reject zero identities or invalid
connectors. Duplicate connector aliases are invalid even when their identity
values differ. Only UnknownProperty permits the legacy inventory path; query
failure, malformed/unsupported versions and missing sources must not silently
downgrade an identity-bearing selection.

At RecordMonitor, the worker requires the selected canopa-source-id. An
identity-bearing request cannot run against an unextended provider. After the
producer's PipeWireStreamAdded signal, read current Parameters, verify its
canopa-source-id against the selection and validate logical geometry before
publishing ready. This also avoids publishing geometry read before Start.
Failure stops the original capture session and exposes no streams. Window
selection retains its unique process-local MappedId and sends it as the same
optional precondition to RecordWindow. Existing window geometry remains omitted
from public results until its separate capture-mapping integration.

This establishes monitor identity across consent on an identity-capable provider.
Legacy view-only capture retains its older connector semantics and is not enough
for absolute pointer authorization. Before pointer advertisement, adopt and
distribute the compositor extension, require capture-bound regions, integrate
worker renewal/revocation, EI and legacy Notify pointer handling, and explicit
Iced pointer permission. The prepared OpenMappedPointer transport and its native
proofs do not themselves create a portal grant.

The portal application uses Rust/Iced and adds no dependency or GTK component.
The standard frontend retains sandbox identity, documents and consumer PipeWire
access. Current routing and normal RemoteDesktop registration are unchanged.

The final portal checks pass 145 library tests and 37 integration tests, with
all-target Clippy clean. Forty native groups pass on frozen portal executables:
17 identity-capable video/controls groups, 17 legacy view-only groups and six
combined keyboard EIS/media lifetime groups. Private D-Bus traces show the
identity precondition on every new-provider monitor recording. One earlier
parallel library run's two cancellation timeouts are retained with their cause
unresolved; direct controls and the full normal rerun pass.

Evidence and remaining work: [portal source identity evidence](../../.scratch/xdg-portals/portal-source-identities/README.md),
[capture mapping ledger](../../.scratch/xdg-portals/POINTER-MAPPING.md),
[ScreenCast consent](0042-screencast-sessions-and-consent.md),
[RemoteDesktop lifetime](0046-session-owned-remote-input.md).
