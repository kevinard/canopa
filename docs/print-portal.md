# Print portal

Print is under development. A desktop build of `xdg-desktop-portal-canopa` can
register the interface with `--experimental-print`. The normal backend leaves it
disabled by default, and packaged portal selection does not advertise Print.
The switch is for explicit integration and native validation while the remaining
runtime, device and failure acceptance is completed.

The service uses Rust/Iced printer and file consent, shared Canopa appearance,
the Files Save picker and isolated document workers. It shares request admission
and Lockdown with the other interfaces. Application-prepared documents use a
one-use approval; direct documents are snapshotted before their Iced review.
File destinations require a fresh retained selection and explicit overwrite
consent. Prepared PDF, PostScript and single-side SVG file delivery preserves the
formatted body. The current native evidence includes real GTK applications as
callers; the Canopa implementation has no GTK startup dependency.

The staging handoff now avoids a false unconfirmed result when another helper
briefly retains a descriptor copy. The actual-worker regression and public Iced
file flows pass in both appearances; see
[staging completion](adr/0105-print-staging-stream-completion.md).

Printer consent exposes paper source, paper type, quality and resolution when
the selected queue advertises them. Saved queue and named-instance defaults are
restored. Device edits check the selected settings together; incompatible choices
remain editable and cannot enable Print. Custom paper must be checked with the
selected device options, including after restoring its saved size. The worker
checks support, driver conflicts and custom-size limits again before creating a
job. Prepared applications receive the selected quality and resolution as
rendering hints, including both resolution axes.

Initial application preferences now resolve against the selected printer's
current choices, including paper and printer instances. Missing remembered
printers require another destination choice; unavailable device preferences show
a notice and keep the queue default for that field. Initial hints apply once.
File output still requires a fresh selection. Both appearances pass scoped public and prepared-client checks; see
[initial preference ownership](adr/0093-initial-print-preferences.md).

Iced password prompts now serve scheduler sign-in and ordered device credentials.
Wrong-password retry, private credential reuse between prepared-print steps and
cancellation pass scoped checks in both appearances. Credentials stay with the
request and have no permanent Canopa store. Actual GTK and explicit Qt prepared
callers preserve six expected pages. The explicit Qt fixture isolates its PDF
renderer's own native CUPS lookup; automatic Qt printing remains deferred.
Automatic failure cleanup can reuse an exact approved login for the same account.
It cannot ask for a new password; unavailable credentials leave uncertain printer
records for review. Lost-reply checks pass in both appearances, preserving accepted
and unrelated jobs without sending the document again; see
[cleanup authentication](adr/0095-cached-credentials-during-printer-retirement.md).

A complete scheduler acknowledgement commits the submission before local receipt
bookkeeping. A later catalog error still reports acceptance and leaves the job
intact. Once the catalog is repaired, recovery preserves its received document.
A separate parent cleanup worker requires durable identity; a job number alone
cannot authorize it. Original live-request withdrawal retains its own checked
cancellation authority. See [accepted-job preservation](adr/0099-preserve-accepted-printer-jobs-after-bookkeeping-failure.md)
for the scoped validation and remaining limits.

Sign-in first required during Create-Job now preserves the correct recovery owner
before the scheduler can accept the job. Lost-reply and worker-death checks pass
against a private authenticated scheduler; see
[durable printer ownership](adr/0096-durable-owner-before-authenticated-creation.md).

A server can first ask for sign-in after creating the job. After an explicit
rejection, Canopa verifies the same empty job and can continue after approved
credentials, preserving its original owner. Direct/prepared, wrong-then-correct,
GTK/explicit Qt and cancellation checks pass in both Iced appearances. Lost
acknowledgements still never trigger automatic document resubmission; see
[rejected document continuation](adr/0097-rejected-print-document-continuation.md).

Explicit recovery can now request sign-in after Check and clean up, including
an approved account different from the recorded job owner. Default privacy,
wrong-password retry, cancellation and sixteen-record batches pass scoped checks
in both Iced appearances. Opening review or automatic cleanup cannot ask for a
new password; see [protected recovery](adr/0098-explicit-protected-printer-recovery.md).

An opt-in backend also serves a separate live printer-job review. Run
`xdg-desktop-portal-canopa --printer-jobs` while that backend is running with
`--experimental-print`, choose a printer and select a job that needs sign-in.
The Iced review uses the document already held by CUPS. Each attempt needs an
explicit Sign in action; wrong device credentials can hold the same job again.
Closing either review or password prompt preserves the received document.
The backend owns this review under caller lifetime, Request.Close and live
printing policy. It keeps source proofs privately and rechecks the job's UUID,
queue, visible owner/name, received state, protection and ordered fields before
release. It cannot create another job, upload its document or cancel a job.
An unconfirmed reply requires a fresh review. A release acknowledgement does not
establish physical printing. See [live printer-job ownership](adr/0100-live-printer-job-authentication.md)
for the scoped verification and remaining limits.

