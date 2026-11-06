# Continue a rejected document on its owned job

Status: scoped native verified; Print opt-in, compatibility and full authentication gates remain.

Actual CUPS PAM probes reproduce Send-only authentication. The worker receives401
after its first document stream; the public finish helper authenticates/reconnects
but does not send again. During the challenge the scheduler reports an incoming
job with zero documents. A raw authenticated control accepts exactly one document.
Creation does not return job UUID even when requested. Different nonadministrator
credentials can legitimately send while default privacy hides name and owner;
the read-only job UUID, queue, state and document count remain visible. A separate
owner-required control rejects that account with403. No permissions are changed
to make the worker pass.

One submission module now owns checked transmission, explicit HTTP result parsing,
job proof and bounded authentication continuation. The initial Create still has
its durable tag/owner and one positive ID acknowledgement. Every submission has
an unpredictable tag, including private helpers without a persistent store.

The module reads the final HTTP/IPP result directly. It does not call the CUPS
finish helper which would hide the401 while authenticating. A lost/invalid final
reply or failed write after data began cannot retry. An explicit401 may continue.
A denied reconnect before any document bytes can also continue only after an
authoritative empty-job check. CUPS still builds Basic/Digest/Negotiate credentials;
Canopa adds no authentication encoder or permanent credential cache.

Before changing principal, a separate protected session reads the exact tagged
job as its recorded creation owner. It must match ID/tag/owner/queue and be
incoming with zero documents. Its UUID, if supplied, becomes durable before a
credential can permit continuation. Legacy receipts omit this optional field and
retain their existing serialization. Known IDs and worker ownership gate binding;
a different UUID or accepted receipt cannot be rebound. It also changes the
receipt revision, invalidating old record-removal confirmations.

The rejected HTTP object's real challenge is retained while that sibling query
runs. The sibling's thread-local callback registration is then replaced with the
original retained callback. CUPS performs authentication outside checked document
transmission. Each rejected challenge uses a fresh protected HTTP context with
that actual challenge, forcing its empty CUPS password cache to consult the broker.
The worker attempt count carries over. Successful unauthenticated metadata reads
otherwise reset CUPS retry counters and keep a rejected password cached; the
actual wrong-once regression exposed this before correction. A fresh read on the
actual transmission connection verifies the same empty job. A UUID previously
bound to exact tag/owner permits privacy-hidden fields;
visible conflicts still fail. Without UUID, all exact fields must remain visible.
The immutable creation owner does not become the later HTTP account.

Document transmission cannot reconnect internally. Password callbacks there defer
the challenge without opening Iced or recording a user cancellation. No more than
three document attempts are allowed, and existing prompt/exchange/deadline bounds
remain. Driver options are checked again before continuation. Accepted and uncertain
jobs are never handled as empty rejected jobs; held-device correction remains a
separate Authenticate-Job action, never a document resend.

The existing connection guard moves beside submission and recovery. Its instance
is owned by the pending job and reused by synchronous retirement, through all
cleanup IO. Its seccomp listener cannot be dropped before cleanup: the installed
filter remains on that thread. No nested guard or detached supervisor is added.
Automatic retirement still uses approved credentials only and does not switch
to another account silently; explicit protected recovery remains required.

The frozen candidate passes 195 Print checks, 413 Portal library checks serially,
55 integration checks, 12 Lockdown checks and the explicit PPD runtime case.
Formatting, strict workspace Clippy and original workspace compilation pass.
All 140 native groups pass on the same binary. Nineteen actual PAM Send cases cover
same/different nonadmin accounts, default-hidden metadata, wrong/declined
credentials, HTTP 403, early HTTP 401, lost final acknowledgement, changed identities,
missing UUID, failed proof persistence, worker death and four TLS variants.
Twelve prior creation-owner PAM cases also pass. Real Iced direct/prepared and
GTK/explicit Qt callers pass in both appearances, including wrong-then-correct,
withdrawal and shutdown; toolkit rendering preserves six expected pages.
Existing controls, driver/raster, retirement and cancellation reconnect checks
pass. Images were inspected; all 721 observed fixture process identities are terminal. The
binary has no startup GTK/CUPS/qpdf linkage. See
[scoped evidence](../../.scratch/xdg-portals/print/rejected-send-evidence.json).

Failed evidence remains failed. The earlier candidate reused a rejected password
across three HTTP 401 results; the corrected candidate passes that wrong-once case.
The original parallel library run fails a staging-handoff EOF assertion with
EAGAIN; unchanged isolation and serial runs pass, but the timing cause remains
unresolved. The initial dark UI wrapper ended by SIGTERM after eight groups.
Its recorded services and private data were retired. A signal-traced repeat and
both untraced appearances pass all nine groups; the signal sender remains unknown.
No complete default-parallel or workspace execution green result is claimed.

Nonreusable connections, unavailable identity metadata and actual Send namespace
replacement need compatibility evidence before default Print routing. Identity
mutation and cancellation reconnect tests do not establish those gates. Explicit
protected recovery, held-job Authenticate-Job, Digest and longer credentials
remain required. No physical/driverless/Flatpak or complete packaging acceptance
is implied. This phase reused the root build graph and native stores; no
dependency, native executable or binary copy was added.

Source evidence is in the CUPS [finish/stream interface](https://raw.githubusercontent.com/OpenPrinting/cups/v2.4.20/cups/dest-job.c),
[response handling](https://raw.githubusercontent.com/OpenPrinting/cups/v2.4.20/cups/request.c),
[authentication](https://raw.githubusercontent.com/OpenPrinting/cups/v2.4.20/cups/auth.c)
and [job attributes](https://raw.githubusercontent.com/OpenPrinting/cups/v2.4.20/scheduler/ipp.c).
See [issue06](../../.scratch/xdg-portals/issues/06-authenticated-job-ownership.md),
[creation owner](0096-durable-owner-before-authenticated-creation.md) and
[cache-only cleanup](0095-cached-credentials-during-printer-retirement.md).
Print stays opt-in; the full18-interface goal and remaining authentication work
remain active. Automatic Qt integration is deferred.
