## Phase55: automatic cleanup authentication scoped verified

The full18-interface goal remains active:11 packaged, six experimental, Print
opt-in. The user chose Canopa Print features first; automatic Qt integration is
deferred. No required user input or external blocker.

Automatic synchronous retirement now permits only exact approved credentials for
the checked account. New/rejected scopes cannot open another password dialog, use
prompt budget or become a user cancellation. Same-scope cleanup can still cancel
the exact owned job. Uncertain records stay available for review, with no document
retry and accepted/unrelated jobs preserved. See [ADR0095](../../docs/adr/0095-cached-credentials-during-printer-retirement.md)
and [scoped evidence](print/retirement-authentication-evidence.json).

The final source snapshot is11files/99014bytes; binary SHA256
26939bdff8a14aa8db6f46fb405098d8b1a7c8481ceff20308db91bae8d69e12.
Format, strict workspace Clippy and the original workspace/default/all-target/
offline/incremental/jobs1 compile graph pass.190 Print/12 Lockdown/408 Portal
library/55 integration plus explicit PPD runtime1 pass. Twelve fixture receipts
cover91 native cases and480 observed terminal processes, including both Iced
appearances, real GTK and explicit Qt callers and reconnect guards. Startup has
no GTK/CUPS/qpdf link. No new native binary, binary copy, dependency, runtime
profile or route. Phase free space gained184041472bytes;6GB reserve retained.
No complete workspace execution; ten existing Files integration failures remain.

The corrected real-worker seven-case matrix and four public Iced failure cases
pass. Full authentication11groups and ordinary printer6groups per appearance,
authworker14, controls27 and reconnect5 pass. All native handles are terminal;
no validation is running. The original red failure and intentionally stopped
candidate01 validation remain preserved. Native applications did not require a
debugger in this phase; prior external SIGTERM cause remains unknown.

Next is [actual account/job ownership](issues/06-authenticated-job-ownership.md).
Private read-only PAM/passwd namespace overlays now measure real scheduler
principal binding using a synthetic alias/password, with no host auth changes.
Metadata/Create auth changes job owner; Send-only auth preserves the original.
Lost Create acknowledgement after late login reproduces a wrong-owner durable
record and leaves the job held. Send-only worker gets explicit401, invokes its
credential callback and never issues authenticated Send. Raw authenticated IPP
control succeeds. CUPS source confirms Finish/GetResponse does not replay the
stream; no general retry may be added for uncertain acceptance. See
[measured investigation](print/principal-binding-investigation.json).

No fix for those two gaps is implemented yet. Build regression assertions from
principal-probe03/04/05 before editing. Replay sources are preserved under
`target/portal-print-55/replay/`; no live private services remain. The two earlier
setup failures remain failed. Separate owner from account authority, persist
before authenticated Create, freeze owner after creation, preserve legacy records
and exact checked cancellation. Finish explicit protected recovery and held-job
Authenticate-Job without document resubmission; Digest/long credentials and
native timeout/stall/file-output credential clearing remain required too.

Full Print still needs broader physical/driverless/Flatpak/device precision,
failure/resource/runtime/default packaging. Six experimental companions and final
workspace/native/release gates remain. No commit/PR or goal completion/pause/block.
