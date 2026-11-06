# Print recovery review and export

Status: scoped implementation verified; Print remains unregistered.

## Context

The recovery catalog discovers interrupted work without replaying its journal.
Users still need a way to inspect the result and save retained data. An old
destination, journal or process identity cannot grant a new write after restart.
The file moved aside by a failed publication may contain unrelated or empty data.

## Decision

Recovery is a user-owned Iced window in the existing portal executable. It uses
Canopa appearance and embeds the same Files picker as Print. The review shows the
original destination, last known retained folder and whether the result is busy,
incomplete, unavailable, interrupted, previously published or unconfirmed.

Selecting a copy starts an isolated worker. It checks the current catalog and
owner/worker liveness under the discovery lock, validates the exact recorded work
objects, and creates a sealed snapshot bounded to128MiB. Document snapshots must
match the recorded SHA-256 completion evidence. A valid displaced file can be
copied even when the document is missing or damaged; its bytes must remain stable
during the snapshot. No destination is opened for writing in this stage.

`Recovered` is a separate type from an application's `Frozen` Print document.
Recovery supports opaque and empty displaced files through a separate publication
operation. Normal Print remains nonempty and checks the selected document format.
The two reserved catalog slots admit explicit recovery publications; normal
publication still has fourteen slots, with a total catalog limit of sixteen.

After verification, the Files picker proposes a name prefixed with Recovered or
Displaced. The user chooses a fresh destination and reviews any replacement.
The picker retains its actual reviewed directory and file descriptors through
confirmation. The owner performs only the nonblocking descriptor handoff; the
publication workers revalidate the live destination. No grant is reconstructed
from the catalog. Source-directory evidence only forbids destinations and grants
no writes. Both client and workers reject the selected retained folder, and the
catalog rejects all currently known retained work folders before staging begins.

Saving a copy keeps the source work and discovery entry. A separate confirmation
removes an inactive entry from the list without deleting any of its files. The
worker rechecks liveness for dismissal, including when displayed metadata is
unavailable. Open folder uses FileManager1 with an exact file URI and a five-second
deadline. Missing or oversized data can still be inspected manually in its folder.

Generation guards discard stale UI replies. Back cannot hide an active publication.
A failed or unknown save never reports success or retries automatically; the user
is asked to check the chosen destination. Snapshot failures retire the affected
candidate until refresh. Failed refreshes disable stale copy actions, and a failed
query is presented as unavailable rather than an empty recovery list.

## Startup and lifecycle

`--recover-print` opens review directly. The default catalog is under
`XDG_STATE_HOME/canopa/portal/print-recovery`, falling back to HOME's `.local/state`.
`--print-recovery-store` selects an explicit store for maintenance or private tests.
The helper mode is private and receives configuration over an anonymous pipe.
Missing catalogs are read-only empty results and create no user state.

The normal backend starts discovery after D-Bus readiness. Nonempty discovery
opens an owned review helper with the same shared appearance configuration.
Initial discovery errors are logged; direct review exposes the query failure in
Iced. This background task does not block other portal registration. Shutdown
signals cancellation and returns within its fifteen-second host barrier while
retaining ownership until helper exit is confirmed. Kill/wait errors and timeout
are not proof of death. Publication keeps the existing worker-retirement guards
and conservative recovery records across UI or parent loss.

This does not register Print or complete global cross-restart budgets, converter
descendant supervision, abrupt full-backend/power-loss policy or actual remote
kernel-wait acceptance. Broader Print layout, device, Flatpak, ambiguous CUPS,
runtime packaging and release gates remain separate.

## Validation

`target/portal-print-41/acceptance-01.json` and
`.scratch/xdg-portals/print/recovery-ui-evidence.json` seal the final inputs, binary
pins and receipts. Formatting, strict workspace/all-target Clippy and the original
whole-workspace all-target incremental build pass. All131 Print/12 Lockdown tests
pass. Complete Portal regressions pass349 library/55 integration tests (19/2 ignored).
Fifteen new regular tests cover verified/empty snapshots, checksum tampering,
protected destinations, retained-location updates, independent displaced recovery,
reserved admission, dismissal and stale/cancelled UI actions.

Two actual normal-worker fixtures recover document plus opaque or empty displaced
bytes after confirmed producer SIGKILL. They verify two simultaneous publications,
protected-source rejection and index-only dismissal. An exact normal worker probe
rejects a changed document even when length and mtime match its completion record.
It returns an explicit failed event and never publishes a destination.

Live Iced checks pass in dark/100% and light/125% private Niri sessions. They verify
busy actions, fresh destination selection, document/opaque/empty output, retained
source preservation, manual close, actual normal-backend readiness followed by an
automatic review helper, confirmation and index-only dismissal. Dark also exercises
picker cancellation and Keep in list. Light replaces a freshly reviewed destination:
the new36-byte copy gets a new inode and the old21-byte hardlink remains intact.
Destination sizes/hashes and screenshots are retained. The normal backend's private
introspection confirms FileChooser remains registered and Print remains absent.
The FileManager1 folder-launch action is implemented but was not exercised in these
private GUI sessions.

Existing matrices pass6 filesystem/publication,3 retirement,24 renderer and18
public frontend/controller/Production/Iced groups: files dark/100% and private CUPS
light/125%. All320 observed processes in final accepted checks are terminal.
One earlier renderer runner exited143 during a successful SVG child case; its
fixture scope was confirmed empty before the complete24-group rerun. Its partial
receipt remains explicitly interrupted rather than accepted.

The original build graph/root target and stored native tools were reused. No
application executable copies, dependency changes or extra Rust native driver were
introduced. One stable18,008-byte C wheel fixture was compiled from the existing
repository source. Measured phase net storage growth is361,476,096bytes, leaving
6,967,500,800bytes free at seal, within the6GB reserve/3GB growth cap.

No new full-workspace test gate ran; the ten recorded Files integration failures
remain unresolved. Physical/power-loss/real Flatpak, wider layout/device/CUPS and
release acceptance remain. Installed services, queues and routing were unchanged.
Normal startup linkage contains no GTK/CUPS/qpdf.