An optional private CUPS client now supports bounded longer credentials and
Digest compatibility fixes. For operator testing, build the pinned client described
in [the runtime recipe](../vendor/source-overlays/cups-authentication/README.md),
then set `CANOPA_PRINT_CUPS_LIBRARY` to its absolute regular-file path when starting
the opt-in backend. It freezes a sealed image until exit and loads it only inside
CUPS workers. A recognized capability permits 127-byte usernames and 1023-byte
passwords; the system fallback refuses usernames above 64 bytes or combined values
above 189 bytes before truncation. Unknown capabilities fail before authentication.
The release packager can include this client and its original notices; see
[runtime packaging](print-runtime.md) for the explicit option and installed layout.

The optional client also preserves quoted and backslash-containing authentication
fields. Independent Basic/Digest worker checks pass over loopback HTTP and private
TLS; quoted-field Iced flows pass in both appearances. See
[quoted authentication fields](adr/0103-quoted-printer-authentication-fields.md)
for the exact scheduler, workspace and device limits of this validation.

Basic/SHA-256 through Iced and actual GTK/explicit Qt clients pass scoped checks in
both appearances, including maximum ASCII/UTF-8 values and prepared credential
handoff. Selected-runtime Create/Send-only, explicit recovery and same-UUID held
jobs also pass scoped retry/TLS/lost-reply and Iced checks in both appearances.
Scoped checks cover idle prompts, private reply deadlines and cancellation, and
printer-to-file credential lifetime. Approval, helper exit and the complete
private reply share one 90-second deadline and caller lifetime; a reply retained
after approval cannot prolong that budget. See
[private reply ownership](adr/0102-print-authentication-reply-lifetime.md).
The actual outer 60/300-second worker budgets and repeated authenticated parent
descriptor cleanup pass controlled native checks; see
[worker supervision](print-supervision.md). Controlled early TLS checks now
distinguish a received rejection from a reply lost before client observation.
The optional private client also rejects incomplete headers before sign-in or
reconnect, avoiding a CUPS EOF loop; see
[incomplete HTTP replies](adr/0106-print-incomplete-http-replies.md).
Broader connection, device and deployment acceptance remain open. See
[private runtime ownership](adr/0101-process-private-print-authentication-runtime.md)
and [authentication ownership](adr/0094-request-owned-printer-authentication.md).

