# Request-owned printer authentication

Status: Iced credentials scoped verified; complete protected authentication/recovery remains open.

HTTP authentication for a CUPS scheduler and ordered per-job `auth-info` are
separate contracts. Application print settings cannot supply either credential.
The normal worker JSON protocol, command line, prepared reply and recovery
journal must never carry a password.

A worker can receive a separate inherited Unix stream. It sends bounded,
versioned, nonsecret challenges there and receives bounded binary credential
frames. Replies bind to the challenge number and expected field count. Malformed
frames, closure and cancellation stop the exchange. A refusal is remembered for
the entire operation: the installed library can invoke its callback again while
trying another internal listing/default request after the first refusal.

A real API probe established that CUPS closes the HTTP socket before invoking
its password callback. Treating that descriptor as a live peer rejected every
interactive challenge. The worker reconnects the same public HTTP object before
checking its actual peer and requesting credentials. A separate probe verified
that reconnect succeeds; the public implementation preserves the HTTP fields.
The corrected worker passes the retained native reproduction.

Each CUPS session retains its callback userdata and returned password until the
synchronous requests finish. It installs an explicit refusal callback when the
session ends. Passing a null callback would restore CUPS's console prompt, which
is unsuitable for a desktop service. CUPS remains loaded through thread cleanup,
as established by [ADR0092](0092-printer-controls-and-driver-constraints.md).

Challenges identify the configured server/port, current connection protection,
realm/resource or exact queue/instance and ordered fields. The callback accepts
only its retained HTTP object. Local Unix sockets, loopback TCP and established
TLS are distinguished. Nonlocal cleartext credentials are refused. Established
TLS must retain a required-encryption policy on subsequent library reconnects;
direct-TLS policy stays direct TLS. Certificate policy remains CUPS's policy.

Secret frames and values have a dedicated byte owner with redacted debugging and
`explicit_bzero` at destruction. The CUPS HTTP/IPP library also owns its request
copies until the isolated worker exits. Scheduler-owned credential storage is
outside Canopa's own storage; private validation removes its disposable scheduler
and credential files. No keyring or permanent Canopa credential cache is added.

The installed CUPS 2.4.20 public API was measured with synthetic credentials.
Its Basic encoding preserves a 189-byte `username:password` value but truncates
190 bytes and above. The current worker explicitly refuses that truncation.
Broader credential lengths remain a required compatibility follow-up, rather than
an undocumented change of the complete feature's scope.

The standard option-string encoder also changes a single escaped authentication
value containing punctuation. Multiple escaped values and literal public IPP
strings were measured separately. Authenticated Create-Job therefore uses literal,
ordered IPP text values and the selected scheduler's advertised URI/version.
It retains the fixed device-option whitelist, current driver check, journal,
physical-copy rules and final HTTP acknowledgement. Canopa does not retry an
uncertain creation or upload. Queue credentials and URI/version preparation precede the
intent journal. The URI supplies the resource on the retained connection and
cannot select another connection host.

