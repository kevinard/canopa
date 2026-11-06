# Prepared print authority

Status: in progress; request model and authority foundation verified, 2026-11-02.

## Context

PreparePrint returns settings and a token before the application renders its
document. Print subsequently receives that document descriptor and token. Neither
returned settings nor an application-supplied output URI is authority to submit
to a printer or write a file. The backend method sender is normally the shared
XDG frontend; an empty application ID is valid for native clients and cannot
identify one particular application connection.

The existing interactive request host owns admission, authenticated Close and
method-sender loss while a request is pending. A completed PreparePrint needs a
separate lifetime for its approval. This decision extends that ownership within
the Print module without changing other portal request behavior.

## Decision

Keep caller hints, approved layout and destination authority separate. Publish
only validated known layout/page-setup fields. Page ranges stay zero-based on
the portal wire; conversion to one-based CUPS ranges occurs at the submission
boundary. Printer/output/CUPS extension strings remain advisory inputs.

Authenticate the current owner of `org.freedesktop.portal.Desktop` before trusting
the original client encoded in its canonical request path. Accept the canonical
collision suffix as well as the ordinary path. Direct backend callers are scoped
to their own unique bus name; a forged handle or matching application ID cannot
authenticate another client. Bind the resulting identity to the bus GUID, backend
caller, original client, application ID and whether it came through the frontend.
This initial decoder targets the canonical frontend and dbus-daemon unique-name
encoding; broader bus implementation compatibility remains an acceptance gate.

Subscribe before querying owners. Retain an irreversible revocation monitor for
the original client, method sender and, for frontend-mediated calls, frontend
well-known name. Losing and reacquiring that name on the same connection cannot
revive earlier proofs. Before granting an approval, query current ownership and
synchronize with queued loss processing. A failed verification revokes the proof.

Store at most 64 volatile approvals, at most 8 per original caller regardless of
changes to its application-ID hint. Generate nonzero unpredictable 32-bit tokens,
expire them after 300 seconds and consume each approval at most once. A wrong
identity cannot consume another client's token. Unknown, expired or revoked tokens
require fresh consent; they never select a default destination silently. Remove
an approval before any asynchronous submission work so simultaneous Print calls
cannot use it twice.

The store owns its approval payload, which may contain descriptors and does not
need serialization or cloning. Expiry and owner loss retire that payload without
another incoming method. Store shutdown releases payloads and cancels retirement
tasks. Limits and lifetime are backend policy, not XDG protocol requirements.

## Verification and remaining implementation

The foundation passes 23 protocol/private-bus/resource-lifetime tests. All Portal
library tests pass (238 passed, 8 ignored), as do its integration tests (55 passed,
2 ignored). Formatting, strict workspace/all-target Clippy and the whole-workspace
all-target build pass on the existing incremental graph. Exact Cargo-selected
harnesses run from the Portal crate directory. Receipts live under
`target/portal-print-29/`; this is not a new full-workspace test gate or native
Print acceptance. The earlier ten Files integration failures remain unresolved.

No Print D-Bus interface is registered or advertised by this foundation. The
Iced presenter, retained printer/file destination, bounded document snapshot and
submission worker still need implementation and wire/native verification.
PreparePrint must not create an output file. Its approved target must remain
backend-owned through the subsequent Print rather than being reconstructed from
the application's returned settings.

Keep client-rendered prepared documents distinct from direct Print so copies,
page selection and layout are not applied twice. Actual application compatibility
must determine those mappings. Submission succeeds at final scheduler document
acceptance; it does not promise physical completion. Cancellation must target
only an owned partial job, and ambiguous acceptance must never trigger an
automatic duplicate submission.

The private CUPS transport proof is recorded in
[the Print investigation](../../.scratch/xdg-portals/PRINT.md). Protocol and
private-bus tests for this foundation are separate from end-to-end Print
acceptance. Packaging still contains eleven interfaces; six other interfaces
remain experimental and Print remains unfinished.
