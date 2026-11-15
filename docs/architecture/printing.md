# Printing architecture

This document covers the experimental Print portal in `crates/canopa-portal`:
request handling, isolated workers, file publication, CUPS submission, printer
job ownership, recovery and the private CUPS client. For the user and packager
view, see [Printing](../printing.md). For the shared backend structure, see
[Portals architecture](portals.md).

Earlier decision records were consolidated into this document; the originals are
in the git history at commit cb300936 under docs/adr/.

## Components

| Part | Role | Source |
| --- | --- | --- |
| Request controller | `org.freedesktop.impl.portal.Print` (PreparePrint, Print), admission, Lockdown, tokens | `print/service.rs`, `print/service/`, `print/tokens.rs`, `print/authority.rs` |
| Printer-job review | Private `org.canopa.Portal.PrintJobs` interface behind `--printer-jobs` | `print/service/jobs.rs`, `print/jobs_review.rs`, `print/live_jobs.rs` |
| Production adapter | Connects the Iced presenter and the shared worker client | `print/production.rs`, `print/presentation.rs` |
| Worker client | Spawns and supervises workers, budgets and retirement | `print/worker/client.rs`, `print/worker/client/` |
| Document and CUPS worker | `--print-worker`: reads documents, talks to CUPS | `print/worker.rs`, `print/worker/document.rs`, `print/worker/cups/` |
| Layout and converters | `--print-layout-worker` and external converters under bubblewrap | `print/render.rs`, `print/render/` |
| File destinations | Retained file pins, staged publication, recovery catalog | `print/file_target.rs`, `print/file_target/` |
| Printer-job ledger | Durable printer submission intents | `print/printer_jobs.rs`, `print/file_target/publication/recovery/jobs.rs` |
| Authentication | Request-owned credential channel and prompt | `print/authentication.rs`, `print/authentication/` |
| CUPS runtime | System or private `libcups` selection | `print/worker/runtime.rs` |
| Startup recovery | Cleanup and review at backend start | `print/recovery_host.rs` |
| Dialogs | `--print-dialog`, `--print-authentication-dialog`, `--print-recovery-dialog`, `--print-jobs-dialog` | `print_ui.rs`, `print_authentication_ui.rs`, `print_recovery_ui.rs`, `print_jobs_ui.rs` |

Paths are relative to [`crates/canopa-portal/src/`](../../crates/canopa-portal/src/).
`main.rs` registers Print and PrintJobs only with `--experimental-print`, which
also requires a usable recovery store. `RecoveryHost` runs at every backend start,
with or without the flag.

## Request controller

Print uses the shared request admission and Close handling of the backend.
Parse and policy errors are D-Bus errors; cancellation returns 1 and runtime
failure returns 2. The Lockdown `disable-printing` property is checked before the
dialog and again before delivery. A denial is latched by policy revision, so a
later allow cannot revive an approval granted before it. `disable-save-to-disk`
removes file destinations.

Caller identity comes only from the current owner of `org.freedesktop.portal.Desktop`
and the canonical request path, bound to the bus GUID, sender, original client
and app ID (`authority.rs`). Losing that owner revokes everything it authorized.

PreparePrint stores a backend-owned approval, including any live file pin, under
a random nonzero 32-bit token (`tokens.rs`): at most 64 tokens, 8 per caller, each
valid for 300 seconds and usable once. Print removes the token before any other
work. An unknown, expired or revoked token always opens a fresh dialog. The
settings returned to the application are only hints; the approval is the
authority.

## Workers and isolation

The backend never reads an application document or calls CUPS itself.

