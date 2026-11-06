# Phase89: FileChooser per-application folder preferences

The user asks what actually prevents broader portal progress and reports that
Print is taking too long. There is no missing user decision or external blocker.
Print remains deferred and separately opt-in. Its phase79 cross-restart worker
admission and inherited-descriptor launch regressions, and the separate measured
Qt routing gap, are not prerequisites for delivering the other portals. Do not
restart Print, its failure matrices, or completed phase86/87/88 cohorts.

## Terminal delivery checkpoint

This goal turn completed the current feature; it is concrete progress, not a wait.
The normal FileChooser now remembers an identified application's last accepted
folder across backend restart. Explicit current_folder/current_file take
precedence. A current_name alone can use folder memory. Anonymous callers share
no history; canceled or invalid choices do not update preferences. State contains
folders, not selected filenames or grants. It is limited to128applications/2MiB,
with advisory fallback and --no-file-chooser-history. ADR0114 and the chooser
guide document ownership and limits. No dependency or compositor change.

All current validation handles are terminal. Combined receipt:
target/portal-chooser-history-89/result-01.json.

- Six history unit tests PASS on the original workspace/default/debug/incremental
  graph; history-tests-01.log. Other libraries were filtered out.
- Both FileChooser wire/lifetime tests PASS; wire-tests-01.log, handle89881.
- Normal workspace bins PASS9.06s; strict workspace Clippy PASS172.28s; format
  PASS12.74s. Sequential build-checks-01.json, handle21202. Hash read16428terminal.
- Full compiled Portal library PASS457/20ignored in47.89s, from its crate working
  directory on a private bus; portal-library-01.log, handle64440. Latest full
  workspace test gate remains phase86; current focused results do not replace it.
- Native dark/100% PASS20groups on backend dfdf49c5, native-dark-02/result.json,
  handle6494. Includes per-app persistence across restart, isolation, explicit
  current_file precedence, cancellation, deleted-folder fallback and private
  folder-only records, alongside existing chooser behaviors.
- Real Flatpak PASS3groups, using the existing GIO client/runtime and frozen
 6e3d Niri/277568 frontend. The app writes through Documents into the Iced-created
  child; the accepted folder is recorded under its authenticated app ID, without
  the selected filename. Original unshared file unchanged; helper maps no GTK.
  target/portal-chooser-history-flatpak-89/native-dark-01/summary.json, handle5348.

The first native attempt40601 remains FAILED: fixture applications lacked desktop
entries required by Registry.Register. Its preceding14chooser groups passed.
Adding two private desktop entries fixes the fixture; no product Rust change
followed the failure. Python AST passes. No retry-until-green product regression.

The single existing development archive was replaced in place, package107.94s,
full smoke10.40s, handle5934terminal0. All41docs/92offline links pass. Streaming
verification92403terminal0 proves all8apps+2companions, both policy files and the
updated folder-memory guide, without retained extraction. The other7apps,
companions and policies match phase88 exactly; the new backend and disable flag
match current source/build. Receipt: delivery-verification-01.json.

Backend SHA256: dfdf49c5ae922bbfbc7ed4cf1e44133e04c865d0f12b34a3cc893114067ade8a.
Archive: target/portal-release-package-check/canopa-x86_64-unknown-linux-gnu.tar.gz
Bytes209601608; SHA256141fd6ea20e5aeb47934236e846252c3c75c288d230f47c9dcca2b5822d3d74b.

Host-namespace cleanup audit finds all24recorded native PIDs gone, all3private
paths gone, and no process argv/environment references. Flatpak Documents/runtime
mounts retired. Delivery temporary parent removed. Root free3412946944bytes,
above3GiB; no user cleanup requested. Existing build cache/feature graph, runtime
and native companions reused; no SDK download, fresh target or binary snapshots.
Source receipt compares against the phase86 workspace snapshot, accounting for
phase87/88 additions: Cargo and vendor runtime inputs unchanged. Historical
vendor README documentation differs; do not call that a new fork/runtime change.

This cohort is DONE. Do not rerun completed matrices/builds/package on continuation.
Continue a concrete remaining non-Print capability or delivery seam from the
original map after inspecting current source. No external blocker or missing user
answer. Print stays deferred and opt-in, full goal ACTIVE, live routing unchanged.
