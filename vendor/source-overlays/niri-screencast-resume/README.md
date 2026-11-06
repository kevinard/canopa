# Static screencast frames after consumer resume

This overlay pins Niri main `b22eb842ca9506d4e2ff997b116031cdb3f650f8` and changes
only `src/screencasting/pw_utils.rs`. The source archive, GPL license and before/
after file hashes are recorded in `vendor/provenance.json`. Its GPL-3.0-or-later
license is retained. Niri's dependency
lockfile remains unchanged, including Smithay `8a270232f1bfc416c781273c98a3f52a812a0bba`.
The older `niri-source-cancellation` overlay is independent and unchanged.

GStreamer's PipeWire source briefly corks its stream during READY → PAUSED →
PLAYING negotiation. Niri can queue its first static-window frame while paused.
The consumer discards that buffer, then Niri skips rendering after resume because
its damage history already records the static content. The consumer receives no
frame until some independent content changes. Native traces reproduce this at
both 100% and 125%; it is not a buffer-size limitation.

On every Streaming transition, clear ready-state content/cursor damage history and
the last cursor position before requesting a redraw. This replays current content
and metadata after resume. It does not transfer a grant to another source or add
an application-side frame cache.

The private frontend test checks actual pattern pixels through the FD returned by
OpenPipeWireRemote, then explicitly pauses a static window consumer, observes the
producer idle, resumes it and requires a newer PipeWire sequence without changing
the source. The unmodified candidate fails the initial-frame regression; patched
native results are recorded in `.scratch/xdg-portals/SCREENCAST.md`.

Prepare/build without installation:

```sh
python3 scripts/build-niri-screencast.py --archives /path/to/archives \
  --work-dir target/niri-screencast-sources
```

The archive filename and official download URL are in the provenance record.
Dependencies for the original upstream lockfile must already be fetched. The
helper verifies the canonical patch, preserves the lockfile, uses one incremental
dev compiler job, and records its binary checksum in `WORK/canopa-build.json`.
Use `--toolchain NAME` to match an existing baseline compiler explicitly.
Outputs stay under root `target/`. This build is a private compatibility candidate;
it does not install or select a compositor for the user's session.
