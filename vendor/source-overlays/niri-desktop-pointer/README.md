# Desktop companion with remote pointer and recording cleanup

This cumulative overlay pins Niri `b22eb842ca9506d4e2ff997b116031cdb3f650f8` and
Smithay `8a270232f1bfc416c781273c98a3f52a812a0bba`, the revision in Niri's lockfile.
Its two patches declare 22 Niri and eight Smithay files. Niri keeps its
GPL-3.0-or-later licence and Smithay its MIT licence. Archive names, URLs and
SHA-256 values are recorded in `vendor/provenance.json`.

It extends `niri-remote-input`, which already carries the drag-source, ScreenCast
resume and remote keyboard layers, with:

- remote pointer leases on the private `RemoteInput1` service
  (`dbus/remote_input/pointer*`, `input/remote_pointer.rs`), mapped through the
  screencast source they were granted for, including physical scale;
- per-source pointer button ownership (`input/pointer_buttons.rs` and Smithay
  `virtual_pointer`), so one virtual pointer cannot release another's buttons;
- atomic capture mappings and stable source identity (`screencasting/sources/*`),
  so a recording stays bound to its stream instead of a monitor name that may be
  reused;
- creator-owned recording retirement: a recording ends when the D-Bus
  connection that created it disappears, without stopping other clients'
  recordings.

See [ADR0056](../../../docs/adr/0056-capture-creator-lifetime.md) and
`docs/vendor-maintenance.md`. Packaging does not build this recipe on its own.
`niri-remote-touch` extends it and is the recipe the Arch package builds.