- **Document and CUPS worker** (`--print-worker`). It inherits only explicit
  descriptors. It copies the document into a sealed memfd of at most 128 MiB,
  reading regular files positionally (the caller's offset is untouched) and
  polling streams together with a cancellation pipe. Review and delivery see the
  same immutable body.
- **Layout and converters** (`render/runner.rs`). Each converter runs under
  `bwrap` with new namespaces, no capabilities, as PID 1 of its own PID
  namespace and with `--die-with-parent`. Only the sealed input, a fresh output
  memfd and a diagnostic pipe cross into it, with a 256 MiB `/tmp`. The
  pipeline is: Ghostscript (`pdfwrite`, with printable annotations flattened) or
  `rsvg-convert` normalizes the input to PDF; the Rust layout worker
  (`--print-layout-worker`, using qpdf's C API) builds the sheets; Ghostscript
  (`ps2write`) or `pdftocairo` produces PostScript or SVG output. The layout
  helper is executed through a held `/proc/self/fd` handle of the backend
  executable, so an upgrade that unlinks the binary cannot swap it.
- **Dialogs.** The Iced dialog receives catalog and capability data from the
  worker and returns only an explicit grant; Enter alone never prints. Results
  are generation-checked so stale printer data is discarded.

### Supervision

One shared `Client` limits the backend to 2 retained documents and 2 active
operations, plus 1 derived body per source.

- Ordinary operations have a 60-second budget; operations that may prompt for
  credentials have 300 seconds. When a budget expires, the client closes the
  cancellation pipe and withdraws authentication.
- Caller cancellation, a dropped future and backend shutdown close the same
  cancellation pipe.
- A worker that does not stop within 10 seconds is killed. If its exit is not
  confirmed 2 seconds later, the caller is told the result is unconfirmed, but a
  detached task keeps the child, its permits, body and file pins until exit is
  confirmed. Settlement, publication and partial-job cleanup wait for that
  confirmation. An observation error never counts as proof of exit.
- Shutdown waits at most 15 seconds for workers, then logs the pending count.

Descriptor rules:

- Workers inherit only the descriptors passed to them, re-marked close-on-exec.
- Converters keep only standard I/O and their explicit inputs.
- The sealed CUPS client image goes only to CUPS workers and the metadata
  helper; credential dialogs never receive it.
- Credentials travel only on the private authentication channel, never in worker
  JSON, argv, settings, tokens or journals.
- After each operation the backend returns to its own descriptor baseline with
  no leftover helpers.

## Copies and layout

For a printer, the application is always told `n-copies=1`; the approval keeps
the real copy count and collation, and CUPS always receives `copies=1`. Canopa
makes physical copies itself so that front and back pages stay paired: in
uncollated duplex output each front/back pair repeats, and an odd-sided document
gets a blank back between copies.

Direct documents are laid out in Rust in this order: ranges (in the order
given, repeats kept), pages per sheet, page set, reverse, copies. Prepared
documents (laid out by the application, for example GTK's prepared PDF) are never
laid out again. Prepared PDF, PostScript and SVG printer bodies always go
through copy-only conversion to PDF, even for one copy, keeping each page's exact
geometry and printable annotations, because PDF-only queues reject PostScript and
SVG. Print to file of a prepared document with one copy keeps the application's
bytes exactly.

## Printer settings

The worker returns bounded catalog data (256 printers, 128 choices per option)
and leaves missing attributes missing. Printer controls are fixed: paper source,
paper type, quality and resolution (both axes, DPI or DPCM); there is no
arbitrary option editor. Checks use IPP support first, then the queue's PPD
(at most 1 MiB, held in a sealed memfd; no driver code runs). Every device edit
is rechecked, and the worker checks again before recording the job intent and
creating the job (`worker/cups/validation.rs`, `worker/cups/ppd.rs`).

Custom paper is held in hundredths of a millimetre with no silent rounding
(`worker/cups/media.rs`). The user must check a size with the worker before
approval. The worker initializes CUPS's media catalog before resolving a name,
compares dimensions explicitly and parses saved PPD spellings. Submission sets
`print-scaling=none`, names the exact page size and compares all four margins
again before Create-Job.

Application preferences are resolved against the selected queue through a fixed
list of keys (`worker/preferences.rs`). A value the queue does not offer keeps
the queue default and shows a notice. Hints apply once.

## File destinations

The Files save dialog returns `O_PATH` descriptors for the parent directory and
any existing entry, passed over a one-shot socket (`file_target.rs`). The
overwrite warning therefore applies to the exact object reviewed; a new entry
cannot turn consent to create into consent to replace. The backend only checks
the descriptors' shape; workers revalidate them before the token is issued and
again before delivery, so backend threads never touch a stalled filesystem.

Output is staged in a private random-named `0700` directory with a `0600` body
and journal (`file_target/publication.rs`). A new file is published by linking
the staged body into place through `/proc/self/fd`. A replacement moves the old
file aside with `RENAME_NOREPLACE` after a SHA-256 completion record. The
journal records Publishing before the link and never resets it. Only a positive
receipt proves success; otherwise both bodies are kept for recovery. This needs
procfs and hard links on the destination filesystem, and there is no rename
fallback. The setup worker shuts down its write side after sending the bundle so
a stray inherited copy cannot delay end-of-file.

## CUPS submission and job ownership

A job counts as accepted only with HTTP 200 and an IPP success status read
directly from the final reply (`worker/cups/submission.rs`). The outcome is
committed in memory before any bookkeeping write, so a failed local write still
reports acceptance and leaves the job alone. A lost or ambiguous reply is
reported as uncertain and the document is never sent again automatically.

Before Create-Job, the worker persists an intent with a 128-bit random tag that
also prefixes the job name, plus the endpoint, encryption, user and queue
(`printer_jobs.rs`). The ledger `printer-jobs.json` in the recovery store holds
16 records in 64 KiB; when full, new printer jobs are refused before creation.
Recovery matches tag, user and queue exactly, never a numeric job ID alone.
Cleanup cancels only a freshly identified incoming job with no received document.
Live withdrawal (Close, caller loss, Lockdown) may cancel the request's own job
before acceptance.

During cancellation a seccomp user-notification supervisor rejects new
`connect` calls (`worker/cups/connection.rs`). If libcups reconnects, the job is
looked up again; after three changed connections the record stays unresolved.
This keeps a cancellation from reaching a different scheduler that reuses the
same job ID.

## Authentication

Challenges and credentials travel on a separate inherited socket, never in
worker JSON, argv, settings, tokens or journals; secrets are cleared after use
(`authentication.rs`, `authentication/`). Credentials to a non-local host
without TLS are refused. A request has at most 6 prompts, 8 cached scopes and 64
exchanges. Approval, helper exit and the complete reply share one 90-second
deadline inside the 120-second channel deadline and the 300-second operation
budget. Prepared printing hands its credential cache to the next step in a
sealed memfd of at most 96 KiB. Choosing file output clears credentials.

- **Owner before authenticated creation.** When Create-Job needs sign-in, the
  login name is persisted as the provisional owner before the password reaches
  CUPS, because an HTTP login takes precedence over `requesting-user-name`. A
  failed write refuses the job.
- **Rejected document.** After a 401 on Send-Document, the worker continues only
  on a proven incoming job without a document (tag, owner, queue and UUID when
  available), with at most three document attempts.
- **Cleanup never prompts.** Automatic and startup cleanup reuse only an exact
  cached scope (and, for server sign-in, the same account). Recovery opens a new
  prompt only after the user chooses Check in the review. The approved HTTP
  account stays separate from the recorded job owner.
- **Held jobs.** `--printer-jobs` calls PrintJobs.Review on the running backend.
  One review runs at a time under the same caller, Close and Lockdown rules as
  Print. It verifies that the job holds a received document and is waiting for
  authentication, then sends only CUPS-Authenticate-Job for the same job UUID. It
  never creates, uploads or cancels a job.

## Recovery

The recovery store defaults to `$XDG_STATE_HOME/canopa/portal/print-recovery`
(`file_target/publication/recovery.rs`). An intent is recorded before staging.
The catalog holds 16 entries in 2 MiB, 14 for normal work and 2 reserved for
recovery exports, under an advisory lock, and identifies processes by boot ID,
PID and start time. Discovery classifies entries but never replays, restores or
deletes. At startup `RecoveryHost` first retires printer records it can resolve,
then opens the review if any file or printer entry remains.

The review exports a SHA-256-verified snapshot to a freshly chosen destination
outside the retained folder; removing an entry removes only its catalog record.
Removing a printer record is metadata only and requires confirmation.

## CUPS runtime

`Runtime::configured()` chooses the client once per backend lifetime:
`CANOPA_PRINT_CUPS_LIBRARY` if set, otherwise `PREFIX/lib/canopa/print/libcups.so.2`
next to the executable. A missing bundled file means the system library; a
present but invalid choice is Unavailable with no fallback. The image (at most
16 MiB) is copied into a sealed memfd and mapped only by CUPS workers. The fork
exports `canopaCupsAuthenticationVersion()` = 1, which raises the credential
limits to 127 bytes for the username and 1023 for the password; with the system
library, usernames over 64 bytes or `user:pass` over 189 bytes are refused before
CUPS can truncate them. The fork also preserves quoted authentication fields,
fixes the custom-media bottom margin and rejects incomplete reply headers.
libcups stays loaded for the life of a worker because its thread-exit callbacks
crash after unloading.

The matching IPP backend (`lib/canopa/print/ipp`) forwards an approved
`printer-resolution` that the stock backend drops. It requires the matching
client's capability and cannot fall back to stock CUPS. It is used only by a
queue an administrator configures for it. A test-only IPP receiver is built from
the same fork with `scripts/build-print-ipp-receiver.py`.

## Testing

Library and D-Bus tests cover the controller, tokens, workers and publication
(see [Testing](../testing.md)). Native and packaging checks:

| Script | Covers |
| --- | --- |
| `scripts/test-print-supervisor.py` | Worker budgets, cancellation and descriptor ownership (`--mode controls`, `resources`, `deadlines`) |
| `scripts/test-print-dialog.py` | Dialog draft and printer discovery |
| `scripts/test-cups-authentication.py` | Basic and Digest handling of the private client |
| `scripts/test-print-ipp-receiver.py`, `test-print-ipp-backend.py` | Receiver and matching backend |
| `scripts/test-print-runtime-package.py`, `test-print-backend-package.py` | Packaged runtime and backend |

## Design decisions

**Opt-in service.** Print is registered only with `--experimental-print` and is
not in the packaged descriptor, so GTK keeps serving Print until the remaining
work is done.

**Approvals are tokens held by the backend.** Returned settings and URIs are
hints; only a one-use in-memory approval authorizes printing. A guessed or
recycled token opens a new dialog. Approvals do not survive a restart.

**Isolated workers for every untrusted operation.** Documents can change under
the caller's descriptor, pipes can stall and CUPS calls can block, so reads and
CUPS calls run in workers and converters run in bubblewrap. The cost is a
runtime dependency on bubblewrap, Ghostscript, qpdf, librsvg and Poppler.

**Retire workers only after confirmed exit.** A signal does not prove a process
has exited, so slots, pins and bodies stay owned until exit is confirmed. A stuck
worker therefore keeps its slot.

**Canopa makes physical copies.** A printer applying duplex cannot keep
front/back pairing across copies, and GTK bakes layout into its PDF. CUPS always
gets one copy.

**Normalize prepared printer bodies to PDF.** PDF-only queues reject PostScript
and SVG, and printable stamps were lost. Exact page geometry is kept.

**File consent pins kernel objects.** A path or inode number cannot hold the
reviewed object, and network filesystems can lose a reply after a link or
rename. Retained descriptors, a staged journal and positive receipts make an
interrupted save recoverable instead of destructive.

**Recovery is evidence, not authority.** After a restart the journal is reviewed,
never replayed. Exports go to new destinations chosen in the dialog.

**Acceptance requires the final acknowledgement, and nothing is resent.** A
transport loss between acceptance and acknowledgement must not print twice or
cancel a received document, so uncertain outcomes are reported, not retried.

**Durable job ownership by tag.** A lost Create-Job reply leaves no job ID, and
job IDs are reused, so jobs are identified by a persisted random tag with user
and queue. The fixed ledger size bounds unresolved work.

**Checked cancellation connections.** A reconnect inside libcups could reach a
replacement scheduler, so cancellation looks the job up again on every new
connection. This is not an atomic conditional cancel against a hostile server.

**Credentials belong to the request.** Nothing is stored; cleanup never prompts
because it must not ask the user for a new decision. Recording the login owner
before an authenticated Create-Job keeps recovery able to find the job.

**Live held-job sign-in reuses the document CUPS holds.** The original request
has finished and the job belongs to CUPS, so the review only authenticates the
existing job.

**Process-private CUPS client.** Credential limits and protocol fixes need a
modified libcups, but the scheduler and filters must keep the system library.
The private image is loaded only inside workers and selected once per process.

**Fixed printer controls.** A free-form option editor would bypass driver
constraints; four fixed controls are checked against IPP and the PPD.

## Known limitations

- Print is opt-in and not in the packaged descriptor; GTK serves Print by default.
- Qt applications that use `QPrintDialog` never call the Print portal.
- Driverless and PDF-only queues expose only the device's default resolution, and DPCM resolutions do not reach the dialog through legacy queues.
- The stock CUPS IPP backend drops an approved resolution; the matching backend helps only for queues configured to use it.
- The system `libcups` fallback has lower credential limits, mishandles some Digest and quoted fields and can stall on malformed replies until the worker budget expires.
- The printer-job review lists jobs of the effective CUPS account only, so jobs created under another account can be hidden.
- Cancellation is not an atomic conditional Cancel-Job against hostile servers or retargeting proxies.
- Retained workers are not bounded across backend restarts: with workers stuck in uninterruptible I/O, each restart adds its own set (each holding a sealed PDF memfd), because admission limits are per process and the source-snapshot phase has no shared reservation; shutdown then waits its full 15 seconds.
- A helper launch can block when the child must close an inherited application descriptor on a stalled filesystem before `execve`; other portal calls can wait behind it.
- Converter descendants, runtime disposal and power loss are outside the worker retirement guarantees.
- The private recovery directory does not protect against another process running as the same user.
- The Print, authentication, recovery and printer-job dialogs publish no accessibility tree.
