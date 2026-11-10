# Smithay drag-source lifetime

This overlay pins Niri `8ed0da44d974c32c6877d2f4630c314da0717ecb` (26.04) and the
exact Smithay revision from its lockfile, `ff5fa7df392cecfba049ffed55cdaa4e98a8e7ef`.
Niri is unmodified; only Smithay is patched. Niri keeps its GPL-3.0-or-later
licence and Smithay its MIT licence. Archive names, URLs and SHA-256 values are
recorded in `vendor/provenance.json`.

Smithay kept treating a drag as live after the source client cancelled it. After a
GTK source cancelled, Niri still delivered a drop to the target and Files received
a transfer that the source had abandoned. The patch ties the drag grab's lifetime
to its data source in `input/dnd/grab.rs`, the pointer and touch grab modules and
`wayland/selection/data_device/source.rs`, so a cancelled source ends the grab
without a drop.

See [ADR0024](../../../docs/adr/0024-niri-drag-source-lifetime.md). This is the
first layer of the cumulative recipe chain. Packaging no longer builds it on its
own: the shipped `niri-remote-touch` recipe carries the same correction on Niri
`b22eb842`. Keep this recipe as the minimal reproduction for an upstream Smithay
submission.
