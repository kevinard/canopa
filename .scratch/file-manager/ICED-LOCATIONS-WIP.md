# Iced locations checkpoint

Previous goal turn: progress. Current goal turn: progress. Full file-manager PRD
remains active and incomplete; every product completion checkbox stays unchecked.

## Authoritative current state

No compiler or native fixture is live. Compiler cells1211/1240/1247 and native
cells1216/1242/1251 are TERMINAL. Final host ps audit shows only original Niri1238
among cargo/rustc/rustfmt/canopa-files/niri. No agents were spawned. Original
PipeWire1537/1562, unrelated Shell research and read-only Git policy were preserved.

Current executable SHA:
8b949f9d196d14b8cf325d8827594a0377cdcc15db58613edf1c47e8cd0ede06
102,887,256 bytes;485 immutable inputs in iced-locations-exact-source.json.
iced-locations-final-candidate.json and iced-locations-verification.json bind it.
All47 adapter and20 public tests pass (17 existing +3 personal-folder filesystem
cases), as do all-target Clippy lints, build, workspace formatting and tracked
whitespace. Four existing unused operation-helper warning groups remain.

The exact-separator candidate passes2 final native workflows/16 declared actual
sidebar/menu launch checks in dark100/light125, with2 empty application logs.
Four final captures were actually inspected: wide Desktop/Home and narrow raw
Pictures in both schemes. This is not exhaustive visual/focus/accessibility QA.

The preceding48ddfc36... candidate passed8 native workflows/106 checks for
locations/preferences/browsing/views with10 empty app logs and4 selected inspected
captures. It is preserved in iced-locations-verification-before-separator-fix.json,
iced-locations-final-source.json and
iced-locations-candidate-before-separator-fix.json. Do not relabel those8 runs as
execution on the final exact-separator candidate.

## Implemented decisions and regressions

Icon gutter: iced-gutter-verification.json binds079a61ca... to483 inputs,
47 controller tests, build/Clippy/fmt,5 native workflows/53 checks +20 resize
cycles,7 empty app logs and3 selected inspected captures. The native border
probe fails before the fix. Default10px floating rail painted over the rightmost
card; embed it with4px spacing and use the same14px conservative allowance for
column count/keyboard stride. Grid source remains unchanged in the final XDG
candidate. ADR0062 records the scoped closure of that border defect.

XDG: PersonalFolders reads a bounded64KiB regular user-dirs.dirs source via
canopa-file, using the shared absolute config-home policy. A joined startup
worker prepares the snapshot before iced::daemon opens windows. It preserves
raw Unix bytes, escaped/localized/absolute paths, last valid assignments and
explicit Home; invalid/missing definitions retain conventional subfolders.
Definitions are data, never evaluated shell. The snapshot updates on next launch.
Desktop::place_path shares it between sidebar and narrow Locations. No new
package/fork was added. Home/Filesystem/bookmarks retain their own meanings.
ADR0064, CONTEXT index and docs/files.md record behavior and lifecycle.

The system reference probe (private HOME/config) confirmed escape behavior and
exposed loss of the trailing separator in $HOME/. Exact-byte test fails before
that fix (0 pass/1 fail in iced-locations-separator-red.log), then passes in all20
public tests after preserving the prefix separator. The final native setup uses
Desktop=$HOME/ to exercise the fix. No temporary diagnostics remain active.
A speculative hypothesis sweep was unnecessary for this case: system output,
exact-byte red and literal source branch directly identify the transformation.

## Fixture mistake, fully cleaned and isolated

First XDG setup used inherited host HOME before the driver assigned private HOME.
It created3 test files and2 folders in /home/kevinard. Birth times inside the exact
run, exact35-byte payload, regular file identities and exclusive folder contents
proved all5 artifacts were new. The approved cleanup removed ONLY those3 regular
files and2 empty directories, never recursively. No preexisting regular file was
involved. Audit and receipt are iced-locations-host-fixture-audit.json and
iced-locations-host-fixture-cleanup.json; all5 paths remain absent after final runs.

Driver now sets private HOME before all setup callbacks. The locations fixture
also derives runtime/home explicitly and checks every generated path/config root
stays private. Failed first attempt is iced-locations-fixture-failed-01. It used
the new48ddfc test-built binary, NOT the older079a61 gutter binary; it is not an
old-code red. cargo test integration targets can rebuild the ordinary executable.
The first public compile failed a test-declaration typo, corrected before the
passing run. Preserve failures as history, not current unresolved blockers.

## Required continuation

Move forward with the full Iced filesystem-operation interfaces using existing
browsing/operation/Undo/Trash/Recovery models and public tests. Complete devices,
real file clipboard/DnD/FileManager1, media playback and syntax highlighting.
Finish durable native job hydration, source lease ownership and exact filesystem-
step continuation. Never replay arbitrary custom commands.

Complete preference focus/source-authority/large-catalog/accessibility acceptance,
shared Shell/Notify native, strict/full workspace, performance, packaging/release,
review and commit gates. Existing shared forks are unchanged in this turn; retain
provenance/patch/archive workflow if later edits need them. Do not mark the full
goal complete/paused/blocked or shrink it to these verified slices.