Both appearances pass the scoped advanced-control checks. The accepted IPP
request carries source, type and quality; the raster output contains four pages
at the selected 600 × 1200 DPI. The earlier PDF passthrough fixture does not
establish downstream resolution selection. Configured driver and physical-device
acceptance remain separate from a correct CUPS job handoff; see
[printer controls and constraints](adr/0092-printer-controls-and-driver-constraints.md).
The PDF-only continuation locates two remaining CUPS limitations: generated
driverless capabilities expose only the default resolution, and the configured
IPP backend drops an explicitly approved resolution before device delivery.
Diagnostic forwarding confirms that boundary but is not a shipped fix. See
[custom margins and the controls continuation](adr/0109-custom-media-bottom-margin.md).
The separately built optional IPP backend now preserves explicit resolution
through configured queues and stock filtering, with public Iced checks in both
appearances and a relocatable matching-client package. Using it still requires
explicit CUPS service selection; packaging changes no installed queue or service.
Driverless capability reduction and physical/raster resolution remain open. See
[matching IPP backend resolution](adr/0110-matching-ipp-backend-resolution.md).
The queue's advertised capabilities remain authoritative. CUPS 2.4.20's PDF-only
[PPD generator](https://github.com/OpenPrinting/cups/blob/v2.4.20/cups/ppd-cache.c#L4273)
uses the default resolution; its
[scheduler](https://github.com/OpenPrinting/cups/blob/v2.4.20/scheduler/printers.c#L4685)
reads PPD resolution choices as DPI. The versioned
[libppd 2.1.1 generator](https://github.com/OpenPrinting/libppd/blob/2.1.1/ppd/ppd-generator.c#L2254)
also retains a default and
quality presets without exposing a separate resolution choice. Changing the
worker's private client cannot change an installed scheduler's generated driver.
These are upstream queue compatibility limits; Canopa must not substitute raw
device attributes or relabel DPCM values to invent queue support. No scheduler
fork or queue-administration feature is introduced by this investigation.

Prepared PDF/PS/SVG printer streams are normalized to PDF even for one copy,
preserving their already formatted physical sides and printable annotations.
PDF copy preparation retains the original exact physical page sizes, including
duplex boundary blanks. Prior private PDF-only printer checks covered both
appearances, simplex, both duplex edges and collated/uncollated copies. The new
annotation/geometry checks use the actual isolated worker and stock filters.
Public frontend and normal-backend Iced checks also pass dark/100% and light/125%
prepared stamped PDFs, with one-copy simplex and two-copy long-edge delivery.
Prepared logical settings are not applied again. Prepared file output still
preserves exact source bytes.

Printer consent also supports custom paper when the configured CUPS destination
reports a custom range. Select Custom paper size, enter width and height in
millimetres with up to two decimal places, then choose Check paper size. Approval
requires a confirmed sheet and its printable margins. Changing dimensions or
sides requires another check. A saved custom `media` preference is resolved from
its CUPS spelling, and Printer default restores that concrete preference. Choosing
a fixed sheet shows its explicit name. The worker checks the approved dimensions
and margins again before creating a printer job.
The optional private client now preserves a declared custom range's bottom margin
independently of its top margin. This avoids rejecting valid legacy custom sheets
because of CUPS's 12.7 mm bottom fallback; approval still requires agreement with
the configured driver's geometry. Unequal-margin metadata and stock-filter
delivery checks are recorded in [the margin decision](adr/0109-custom-media-bottom-margin.md).

Direct output uses the approved paper setup and selected scale. A smaller sheet
at 100% can crop a larger source document; scale remains an explicit choice.
Prepared output preserves the application's already formatted physical pages,
including a toolkit's own page-size rounding. An earlier legacy CUPS path rounded
a 125 × 225 mm sheet to 124.88 × 224.71 mm downstream. The corrected worker now
passes scoped stock-filter driverless checks with exact dimensions, margins,
sides and visible content. The receiver's maintained source, configuration and
default loader pairing also pass verification. Broader device and physical-printer
acceptance remain open; see
[custom paper authority](adr/0091-exact-custom-printer-paper.md) and
[the device precision work](../.scratch/xdg-portals/issues/02-custom-paper-device-precision.md).
Cold fractional-size lookup now initializes CUPS's catalog before resolving the
explicit keyword, preserving requested hundredths even near A4. The actual worker
regression passes, including a duplex saved instance. Submission now retains an
explicit validated PPD PageSize and disables further filter scaling. Actual
stock-filter delivery passes fractional dimensions, both range limits, near-A4
paper, duplex and a saved instance. Prepared stamped PDFs also pass one- and
two-copy delivery without losing printable marks or page-size precision. The
earlier preset/A4 failures remain evidence; no system filter patch is installed.
See [paper submission and PDF appearances](adr/0107-exact-paper-and-print-appearances.md).
The [maintained receiver](adr/0108-maintained-ipp-receiver.md) uses a pinned
one-file source overlay and verified matching client configuration, with its
default loader bound to the checksum-retained library. It is an owned test
fixture and installs no service or queue.

`--recover-print` opens Print recovery with Files and Printers tabs. Discovery also
runs after normal backend readiness, including when only printer records remain.
Files can be copied to a fresh destination; the retained data stays in place.
Removing a file entry removes only its catalog record. Both normal printing and
review use the discovered recovery location, or the explicit
`--print-recovery-store` location. Manual review is a separate command mode and
does not require `--experimental-print`.

The printer review flow passes private native checks in both appearances. It
shows the recorded document, printer, server and known job number. An explicit
check can open a request-owned Iced sign-in form; closing it leaves unconfirmed
work available for another review. Credentials are not saved by Canopa. Check and clean up checks all
records and may cancel only freshly identified empty incoming jobs. A job with
a received document is preserved even when its final acknowledgement or local
receipt was lost. Previously acknowledged jobs are preserved, and documents are
never sent again.
Removing a printer record requires a separate confirmation: it stops automatic
tracking and frees one place in the recovery list, leaving the printer job
unchanged. Check the printer before printing another copy. Live workers and stale
confirmations cannot remove a record. A missing match stays unconfirmed until
cleanup can establish its outcome or the user explicitly removes the record.

The normal-service checks establish opt-in registration, both Iced appearances,
public prepared PS/SVG file output, pending-dialog withdrawal on shutdown, and
worker retirement after abrupt service death during a held document snapshot.
Printer success requires both the final HTTP and IPP success acknowledgement;
lost replies do not trigger automatic resubmission. Production printer submissions
also keep a bounded private intent journal. With a stable configured scheduler,
restart cleanup identifies incomplete jobs by their unique tag, user and queue,
including jobs whose creation reply was lost. Live workers and acknowledged jobs
are preserved; a historical numeric ID alone does not select a cancellation.
Separate local checks now kill the normal backend during reviewed file
replacement: after a completed stage, after claiming the old file, before and
after linking the new file, and after syncing the Published record. A real
Flatpak caller receives non-success when the service dies. Startup recovery
preserves the staged document, displaced original, old hardlink and any newer
unrelated destination. Iced offers fresh copy destinations without replaying the
interrupted replacement. Additional normal-service checks cover new files before
and after linking and incomplete staging for new and existing destinations.
Empty stages, an actual 65,536-of-80,516-byte partial copy, and complete PDF bytes
without a completion record stay unavailable for export after restart. Both
appearances pass; confirmed bodies can be saved through fresh Iced destinations.
These checks cover the recorded local SIGKILL boundaries. Power loss,
uninterruptible remote/kernel waits, cross-restart quotas and physical printing
remain separate.
See the [publication interruption fixture](../scripts/desktop-fixture/README.md#normal-service-publication-interruption).
Those earlier source-snapshot checks also did not establish real Flatpak identity.
Separate native checks now run a real Flatpak application through the standard
frontend and normal backend: prepared Print, direct Print and cancellation pass
in dark/100% and light/125%. The frontend forwards the authenticated application
ID, the sandbox cannot read a host-only sentinel, and successful calls deliver an
unlinked sandbox PDF FD to an Iced-approved host file. All four labeled pages
and the printable stamp are checked. The test uses a small GIO runtime assembled
from recorded host libraries, with automatic accessibility/Documents-portal setup
disabled; standard-distribution apps, Documents exports, printer delivery and
deployed Flatpak integration remain separate gates. The
[application-side fixture](../scripts/desktop-fixture/README.md#real-flatpak-print-caller)
records that scope. An additional real GTK3 consumer now exercises automatic
Flatpak routing without a `GTK_USE_PORTAL` override. Basic printing, a ranged/
reversed/two-up/75%-scale layout, cancellation and caller-loss withdrawal pass in
both appearances, with authenticated identity and a real exported Wayland parent.
Four delivered PDFs preserve the exact bytes rendered by the app; all twelve
pages have the expected sheet grouping, A4 geometry and marks. The backend and
consent helper retain Rust/Iced; GTK belongs only to the test application.
Its owned runtime adds the recorded GTK library/data closure and remains within
the shared 128 MiB fixture cap. Standard-runtime applications, printer devices,
Documents exports and deployed integration remain separate. See the
[GTK consumer fixture](../scripts/desktop-fixture/README.md#automatic-gtk-print-in-flatpak).
Further real Flatpak checks verify prepared-token ownership across another
connection in the same process, another process with the same app ID and a
different app ID. Foreign attempts require fresh Iced consent and leave the
original approval usable. Overlapping requests consume an approval only once.
Caller/frontend loss releases unused approval directory descriptors before the
next request. With the same app still connected, frontend restart and backend
SIGKILL/restart cannot revive old tokens; a fresh approval still prints. All
24 groups pass across both appearances, with six byte-identical four-page PDFs.
These are unused-approval and token checks, separate from interruption during
file publication and physical jobs. See the
[ownership fixture](../scripts/desktop-fixture/README.md#flatpak-prepared-token-ownership).
Optional runtime packaging and original
CUPS notices now pass local archive/native checks; release-workflow execution and
deployment remain open. Automatic Qt
dialogs, complete protected authentication, broader device acceptance and custom-device precision remain work. Production retirement now prevents libcups
from reconnecting during cancellation: a reconnect requires a fresh ownership
lookup, with three attempts before retaining uncertainty. Private scheduler-switch
checks preserve an unrelated reused job ID and permit a benign reconnect. This
assumes namespace continuity within an established connection; it does not cover
a proxy retargeting that same connection. Records without an identified matching
job remain retained for review. The
printer journal admits sixteen active/unresolved records and refuses new printer
submissions before creation when full. The six other experimental interfaces and final workspace/
package/release routing also remain open.

The installed Qt 6.12.0 Wayland QPrintDialog was also tested with the
xdgdesktopportal theme. It uses the Qt dialog and CUPS directly, with no Print
portal requests. The separate explicit Qt portal client reaches Iced and delivers
correctly. Automatic Qt client integration remains an open task; see
[the measured routing boundary](adr/0090-automatic-qt-print-routing-gap.md).

See [the service decision](adr/0084-opt-in-print-service-and-public-prepared-formats.md),
[prepared printer formats](adr/0085-prepared-printer-format-normalization.md),
[printer acknowledgement and crash audit](adr/0086-cups-final-http-acknowledgement.md),
[durable printer ownership](adr/0087-durable-printer-job-ownership.md),
[checked cancellation connections](adr/0088-checked-cups-cancellation-connections.md),
[printer outcome review](adr/0089-iced-printer-outcome-review.md),
[custom printer paper](adr/0091-exact-custom-printer-paper.md),
[printer controls and driver constraints](adr/0092-printer-controls-and-driver-constraints.md),
[the implementation map](../.scratch/xdg-portals/ROADMAP.md) and
[normal packaged interfaces](../packaging/portal/canopa.portal).
