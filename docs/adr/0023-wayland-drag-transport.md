# Ordinary Iced windows and Wayland drag transport

Status: implementation and scoped native acceptance in progress, 2026-10-20.
Extends Files ADR0094 and the common Iced fork decisions. This does not finish
the Files drag/drop feature or the shared-stack regression matrix.

Files needs outgoing file-URI drags, incoming offers, negotiated Copy/Move and
asynchronous completion. Winit's existing file-drop events do not expose the
source implicit grab, native offer identity or completion lifetime. Add optional
renderer-neutral drag handles to iced_runtime, an ordinary-window implementation
in iced_winit, and native transport on Winit's existing Wayland connection.
Layer surfaces explicitly decline the request. Other applications create no data
devices until opting in; no file-manager policy enters these shared modules.

An endpoint exposes retained offer/source snapshots and coalesced wakeups. Drop,
Leave and Finish remain observable state. Each dropped offer has its own record,
so a later Enter cannot overwrite a transfer still awaiting completion. Actual
client-and-seat source state identifies internal drags; a public MIME marker
never grants internal authority. Copy/Move selection is frozen at Drop and must
be honored by the application. Native Move completion never deletes a file.
The receiving file manager owns its actual Move transaction.

Outgoing source start requires an opaque capability for the exact original
left-button press. Every client press consumes its corresponding native record,
including ordinary clicks. Release, a new grab, closure, endpoint mismatch or
prior use refuses start. An asynchronous selection-preparation result cannot
borrow a later click's grab. The application captures this capability before
normal row handling and validates its own browser/session identities separately.

Data pipes run outside native dispatch: at most sixteen combined reads/writes,
sixteen MiB per transfer, a five-second maximum deadline and cancellation by
owned handle. Workers use nonblocking I/O and bounded polling intervals. Native
callbacks only dispatch state or delegate an owned descriptor. Pipe failures
are explicit. Application work must never block waiting for a peer's bytes.

Source handles and subscriptions outlive an origin window after DropPerformed.
The destination can complete later while the owning Iced daemon has zero
windows. A cancelled native source ends its transport; it cannot revoke a file
operation already admitted by the backend. An active, undropped source can explicitly stop serving and request native
cancellation for Escape. Local termination is not a foreign acknowledgement. A received drop is finished only after its
accepted application work succeeds; closing a window supplies no implicit
answer to operation conflicts.

The private drag_transport example exercises this exact Iced interface with an
independent GTK peer. It does not mutate files itself and is not the final Files
frontend. The peer performs real private Copy/Move transactions. Evidence must
include pre-release state, byte-preserving URI exchange, held completion,
multiple outstanding offers, stale original-grab refusal and cancellation.
The fixture checks each exact wire source for Drop and its terminal event. A source
can report cancellation after Drop when a later grab starts; that cannot revoke
accepted work or erase the destination's first retained offer. The receiver
finishes both independently. Do not require every retired source to report native
success as a condition of continuing already accepted destination work.

Real Files controls now have scoped native verification under
[Files ADR0095](../files/adr/0095-iced-drag-gestures-and-transfer-lifetime.md): eighteen
checks across dark/100% and light/125%, including actual transfers/Undo, scrolled
and narrow columns, and internal cancellation. A later sixteen checks cover
GTK/Files exchange, quiet foreign cancellation, selected clipboard coexistence
and zero-window incoming/outgoing receipts. Full retained-offer and shared-stack
regressions, accessibility, performance, packaging and release validation remain
required.
Keep provenance, upstream licenses and reviewable patches alongside shared changes.

Logical Escape is handled in native keyboard dispatch before a later Drop can
overtake the application's earlier queued key message. It consumes the exact
seat's held press and terminates only undropped sources. Accepted receipts are
preserved. A real stopped-process Files test reproduced an unwanted Move before
this correction and passes afterward; an unrelated logical key with the same
synthetic native keycode still permits the Move. This is an internal Files
admission safeguard, not proof that a foreign peer receives no phantom Drop.

## Cancellation diagnosis on the installed compositor

