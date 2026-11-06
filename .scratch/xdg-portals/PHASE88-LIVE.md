# Phase88: FileChooser New folder

Goal ACTIVE; Print deferred. Previous goal turn was progress: real Flatpak
ScreenCast, optional WirePlumber policy and refreshed bundle, all terminal/pass.
This turn audited the original PRD/map/current code before picking another feature.
The useful remaining FileChooser gap is explicit folder creation in Save dialogs;
do not restart phase86/87 matrices or return to Print hardening.

Implemented in chooser_ui.rs and new chooser_ui/folders.rs: SaveFile/SaveFiles
toolbar New folder and Ctrl+Shift+N, focused Iced name prompt, Enter/Create,
Escape/Cancel returning to the original chooser, error correction, existing-entry
refusal and navigation into the created child without changing Save names. Opens
and mkdirat execute on existing filesystem workers. The prompt holds the parent
descriptor and verifies its path identity before/after creation. Single UTF-8 name
only, raw parent path bytes retained, no document writes, rollback or retries.
An explicitly created folder remains after later Save cancellation. Open dialogs
retain their browsing-only flow. No shared/vendor/dependency/routing changes.

Four new tests plus existing chooser and app-chooser tests pass (11 total) through
the original workspace/default graph, --lib chooser_ui::tests, serial private bus.
Exec80344 TERMINAL0. First attempt log01 had sandbox Unix bind denial BEFORE Cargo;
it was retried with escalation, chooser-tests-02.log. Normal root debug/incremental
cache, jobs1, no new target or cache deletion. Source was formatted before build.

## Terminal milestone

All handles TERMINAL0:90758(wire),45377(bins),52963(Clippy),91937(full Portal lib),
9758(dark02),94786(light01),15045(real Flatpak),69519(format),96082(package/smoke),
56781(streaming). No validation is live. Combined receipt:
target/portal-new-folder-88/result-01.json.

The wire/lifetime test passes on the original workspace/default feature graph.
Normal workspace bins build passes9.02s and strict all-target workspace Clippy
passes. Full compiled Portal library451PASS/20ignored in47.95s, run from its crate
directory on a private bus. Format/Python syntax pass. No new dependency graph,
profile/target, SDK/runtime, cache cleanup or compositor/frontend rebuild.

Current backend6d08eae2c3fbc576afb4785c4c5b09645975052b42592719dd850dfc4af685e8
passes15 public native groups at dark100 and light125. Both include actual New
folder creation, traversal/duplicate correction, retained unwritten Save name,
prompt-only Escape, Unicode SaveFiles folder and original12 behaviors. The
failed dark01 remains failed: y72 missed the client title-bar offset. The correct
toolbar y103 passes; no product Rust change followed that fixture calibration.
Both prompt captures were visually inspected. Source pins stay unchanged.

The existing GIO Flatpak client also passes Save/new-folder plus authenticated
identity/original-preservation groups on the same backend. The sandbox writes
through the real Documents/FUSE URI into the explicitly Iced-created child; its
unshared original is unchanged and the helper maps no GTK. Existing runtime,
client and6e3d/277568 companions reused; no probe rebuild. Receipt:
target/portal-new-folder-flatpak-88/native-dark-01/summary.json. All mounts/binds
retire. Host PID-namespace audit host-cleanup-02.json finds85recorded direct/
activated PIDs absent, no process argv referencing private paths, and no remaining
private tree across phase87/88 completed cases. This host-surface audit is stronger
than merely checking host PIDs from the tool's sandbox PID namespace.

source-01.json compares current crate/Cargo/non-Markdown vendor inputs to phase86:
only chooser_ui.rs and its tests change, plus the new folders.rs. Cargo/vendor
inputs stay unchanged. Latest WHOLE workspace test gate remains phase86 PASS;
current focused success is not a new full-workspace/native acceptance claim.

One existing development archive refreshed in place, full smoke PASS,41docs/
92offline links. Streaming verifies current new backend, unchanged other7apps,
both companions, both WirePlumber policy assets and updated New folder guide.
Archive: target/portal-release-package-check/canopa-x86_64-unknown-linux-gnu.tar.gz
Bytes:209559410; SHA256:46c97c9f96d510b22328f6c19a50b0248745b9572ce38ff6f3426d264e03a505.
Delivery117.67s/smoke10.58s; temporary parent removed, no extraction retained.
Root free3404341248bytes (>3GiB), /tmp1.9GiB free. Ordinary cache growth is retained
for reuse; no per-case backend/compositor executable copies.

This feature cohort is DONE. Keep full18goal ACTIVE and Print deferred. Do not
replay phase86/87/88 completed tests/package merely for continuation. Continue
actual implementation from the original map and documented chooser follow-ups;
last-folder preferences respecting explicit caller hints are a concrete remaining
usability gap. Preserve touch/USB/client/distribution scope limits without making
up new validation matrices or claiming live installation. No user input is missing.