Primary references are the [CUPS programming manual](https://openprinting.github.io/cups/doc/cupspm.html),
[authentication implementation](https://raw.githubusercontent.com/OpenPrinting/cups/v2.4.20/cups/auth.c),
[option encoder](https://raw.githubusercontent.com/OpenPrinting/cups/v2.4.20/cups/encode.c),
[public job implementation](https://raw.githubusercontent.com/OpenPrinting/cups/v2.4.20/cups/dest-job.c)
and [IPP authentication attributes](https://openprinting.github.io/cups/doc/spec-ipp.html).
The GTK 3.24.52 backend was inspected as a behavior reference, with its original
attribution retained in the source evidence; no GTK implementation is linked.

The Iced provider owns explicit authentication consent, cancellation,
request-scoped credential reuse and secure handoff across prepared printing. It
must bind reuse to the endpoint/protection space or queue/field order, invalidate
rejected credentials, preserve user edits and scope all prompts to the live
request. Caller loss, Lockdown, shutdown, timeout and helper retirement must close
pending prompts. Worker deadlines must account for bounded interactive waits.
Held jobs requiring corrected device credentials need identity-checked
CUPS-Authenticate-Job support without resubmitting the document. Long credentials,
protected recovery, field/endpoint changes and real device/Flatpak acceptance
remain open. See [the authentication follow-up](../../.scratch/xdg-portals/issues/04-authenticated-printing.md).

Print stays opt-in. This worker boundary does not establish complete authenticated
printing, default routing, automatic Qt integration or the full portal objective.

Scoped validation passes 173 Print tests, the complete Portal library/integration
selection, the explicit PPD runtime case, strict workspace Clippy and the existing
workspace compilation graph. Fourteen actual worker authentication cases and 39
ordinary worker/public-Iced regression groups pass; 109 observed final processes
are terminal. Both appearances were visually inspected. The native fixture uses
no recovery registration and cannot establish durable authenticated ownership.
See [the scoped receipt](../../.scratch/xdg-portals/print/authentication-evidence.json).

## Iced provider and prepared handoff

One small Iced dialog serves metadata queries and later document submission. It
shows the requesting document/application, scheduler or queue and connection
protection. Password input is masked; acceptance requires an explicit action.
The helper returns only an approval bit on ordinary stdout and a bounded binary
credential frame on its inherited private socket. Edited values and keyboard
messages have redacted debugging; owned secret values are cleared when dropped.
Iced's own temporary text-input allocations are not permanent credential storage.

A request-local broker serializes prompts and caches only exact endpoint,
transport, realm/resource or queue/ordered-field scopes. Rejected values are
invalidated; the edited username survives a retry. Six interactive prompts are
separate from the worker's64 bounded exchanges, so cached replies do not consume
another user prompt. Switching to file output clears credentials and invalidates
late prompt answers. Outer supervisor cleanup has no interactive provider. Synchronous partial-job
retirement now restricts the live channel to exact approved scopes and the checked
account; new or rejected credentials cannot open another prompt. Native fault
checks pass in both appearances; see [ADR0095](0095-cached-credentials-during-printer-retirement.md).

Prepared printing retains its small cache through a sealed anonymous memory
file transferred over the existing private approval socket. Public Grant JSON,
application settings, tokens and recovery records carry no passwords. The existing
one-use sender-bound token owns the approval's private cache until use/expiry.
File grants use their original retained destination descriptors and receive no
printer cache. Restored caches preserve the request's prompt count.

Interactive CUPS operations have a finite300s budget; a prompt is limited to90s,
within the worker channel's120s exchange deadline. Request cancellation stops the
prompt before worker retirement. Cache/handoff/form tests and eleven actual public groups in each appearance pass. It does not complete protected
recovery, late authentication ownership, held-job authentication, Digest or long
credential compatibility, and does not change default Print routing.

The phase54 evidence includes184 Print tests and75 native worker/public groups,
including actual GTK and explicit Qt prepared documents with six expected pages.
The Qt PDF fixture isolates its own native CUPS lookup; automatic Qt integration
remains open. Full light acceptance ran with a debugger on the Python controller,
with application children detached. Earlier interrupted/failed receipts retain
their status. See [Iced evidence](../../.scratch/xdg-portals/print/iced-authentication-evidence.json).

## Actual scheduler account investigation

Private read-only PAM/passwd namespace overlays now verify actual scheduler
authentication with a synthetic account/password; host authentication files are
untouched. Metadata/Create login changes the job owner, while Send-only login
preserves its original owner. A lost Create acknowledgement after late login
reproduces a wrong-owner durable record. Send-only authentication reproduces an
explicit401 without an authenticated document send; no ambiguous operation was
retried. The Create-time owner mismatch is now scoped verified in [ADR0096](0096-durable-owner-before-authenticated-creation.md).
Send-only authentication and complete protected recovery remain open. See [the investigation](../../.scratch/xdg-portals/print/principal-binding-investigation.json)
and [required work](../../.scratch/xdg-portals/issues/06-authenticated-job-ownership.md).
