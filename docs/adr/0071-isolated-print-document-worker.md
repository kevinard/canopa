# Isolated print document worker

Status: scoped worker transport verified; Print interface and presentation remain
in progress, 2026-11-02.

## Context

A Print caller supplies a descriptor, not a stable pathname. Its contents can
change, its offset can be shared, and a producer can leave a pipe open forever.
Printer discovery/submission can block independently of the Iced event loop.
Request cancellation also drops futures; that alone must not abandon a child
process, document allocation or known partial job.

Prepared approval ownership remains with ADR0070. This worker does not make
caller settings or an output URI into printer/file authority, and does not
register a Print portal by itself.

## Decision

Run descriptor reads and CUPS operations in the existing executable's private
`--print-worker` mode. Inherit only explicit descriptors, duplicate validated raw
numbers into owned files and restore close-on-exec isolation. The worker does not
load shared appearance, initialize Iced or connect to an installed portal service.

Snapshot received contents into a fresh sealable memfd with a 128 MiB ceiling.
For regular files, query the received offset and use positional reads without
moving that shared offset or changing caller status flags. Poll streams alongside
a separate cancellation descriptor; handle delayed nonblocking producers. Reject
empty/oversized inputs, changed regular sources and unsuitable output targets.
Seal the completed snapshot against writes, growth and shrink before returning it.
Known PDF/PostScript/SVG prefixes are optional format hints; other readable printer
formats are retained rather than rejected solely for lacking those prefixes.

The Rust client owns two retained-document slots and two active-operation slots.
A task retains those resources after its initiating future is dropped. Closing
the independent cancellation pipe stops cooperative work. A 60-second operation
deadline and 10-second stop grace lead to child termination and reaping. Hold
snapshot storage until retirement completes. These bounds prevent repeated
cancellation from accumulating helpers or retained documents.

Load the system `libcups.so.2` only for printer operations. Bind its public
destination API in Rust and retain the library/connection/destination/capability
objects through every call. Accept a named configured queue/instance and explicit
typed device choices, then recheck those choices against current advertised
capabilities. Never forward arbitrary caller `cups-*` hints. Keep layout applied
by the producing application separate from device copies/options; actual consumer
compatibility remains a gate before the portal chooses those mappings.

Report a created job before streaming its sealed document. Report acceptance
only after final scheduler acknowledgement. On pre-commit failure, cancel that
known owned partial job through a fresh connection; a busy streaming connection
cannot reliably perform its own cleanup. Retain client cleanup after caller
cancellation and handle a noncooperative worker with a separate cleanup helper.
Retirement is idempotent for an already terminal owned job. Never automatically
retry ambiguous acceptance and never describe scheduler acknowledgement as
physical printing completion.

## Verification

Formatting, strict workspace/all-target Clippy and the original-graph all-target
build pass. Portal library tests pass249/8ignored and integration55/2ignored;
34 Print cases include descriptor, bounds, cancellation, token and ownership tests.
No new full-workspace test gate or full-workspace-green claim is made; the earlier
ten Files integration failures remain unresolved.

Nine native worker groups pass on the normal application509b1f20… against an
owned private CUPS scheduler and loopback-only synthetic printer. The receiver
gets the original603-byte PDF after its source is mutated, with two copies,
duplex and exact21000×29700 media dimensions. Four actual cancel/future-drop
rounds restore the warmed descriptor count to10 and retire their helpers. A
controlled CUPS-call stall exercises hard retirement: the created partial job
moves4→7 while the earlier accepted job stays9. All observed owned processes
are terminal and source/binary pins remain unchanged.

The raw queue correctly rejects unadvertised paper/duplex choices. The positive
fixture uses a PDF pass-through PPD with truthful paper/duplex capabilities, not
production document filtering. Earlier missing-media-field and rounded-geometry
fixture failures are preserved. The final paper check verifies the actual
structured media collection. Dynamic dependency inspection confirms no GTK and
no CUPS startup dependency on the portal executable.

Exact receipts live in `target/portal-print-30/` and
`target/portal-print-worker-native-05/summary.json`. Native fixture executables
use one checksum-addressed store rather than copies per case.

## Remaining acceptance

This does not prove backend SIGKILL/unacknowledged-create handling, real printer
filtering, toolkit layout/copy behavior, Iced consent, print-to-file publication or
the public Print wire flow. Those remain required work before registration and
advertisement. In particular, scheduler processing-time limits cannot simply be
treated as pre-commit approval leases; the investigation records the source
evidence and required crash tests. Print remains unfinished; the eleven packaged
and six experimental interface counts stay unchanged.
