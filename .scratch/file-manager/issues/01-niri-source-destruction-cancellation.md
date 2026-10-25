# Niri delivers Drop after acknowledged drag-source destruction

Status: ready-for-agent

Stage: private companion correction verified; compatible distribution selection
and broader acceptance remain open.

The full Canopa Files goal remains active. This issue resolves the strict native
cancellation gate under root ADR0023; it does not replace the product brief.
Do not mark it fixed from Files' existing no-mutation guard.

## Reproduction and observed behavior

On `/usr/bin/niri`, version `26.04 (8ed0da4)`, an independent GTK source starts
an actual pointer drag to an independent GTK receiver. Both clients have one
`wl_data_device`; no Iced process runs. The receiver negotiates Copy while the
pointer remains held. `Gtk.DragSource.drag_cancel` destroys the exact started
`wl_data_source`. Its compositor `delete_id` arrives before native button release.
Releasing the pointer still sends receiver Drop, followed by Leave.
Input uses the private compositor's virtual-pointer protocol; no physical input
hardware is exercised.

Native11 and unchanged repeat12 both reproduce this sequence. Repeat12:

```text
02:40:19.066177  pointer button pressed
02:40:19.247236  source49 start_drag
02:40:22.675979  source49 destroy
02:40:22.733962  source49 delete_id
02:40:22.896386  pointer button released
02:40:22.946684  receiver Drop
02:40:22.946750  receiver Leave
```

The same GTK clients and pointer workflow pass twice on `/usr/bin/sway`, version
`1.12`: Leave follows source destruction and no Drop follows button release.
All compositors, sockets, bus, clients and files belong to private fixture sessions.
No host compositor was changed or installed.

Run from the Cargo workspace root, choosing new output directories:

```sh
python3 scripts/run-desktop-acceptance.py --case files-dnd-cancel-gtk-receiver --output .scratch/file-manager/niri-cancel-reproduction-new
python3 scripts/run-desktop-acceptance.py --case files-dnd-cancel-gtk-sway --output .scratch/file-manager/sway-cancel-control-new
```

The pure GTK controls do not require a built Iced probe. The default Iced
transport case additionally requires the `drag_transport` example. See
`dnd-cancel-verification.json` for exact source/executable identities and terminal
handles; `dnd-cancel-wire-sequence-11.txt`, `-12-gtk-repeat.txt` and `-14-gtk-sway.txt`
retain the minimal protocol sequences.

## Explanations tested

- An extra clipboard device is not necessary. The current Iced receiver fails
  twice normally and once with only its ordinary clipboard connection disabled;
  that diagnostic trace contains exactly one data device. All temporary adapter,
  patch and provenance changes were restored and verified against published
  archives. The clean probe and original Files executable are preserved.
- The raw Iced receiver is not necessary: two independent GTK clients reproduce
  the native event, twice.
- Source ID confusion or button-release ordering does not explain the control:
  it matches the exact `start_drag` source and traces the input helper's actual
  press/release requests, with acknowledgement before release.
- The successful Sway comparison isolates a compositor-path difference. It does
  not establish which Niri/Smithay callback is wrong or prove a source-level fix.

## Required outcome

Identify and correct Niri's source-destruction/active-grab handling at its actual
source seam. The [Wayland start_drag specification](https://wayland.freedesktop.org/docs/html/apa.html#protocol-spec-wl_data_device)
requires source destruction to cancel the session. Preserve the failing native
control; source deletion acknowledged before release must end the receiver
session without Drop. Keep already accepted drop receipts and their later
completion independent of cancellation of a subsequent gesture.

Validate the original Iced/GTK transport workflow and both GTK controls, then
actual Files Escape/foreign-cancellation, retained-offer and clipboard
coexistence workflows. Record exact compositor source/build identities. Follow
`docs/vendor-maintenance.md` for any shared fork, attribution or reviewable patch;
do not create another application-local dependency copy. Work in private sessions
and do not replace the installed compositor as an implicit debugging step.

## Comments

2026-10-20: The pinned companion is compiled and preserved. Native12 passes the
GTK-only control; native14 passes the original five-check transport workflow and
three controls requiring Leave before release/no Drop. The retained-offer trace
destroys source49 during source55's held grab; source55 still drops and finishes.
Native16 passes actual Files exchange in both appearances (eight checks each)
and dark gesture regression (nine), including clipboard coexistence, zero-window
work/questions, buffered Escape and narrow Move/Undo. Native15 was a restricted
private-bus startup failure before compositor/Files; it is not product evidence.
The correction is entirely in Smithay; original crate provenance, Files/probe
binaries and the Canopa Cargo lockfile remain unchanged. Installed Niri remains
unchanged. ADR0024 and niri-cancel-verification.json record exact sources and
limits. This verifies the scoped source correction, not distribution acceptance
or the separate later-grab clipboard/retained-offer cause. Earlier comments below
describe the preceding diagnosis and build stages.

2026-10-20: The exact upstream Niri/Smithay archives were fetched and verified.
An unmodified private Niri build reproduces the failure (baseline native06).
Smithay marks the source dead but leaves the input grab active and drops on
release. A canonical source overlay now adds grab liveness and immediate dead-grab
retirement; the prepared patch applies exactly to six Smithay files. The source
binding helper preserves all other Niri lockfile dependencies. See root ADR0024;
candidate compilation, native correction and distribution acceptance are pending.

2026-10-20: Primary diagnosis only; no external issue/message has been sent.
The original transport regression still passes its four preceding checks then
fails cancellation. Its aggregate remains failed. The full Files product goal
and the separate later-grab retained-offer investigation remain open.
