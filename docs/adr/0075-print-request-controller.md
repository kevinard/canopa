# Print request and approval controller

Status: scoped controller/policy and GTK3 rendering checks passed. The normal backend
does not register or advertise Print. The production rendering/delivery adapter
is still required, along with the remaining Print acceptance gates.

The Print model, retained Iced consent and isolated document/destination workers
previously existed separately. `print::Print` now implements the backend
PreparePrint and Print methods, using the shared Requests admission/Close owner
and the authenticated original-client proof from ADR0070. Its injected Backend
port separates review, bounded document snapshot, delivery and shutdown. This
is the request controller, not a substitute for the unfinished renderer.

The exact contract follows the inspected canonical frontend1.22.1 XML and the
[official backend reference](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.impl.portal.Print.html).
PreparePrint receives the request handle, app/window/title, settings, page setup
and options, then returns two outputs: response and results. Print additionally
receives a Unix descriptor and token options, with the same two output shape.
The backend contract has no version property. Request bodies are bounded before
interpretation by the controller; settings/options then pass the existing typed
model validation. Parse/identity/policy errors remain D-Bus errors; cancelled
operations return1 and runtime failures return2, both with empty results.

PreparePrint returns only canonical known layout/page-setup fields and the approved
file format when applicable. Caller printer/output/CUPS hints are not forwarded
as authority. The backend retains the actual owned Approval, including its live
Pinned destination, in the one-use token store. A guard owns an unreturned token
through shared Request cleanup; cancellation or failed result construction drops
that exact issuance and releases its retained objects.

Guards and asynchronous token retirement use a distinct live issuance identity,
in addition to the public32-bit token. A consumed/expired token number may later
be generated again; cleanup from an older issuance must not remove the new one.
Both the guard and expiry/owner/policy retirement check that identity. The store
also checks auxiliary cancellation during admission, sweeping and consumption,
so it does not depend solely on its retirement task being scheduled promptly.

Print consumes a matching token before asynchronous work. Unknown/revoked tokens
require direct review; they never choose a default destination. The Backend
snapshots the received descriptor before direct review so that review/delivery
refer to one immutable body. Prepared file output must match its approved format,
and its held destination is checked again before delivery. The destination
worker still rechecks at the actual publication boundary under ADR0074.

Delivery carries the complete Approval and Frozen body, with an explicit
Prepared/Direct rendering value. It does not silently map all returned layout
settings to CUPS. GTK's portal client renders through its file backend using
manual page selection, copies and layout; see its primary
[portal client](https://raw.githubusercontent.com/GNOME/gtk/gtk-3-24/gtk/gtkprintoperation-portal.c)
and [file backend](https://raw.githubusercontent.com/GNOME/gtk/gtk-3-24/modules/printbackends/file/gtkprintbackendfile.c).
Actual GTK/Qt transform/copy compatibility and direct-document rendering remain
necessary before the production adapter or normal registration can be completed.

Lockdown now exposes a read-only Rust snapshot of its same committed policy.
Each restriction remembers denials with a revision, including rapid deny/allow
changes coalesced by a watch receiver. Earlier permits cannot revive after that
transition. Revisions are local to the backend lifetime; prepared tokens are also
volatile. Counter exhaustion stays unavailable until a new backend lifetime.
The exported boolean properties, names, persistence and relay remain unchanged.

Print checks Printing before presentation/snapshot and again before delivery.
SaveToDisk removes file choices when initially denied. If saving is revoked
while a dialog offers file output, that review retires; after preparation only
file approvals retain the saving restriction. Printer approvals survive unrelated
save-policy changes. Owner loss, relevant policy denial/source closure, request
cancellation and backend shutdown feed an irreversible cancellation scope. Its
Drop signals independently owned worker cleanup before aborting the scope monitor.
Shutdown clears prepared objects and asks the Backend to drain its active work.

Wire tests register this actual controller on a private bus with a fake Backend
port. They exercise real D-Bus signatures, descriptor passing, identity proofs,
policy mutations and retained filesystem pins. Their fake renderer is deliberate:
these tests do not establish current Iced-to-public-Print, CUPS, physical printer
or toolkit-rendering acceptance. The small sealed-body factory exists only under
cfg(test). No new executable, dependency, installed routing, service or queue is
introduced by this controller. The broader eleven-packaged/six-experimental/Print
unfinished roadmap and earlier ten Files failures remain open.

The current source passes formatting, strict workspace/all-target Clippy, the
original all-target build, all83 Print tests and12 Lockdown tests. Full Portal
regressions pass301 library/11ignored and55 integration/2ignored. Eleven new wire
cases cover the controller; three token and two policy cases cover lifecycle
details. Receipts are `target/portal-print-34/acceptance-01.json` and
`.scratch/xdg-portals/print/controller-evidence.json`. No new full workspace test
gate ran; the preceding ten Files failures remain unresolved.

A separate actual GTK3.24.52 application probe uses a private fake public frontend
on the retained nested compositor. Six groups capture its received PDF rather
than merely observing draw callbacks: four default pages; eight collated copies
in1,2,3,4,1,2,3,4 order; eight uncollated copies in1,1,2,2,3,3,4,4 order; zero-based
ranges1–2 producing source pages2–3; four logical pages on two physical sheets;
and landscape physical page geometry. All15 owned processes are terminal, guarded
inputs are unchanged and the PDFs are retained with content checksums. This
confirms that those settings are already baked into these prepared GTK documents.
Reapplying requested copies or those transforms downstream would duplicate them.
It does not establish Qt, direct Print, PS/SVG or Canopa controller/Iced/CUPS
integration. See `target/portal-print-gtk-render-03/` and
`.scratch/xdg-portals/print/gtk-prepared-evidence.json`.

The first probe missed GTK's required unicast Response and timed out; the second
exited on the operation's done signal before asynchronous document transfer. Both
are terminal with exact tested sources retained. The final client keeps its event
loop alive until the private frontend has captured the document. Its only new
native executable is17,880 bytes in the checksum store; the obsolete17,688-byte
client is archived losslessly after restored-checksum/no-live-user checks. No
additional Rust driver or per-run compositor binary was built/copied. Total phase
net storage growth is about97MB on the reused root incremental graph.
