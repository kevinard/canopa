# 0043: Replay static screencast content after consumer resume

Status: compatibility candidate implemented; native pixel/resume checks pass in
both appearances at 100%/125%, 2026-10-30. Distribution selection remains open.

Canopa Portal's Rust capture/session implementation delivers actual monitor and
window pixels through the standard frontend's restricted PipeWire FD. Native
acceptance exposed an upstream producer race: GStreamer briefly pauses its stream
during initial negotiation, and Niri sometimes queues the first frame during that
pause. On resume, Niri sees no source damage and never supplies another frame.
Static content is valid and must not require unrelated user input to become visible.

Fix producer resume semantics in a pinned Niri source overlay. Every Streaming
transition clears ready-state content/cursor damage trackers and the last cursor
position before queuing a redraw. Keep this with the producer's existing damage
history; the portal retains source consent and recording ownership from ADR0041/42.

The canonical patch, upstream GPL license and source/archive hashes live under
`vendor/source-overlays/niri-screencast-resume/` and `vendor/provenance.json`.
The helper builds a disposable upstream workspace with an unchanged lockfile and
incremental dev outputs under root target/. No installed compositor is replaced.
The existing drag-source compatibility overlay remains independent.

Native regression checks use the real frontend, an Iced chooser, a private
hardware-GL desktop, PipeWire and WirePlumber's policy-only profile. Verify actual
asymmetric pattern pixels, source geometry, foreign FD denial and producer
retirement. A controlled consumer pause/resume requires a newer frame sequence
from an unchanged static window. Results and exact binary checksums belong in the
ScreenCast evidence ledger; initial metadata or a successful Start alone cannot
establish working video.

References: [pinned producer implementation](https://github.com/niri-wm/niri/blob/b22eb842ca9506d4e2ff997b116031cdb3f650f8/src/screencasting/pw_utils.rs),
[GStreamer consumer transitions](https://github.com/PipeWire/pipewire/blob/1.6.10/src/gst/gstpipewiresrc.c).

Update, 2026-11-10: the `vendor/` source-overlay recipes and their verifier were removed. The resume correction is now the `canopa/screencast-resume` branch of [kevinard/niri](https://github.com/kevinard/niri), with a tracking PR in the fork, and is part of the pinned `canopa/main` commit that `scripts/build-niri.py` builds for the packaged compositor (`packaging/forks.json`). `--ref canopa/screencast-resume` rebuilds the topic for comparison. See ../vendor-maintenance.md#niri-compositor.
