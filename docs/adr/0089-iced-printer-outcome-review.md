# Printer outcome review in Iced

Status: scoped private native review verified in both appearances. Print remains
opt-in and absent from packaged routing.

The bounded printer ledger intentionally retains missing matches, changed server
configuration and unconfirmed cleanup. These records can fill all sixteen places
even after the original request has ended. Automatic deletion would turn absence
into an unsupported success claim; automatic resubmission could print a duplicate.
Users need to inspect uncertain work and explicitly stop tracking a record.

`--recover-print` now opens a shared Print recovery window with Files and Printers
tabs. File snapshot/export/overwrite handling stays in the existing flow. Printer
review shows the recorded document title without its internal ownership tag, the
queue/instance, recorded server and acknowledged job number where available.
Records distinguish live workers, previously accepted jobs, unconfirmed outcomes
and unavailable process evidence. A printer-only list selects its tab initially.

Refresh reads metadata without contacting CUPS or claiming records. File and
printer catalogs are queried independently through the existing isolated worker
transport; a failure in one leaves the other usable. Normal startup performs its
existing exact-intent cleanup after D-Bus readiness, then opens review when either
catalog contains work or cannot be read. Printer-only records no longer disappear
from startup presentation because the file catalog is empty.

The separate Check and clean up action describes its scope across all printer
records. It uses the existing bounded retirement operation from
[ADR0087](0087-durable-printer-job-ownership.md) and
[ADR0088](0088-checked-cups-cancellation-connections.md): live workers stay busy,
accepted metadata is removed without cancelling the job, only exact identified
unaccepted jobs may be cancelled, and no document is sent again. A missing match
continues to occupy a place. Completion counts describe records, not physical
printer completion. Unconfirmed replies prompt a refresh and printer check.

Remove record is a distinct, confirmed metadata-only action. Its text explains
that the printer job remains unchanged, Canopa stops checking that record and
one place is freed. It also tells the user to check the printer before printing
another copy. In the confirmation, Keep and Escape preserve the record. Busy or unavailable evidence
disables removal; the UI never infers that an absent job was printed or cancelled.

Every displayed record carries a SHA-256 revision of its complete validated
receipt, including intent, endpoint/user/queue, job ID and accepted state. The
removal worker reacquires the existing exclusive catalog lock, locates the exact
random key, rechecks whole-worker liveness and compares the current receipt
revision before atomic bounded journal replacement. A changed receipt or a new
live cleanup claim rejects an old confirmation. This revision is stale-view
evidence, not printer mutation authority. It does not include document bytes or
authentication secrets. Other records and retained file data stay untouched.

The private protocol adds metadata listing and confirmed removal operations; no
portal interface, default advertisement, dependency or installed routing changes.
Existing worker admission, cancellation, confirmed retirement and shared appearance
remain in use. UI generations reject late completion and keep mutation state
visible until its reply or closure. Closing may leave the outcome unconfirmed, so
a later review reads the actual current journal rather than replaying an action.

Regular regression tests cover read-only listing, live-worker protection, changed
receipts and reclaimed records, independent catalog failure, separate confirmation,
stale completion and the file-copy flow remaining available after printer failure.
Formatting, strict workspace/all-target Clippy and original-graph compilation
pass, with 147 Print/12 Lockdown and complete Portal 365 library/55 integration
tests. Both actual Iced appearances pass metadata removal, Keep, busy protection,
a live claim during confirmation, missing-match retention/removal, printer-only
startup, cleanup scoped to an exact incomplete job, and retained-file copy/index
removal. The native fixtures use controlled valid records and four real CUPS
incoming jobs without document uploads; the accepted-record UI fixture does not
establish an actual final document acknowledgement or delivery.

Thirteen normal-service public crash/reply-loss groups and the five-case real
scheduler reconnect regression also pass. All 84 observed final fixture processes
are terminal, and the replay drivers exit normally. Narrow 580×440 action panes
and the corrected unavailable-file-catalog heading are visually checked in both
appearances. An earlier functional pass exposed a separate false-empty heading;
that failed visual receipt, exact source and screenshot remain preserved. The
light initial capture shows its honest loading state; settled accepted/startup
frames and completed interactions establish the loaded view.

Measured free space increased by 65,466,368 bytes over this phase, leaving
7,179,587,584 bytes free at the seal. The 6 GB reserve stayed intact. Root target
outputs and checksum-addressed fixture executables were reused, with no cache
cleanup, alternate graph, binary copies or new native executable. Exact storage
and source/binary pins are recorded in the evidence and checkpoint. These checks
do not establish full workspace test execution; ten Files integration failures
remain open.

See [the guide](../print-portal.md),
[review evidence](../../.scratch/xdg-portals/print/printer-review-evidence.json) and
[checkpoint](../../.scratch/xdg-portals/CONTINUE.md). This review does not establish
automatic Qt/live toolkit printing, advanced authenticated/physical devices, real
Flatpak, publication power loss, global resources or completed release packaging.
The six experimental interfaces and broader workspace/release gates remain active.
