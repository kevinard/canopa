# InputCapture Clipboard authority follows actual activation

Status: experimental scoped acceptance passed, 2026-11-01. Complete Portal checks
pass 176 library and 53 integration tests, formatting, all-target Clippy and the
main build. The final candidate passes 143 native groups across the explicitly
recorded appearances and modes. This extends the
experimental pointer sessions from [ADR0061](0061-input-capture-sessions-and-consent.md)
and the Clipboard transport from [ADR0058](0058-clipboard-session-consent-and-ordering.md).
Normal provider advertisement and installed routing remain unchanged.

Clipboard belongs to the existing InputCapture session and its original backend
caller. RequestClipboard must be admitted before Start. The Iced dialog presents
a separate, initially unchecked Clipboard choice; requesting it does not grant it.
Pointer permission remains necessary to start capture. Start reports the actual
Clipboard decision. Existing RemoteDesktop Clipboard sessions keep their own
registry and authorization path; an InputCapture authorization failure cannot
fall through to another session registry.

Permission to capture and an active capture are separate facts. A granted but
inactive InputCapture session opens no Clipboard connection and observes neither
Clipboard bytes nor owner-change metadata. The compositor receiver's actual
activation snapshot controls authority. The standard frontend's cached state or
the arrival order of public signals cannot extend that authority.

Each activation creates a fresh Clipboard transport. A gate independent of the
session actor revokes its offers and byte pumps when the source activation ends,
even while the actor awaits another operation. Actor admission and execution
both check the activation epoch. Queued events retain their epoch and are
discarded when stale. The capture transport clears active state before awaiting
child retirement, so cleanup latency cannot preserve Clipboard access.

An application can retain a pipe descriptor after receiving it. Revocation closes
the backend's transfer endpoints and stops pumping; it cannot recall the
application's descriptor or erase bytes already admitted to its pipe. Native
acceptance must hold readable and writable pipes across deactivation and verify
EOF, closed writers and the bounded admitted prefix, rather than infer revocation
from a dropped duplicate descriptor.

Transfer serials come from a backend-wide, nonwrapping sequence shared across
sessions and replacement workers. The session retains a bounded ledger of issued
serials and their activation epochs. SelectionWrite requires the current issued
serial. A known retired SelectionWriteDone can finish bookkeeping once without
performing I/O against a new transport. Unknown, replayed and stale writes remain
denied. This distinguishes completion from permission to transfer more bytes.

The actor mailbox, event queue, issued-serial ledger, operations and helper
retirement remain bounded. Loss of the session, source or ongoing Iced controls
revokes both capture and Clipboard. The controls describe Clipboard as available
only while input is captured; Stop ends both grants. Keyboard and touch capture,
broader physical/failure acceptance, frontend phase-cache corrections and
companion distribution remain separate work before production advertisement.

Validation must cover exact session authorization and request ordering, unchecked
consent, no inactive connection or metadata, fresh activation transfers, retained
pipes, stale completion isolation, Iced Stop and original caller/backend loss.
Preserve the existing receiver, RemoteDesktop Clipboard and ScreenCast native
regressions on the same candidate. The
[InputCapture ledger](../../.scratch/xdg-portals/input-capture/clipboard-evidence.json)
records exact source/binary pins, terminal process evidence and results. Thirty
combined Clipboard/capture groups, sixteen pointer-only baseline groups, nine
private receiver groups, 54 Clipboard/RemoteDesktop groups and 34 standalone
ScreenCast groups pass. All thirty capture-worker handles in the final suite are
terminal. A later Python-only extension adds Disable/topology checks; the source
follow-up proves Rust, manifests and vendor inputs unchanged after the full check.