On Niri 26.04 (8ed0da4), both this Iced source and an independent GTK source
reproduce a Drop after the compositor acknowledges source destruction. The GTK
control calls Gtk.DragSource.drag_cancel, releases its native references, and
records the exact source destroy/delete_id before native button release. Its
Iced receiver nevertheless reports dropped=true with the prefetched URI bytes.
The reverse Iced-to-GTK case also sees a Drop; its read then returns no file
references and it makes no filesystem change. This native behavior is reproduced
with both sources. The current-source probe reproduces it twice. A temporary
comparison disables only the ordinary Iced clipboard connection: the receiver
then has exactly one data device and still receives Drop after acknowledged
source destruction. The extra clipboard device is therefore not necessary for
this cancellation failure. The temporary switch, dependency patch and provenance
changes have all been restored; diagnostic source and executable archives remain.

The independent GTK-to-GTK control reproduces the failure twice on the same
Niri 26.04, with one data device per client and no Iced process. Virtual-pointer wire
traces establish the order: exact drag-source destruction acknowledged, native
button released, then receiver Drop and Leave. The same GTK control passes twice
on private headless Sway 1.12: Leave follows source destruction and no Drop
follows release. This isolates a compositor-path difference; it does not prove
the source-level cause or a correction in Niri/Smithay. The original transport
workflow still passes its first four checks on the current clean probe, then
fails strict cancellation. Its aggregate remains a failure.

These are the installed compositor's baseline results, retained after the
companion correction below. Do not remove normal clipboard functionality to
avoid a failure that also occurs without Iced. See
[the focused issue](../../.scratch/file-manager/issues/01-niri-source-destruction-cancellation.md)
and [the exact diagnosis evidence](../../.scratch/file-manager/dnd-cancel-verification.json).

The pinned companion in [ADR0024](0024-niri-drag-source-lifetime.md) now corrects
Smithay's dead-source grab lifetime. Its native controls require Leave before
pointer release and no subsequent Drop. The original transport workflow passes
all five checks, including two independently retained offers and windowless Move
completion. Actual Files exchange passes eight checks per appearance and the
dark gesture regression passes nine, including ordinary and buffered Escape.
Destruction of an older accepted source during a newer held grab preserves the
newer drag. These results apply to the preserved private companion, not the
unchanged installed Niri. Compatible distribution selection, broader shared-stack
acceptance and the separate retained-offer cause remain open. See
[the correction evidence](../../.scratch/file-manager/niri-cancel-verification.json).

Files implements and now verifies the following safeguard with an independent
GTK peer. Foreign hover bytes alone must not authorize a mutation.
Read the offer again after Drop and confirm the same decoded local file list
before admission, refusing empty, changed or failed final reads. Keep original
source observations and destination proofs; a second read must not recapture new
filesystem authority. Internal drags also require the original backend gesture
still to be valid. The actual Files workflow verifies both guards. An empty
foreign read ends the gesture quietly rather than adding an Operations error.
Selecting the corrected compositor for distribution and completing shared-stack/
scaling regressions remain required. The source correction is maintained as a
pinned vendor overlay; no installed compositor was changed.

A separate retained-offer coexistence cause remains under investigation: Iced's ordinary
smithay-clipboard data device ignores DND callbacks while SCTK retains its dropped
offer until the next Enter, then destroys it. That can affect the original
source's cancellation independently of the separately retained raw offer. Do not
attribute the later-grab cancellation to Niri alone. Actual Files text-paste and
owned-file clipboard coexistence now pass, including after cancelled drags; that
does not isolate the retained-offer cancellation cause. The one-device experiment
above concerns source destruction before Drop, not this later-grab interaction.
No registry sources or
additional dependency forks were
changed during this investigation.

The [core Wayland specification](https://wayland.freedesktop.org/docs/html/apa.html#protocol-spec-wl_data_device)
states that destroying the source cancels the drag session. We retain the strict
peer cancellation gate rather than redefining cancellation as successful native
Drop delivery. Current Files guards establish no mutation and quiet refusal in
this fixture; they do not establish that the residual native event is correct.
