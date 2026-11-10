# 0045: One compositor candidate for capture and drag-source lifetime

Status: scoped native acceptance passes; full matrix/release/distribution pending,
2026-10-30.

The capture companion in ADR0043 and source-cancellation companion in ADR0024 use
different upstream pins. Successful ScreenCast acceptance on one does not prove
that it preserves Files' drag-source lifetime requirements. The capture-only
candidate fails the unchanged independent GTK cancellation control twice. Source
destruction is acknowledged for the exact started source after Copy negotiation,
but the receiver receives no Leave while the pointer remains held. The same
clients and control pass on private Sway.

Add an explicit combined source-overlay recipe, `niri-desktop-compat`. Keep the
capture pin b22eb842ca9506d4e2ff997b116031cdb3f650f8 and its original Smithay pin
8a270232f1bfc416c781273c98a3f52a812a0bba. Apply the existing Niri resume patch and
port the six-file Smithay lifetime correction to that revision's InputTime
interface. The older independent recipes remain unchanged; a default invocation
must not silently substitute a different compositor.

Canonical patches, source hashes and upstream licences stay under root vendor/.
Disposable prepared workspaces live under target/ or /tmp. They do not become
application crates or another supported Cargo workspace. Preserve Niri's GPL and
Smithay's MIT attribution. Building never installs or replaces the session's
compositor.

Use one shared source-binding transform for the legacy and combined builders.
Validate both workspace manifest bindings against the exact official Git URL and
revision, and validate both locked package sources before writing either file.
Replace only smithay and smithay-drm-extras with the verified local patch paths.
Retain every other manifest/lockfile value. Tests cover both actual pins, paths
with spaces, incorrect revisions/repositories and missing/duplicate locked nodes.

The capture builder accepts an explicit --overlay selection and records sources,
recipe/compiler/binary identity, original/prepared lock hashes and the two-package
binding proof. Keep its ordinary incremental dev build, one job, original locked
dependency graph and existing root target cache. No release/measurement flags
enter development checks. A changed prepared lockfile fails the build receipt.

Before selecting the combined candidate, rerun the exact cancellation control,
Iced/GTK transport including retained offers, and active-source destruction while
a newer grab remains live. Then verify actual Files exchange/clipboard/gestures
and the ScreenCast media/cursor/control/crash cases on that same executable hash.
Keep pointer evidence separate from physical touch, scoped acceptance separate
from the full desktop matrix, and debug acceptance separate from release and
distribution acceptance. The application archive still does not install Niri;
compatible provisioning and portal advertisement remain explicit later work.

The dev candidate73b993cbc3c5036701bf612ecb8e260152cce79f408be61b7d2d4cf1bc923026
passes the original transport/cancellation controls, actual Files exchange in both
appearances and dark gestures/Undo, plus seventeen ScreenCast groups per appearance.
The first aggregate batch remains failed: its dark exchange case timed out before
Niri IPC. A later retry reached Niri but missed the first Files window deadline;
the unchanged case subsequently passed. An initial ScreenCast consent timeout also
preceded successful minimized and complete runs. Causes of these startup failures
are unproven. Retain all results and keep deadlines unchanged; these observations
do not establish a fully green matrix or production startup reliability. Exact
checks and artifacts are in the portal distribution ledger.

Update, 2026-11-10: the `vendor/` source-overlay recipes and their verifier were removed. The combined companion is now covered by the `canopa/fixes` branch of [kevinard/niri](https://github.com/kevinard/niri) (screencast resume plus the drag-source lifetime topic, with kevinard/smithay `canopa/dnd-source-lifetime`), with a tracking PR in the fork, and is part of the pinned `canopa/main` commit that `scripts/build-niri.py` builds for the packaged compositor (`packaging/forks.json`). `--ref canopa/fixes` rebuilds the topic for comparison. See ../vendor-maintenance.md#niri-compositor.
