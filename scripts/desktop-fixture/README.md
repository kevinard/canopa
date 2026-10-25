# Disposable desktop input fixture

`click.c` injects a pointer click into the private compositor used by
`scripts/test-desktop.py`, with coordinates normalized to its 1600×1000 output.
It is test-only code under the project license. The runner compiles its Wayland
bindings into the artifact directory; generated code is not checked in.

The Python fixture creates one shared Canopa configuration, checks banner and Center
native actions, sends Tab/Escape through `wtype`, and checks both consumers' palette
pixels. Reloading the shared configuration switches both palettes; the fixture
reopens the Center after Shell retires its foreground panel during reload.
Run `--color-scheme dark` and `--color-scheme light --scale 1.25`
with separate new artifact directories. Screenshots and `result.json` are retained.

`wlr-virtual-pointer-unstable-v1.xml` is copied without modification from
`wayland-protocols-wlr` 0.3.12's bundled wlr-protocols source (Wayland RS revision
`a3d7927d87799b2955bf491b51c7c2a3a82da661`). Its embedded MIT notice credits
Josef Gajdusek, 2019. The upstream protocol lives in
[wlr-protocols](https://gitlab.freedesktop.org/wlroots/wlr-protocols/-/blob/master/unstable/wlr-virtual-pointer-unstable-v1.xml).

## Acceptance matrix and shared session

`acceptance.json` is the native acceptance inventory. The desktop workflow runs it
through `scripts/run-desktop-acceptance.py`; `bash scripts/check.sh` validates its
coverage without starting a compositor. Adding a `verify-*.py` fixture requires a
manifest entry. The matrix includes both appearances, fractional placement, fork
lifetimes, drawers, transitions and close flashes, scrolling, content rotation,
focused icons, and idle inhibition on both Sway and Niri.

Select a Niri executable explicitly with `--niri PATH`; `--niri-sha256 HEX`
requires its exact checksum before any desktop starts. Otherwise the runner
resolves Niri from PATH once. A private executable copy in
`OUTPUT/compositor/bin/niri` supplies every Niri server and IPC command in the
batch. Rebuilding the original cannot change a later scenario. The batch summary
records both paths, version, SHA-256 and the final unchanged-snapshot check.
Sway comparison cases still run Sway. `--list` only validates the inventory and
does not select or execute a compositor.

For the pinned source-cancellation companion, build using
`scripts/build-niri-compat.py` as described in `docs/vendor-maintenance.md`, then
take the checksum from its `canopa-build.json` receipt:

```sh
python3 scripts/run-desktop-acceptance.py --niri target/niri-compat/debug/niri \
  --niri-sha256 RECORDED_SHA256 --case files-dnd-cancel-gtk-receiver \
  --output /tmp/canopa-cancellation-new
```

The preflight accepts the same two flags. Hosted desktop dispatch requires both
values, builds the transport example, and uploads the selected executable with
the results. `python3 scripts/test-desktop-runner.py --niri PATH` checks actual
executable handoff without opening a desktop. With no Niri available, its six
CLI refusal/inventory checks still run and two handoff checks are skipped.
These commands neither install a compositor nor establish full release
acceptance. Stock Niri 26.04 retains the strict cancellation failure; use the
verified companion or another build that passes that unchanged gate.

During the Files Iced migration, `--iced-browsing-only`, `--iced-search-only`,
`--iced-preview-only` and `--iced-actions-only` exercise the corresponding ported
interfaces. `--iced-interface-only` checks actual caret/range preservation across
wide/narrow toolbar layouts, draft focus through external-source notices, stale
save refusal, and prepared raster/SVG action icons in toolbar/context menus.
It also verifies shared symbolic foregrounds on reload and executable text
fallback when an icon is missing. Run each in dark100/light125; snapshots and
summaries identify their scope. The broader scenarios below retain the full
acceptance requirements and earlier GTK reference evidence; the scoped Iced
checks do not establish that the complete matrix has passed the replacement.

The maintained `files-iced-continuation-dark/light` cases exercise application
Retry and Continue through actual Operations controls, with opposite execution
orders. They copy a selected folder containing a failed child and remaining work,
replace the Files clipboard with an external selection, close every window and
request Quit. The original owner must retain the exact plans through reactivation.
Real bytes, inode preservation, exact Keep both targets and independent successor
Undo are asserted. These are live-owner checks; crash/restart execution authority
remains separate. All files, settings and display/clipboard sessions are private.

The maintained `files-iced-transfer-completion-dark/light` cases submit native
Copy and Move, refuse only the application result save, and kill the real process
after its typed transfer end commits. They create newer target/source work before
restarting, inspect the finished result in narrow Operations, and verify exact
historical reports and original Undo steps without filesystem replay. This covers
completed work; executing unfinished saved transfer plans remains a separate gate.

The maintained `files-iced-dismissal-dark/light` cases review failed Rename and
actual commit/result-save process death through narrow Operations controls.
They preserve colliding files, post-crash edits and inode identity, the exact
request execution digest, and independent Undo. The private SQL fault only
refuses result saving; it does not invent filesystem work or scheduling results.
A real SIGKILL supplies the interruption. These cases verify dismissal after
review, not automatic saved-plan replay or complete restart continuation.

The maintained `files-iced-claim-review-dark/light` cases seed post-interruption
ordinary Rename journals with the private app stopped. Native keyboard controls
Keep a file and a readonly folder in Recovery in a 360px tile, preserve their
occupants/identities/modes, restart and copy out the kept folder, then create an
ordinary folder. The fixture observes copy-dialog retirement and Recovery focus
before requesting Browser focus once. Actual SIGKILL claims and transactional
failure/retry are core tests; these native cases verify presentation and queue
integration. They do not establish general restart execution or normal quit.

The maintained `files-iced-search-dark/light` cases run `--iced-search-only`.
They cover narrow filename matching and exact launches, raw containing paths,
hidden/unreadable locations, superseded queries, independent tabs and restored
selection/scroll, plus the actual 50,000/50,005 result boundary and Stop/More.
The drop-choices cases below separately exercise scope while the query remains
focused, including continued typing after the configured shortcut.

The maintained `files-iced-previews-dark/light` cases run `--iced-preview-only`.
They open a real Unicode source file with CRLF endings and check actual lexical
colors, exact native clipboard UTF-8 bytes with LF separators, live dark/light
restyling and return to plain text. Source bytes stay unchanged; original and
copied private bytes are retained. The same cases keep text read-only/copy,
image zoom/Fit, PDF paging and browser/tab/window ownership checks. The private
GIO provider is used for previews as well as operations; no real volume is changed.
Media playback and the complete preview matrix remain separate requirements.

`--iced-views-only` exercises actual list/icon rendering, grid navigation and
selection, narrow/wide reflow, shared sparse folder choices, live defaults,
raw-path state, natural exit/restart and failed state-write Retry. It uses 2,000
private files and an intentionally replaced private state-file path; no actual
user state is changed.

`--iced-preferences-only` exercises the global Views/Toolbar/Locations/Shortcuts
draft, validation, actual saves and remapped search, action toolbar placement,
stale-source refusal with native field selection, and byte-preserving bookmarks
through the narrow Locations popup. Its current scoped evidence does not replace
the complete focus, source-authority, XDG locations and accessibility matrix.

`--iced-operations-only` exercises floating name prompts, actual Create/Rename,
shared Undo, validation/raw filenames, unanswered Undo and source-window closure.
The private GIO provider supplies real volume signals; a simulated unplug and
loss of the application's exact helper verify queued-request placement. All
directories and the provider belong to the private runtime. It never operates
on a real mount or signals an unrelated process. Run dark100/light125 separately.

`--iced-operations-only --iced-operations-case recovery` exercises retained-file
Recovery through the Iced frontend in dark100/light125: real replacement bytes,
observed preview edits, source binding, copy/Undo, retained refused destination
drafts, raw folder/full-path URIs and owner-preview closure. Long injected strings
use bounded driver chunks. Directory continuation/reconciliation and complete
Recovery acceptance have separate scenarios and gates.

`--iced-operations-only --iced-operations-case recovery-directories` exercises
an unexpected retained directory, cancellation at an actual child collision,
replaced-output-root refusal, exact Continue and separate prefix/continuation
Undo. All windows close around an unanswered decision, then a normal launch
opens a Browser in the same process. Private read-only scheduling receipts prove
that the original request remains Running through reopening/Review and finishes
without a successor only after explicit Keep both. The fixture reads no execution
payloads. Real bytes, newer children and read-only modes are checked; actual
340-pixel geometry and floating-preview resize are observed before capture.
Both themes are maintained. Reconciliation, retry failures, expiry and complete
Recovery acceptance remain open.

`verify-files.py --downloads-only` runs the product brief's full Downloads
Iced walkthrough in one application session through `files-downloads.py`: inspect text
and images in floating Quick Look beside a 360-pixel browser, open a second tiled
destination, preserve a multi-selection through resize, Cut/Paste and Undo from
the other window, then configure and run an image action in the graphical editor.
The real command produces a thumbnail and records its selection and working
folder. Original bytes, destination contents, actual window geometry/focus,
notifications and preview pixels establish the results. It also performs a real
pointer Move/Undo, copies the action's actual captured output in narrow Operations,
checks active preview dismissal returns to its exact source and closes an inactive
preview after fresh destination input without redirecting focus. Fifteen grouped
checks per appearance cover the complete workflow. Existing dark and light1.25
case IDs retain the rendered states; all files/settings and the GIO provider
remain private.

Files scenarios include browsing, previews, transfers, Undo, Recovery, interrupted
operations, drag and drop, editors and automatic shared-configuration reload.
`--transfers-only --transfer-case lifetime` uses `files-job-lifetime.py` to close
every browser during an actual copy, allow the batch to finish with one failed
child, and reopen Operations after natural process exit. Native Retry must copy
only the repaired child while preserving the completed output's bytes and inode.
The external clipboard avoids a Files clipboard lifetime hold. All source,
destination and process state belong to the private fixture. Scenario flags are
mutually exclusive; conflicting `--*-only` choices are rejected before launch.
`verify-files.py --undo-only` covers destination conflicts, changed copies and
changed saved previous destinations. A normal writable descriptor remains open
while native Replace displaces its file; writes through that descriptor then
trigger the saved-version review. Actual keyboard input checks Skip as its safe
default, cancellation, explicit restoration, preview and a subsequent destination
conflict. Recovery is refreshed and the changed saved file previewed while the
Undo review remains unanswered. File bytes and notifications establish the outcomes without history
database inspection. Both themes, including light at fractional scale, are in
the maintained acceptance inventory.

`verify-files.py --undo-moves-only` delegates to `files-undo-moves.py`. It moves
real files with replacement, moves the incoming objects elsewhere externally,
then substitutes new objects at their recorded paths. Native keys check the
current-item review, safe Skip, cancellation, explicit Return, protected Keep,
Recovery preview/copy-out and a separate conflict at the original location.
Actual bytes verify both versions and the externally moved originals. Both
appearances are maintained separately from the broader Undo workflow.
The slice also copies a file, moves it externally, and replaces an independent
destination with it. After both normal and saved-only move Undo, older Copy Undo
must show its changed-item review; Skip preserves all three physical versions.

The same slice checks unavailable moved items: safe Skip, cancellation, restoring
only the saved destination, explicit Finish with protected preview/copy-out,
separate unreturned-item counts, and normal Undo after an item reappears.

`verify-files.py --media-only` uses `files-media.py` for actual audio/video
playback, control focus, volume, seeking and preview navigation. It generates
WAV and VP8/Vorbis WebM media and records PCM from private PipeWire/Pulse services
with a null sink and policy-only WirePlumber. No hardware monitor is loaded;
ALSA fallback is null and SDL is restricted to the private Pulse server. A private
accessibility bus observes visible controls, while actual Tab/Space/arrow input,
captured audio and rendered video pixels prove behavior. The runner preflight
checks the media generator and audio-service/capture tools. It never targets
the user's audio session. Both appearances, including light/1.25×, are maintained.

`--media-formats-only` additionally generates MP3, FLAC, Opus, AAC/M4A and
H.264/AAC MP4. Audio files change tone partway through; actual seeking and rewind
must decode the expected tone on either side. MP4 acceptance checks its distinct
AAC track, advancing H.264 pixels and paused rewind. The same isolated audio and
accessibility services are used. These reference workflows require GTK's GStreamer media
backend plus the distribution's decoder plugins, not only its GTK development
package; see `docs/files.md`. Fixture generation also needs FFmpeg's libmp3lame,
FLAC, libopus, AAC and libx264 encoders.
The maintained cases also pass `--desktop-entry` to register the actual launcher
and choose it as the folder handler inside the fixture's private XDG directories.
GIO cold-launches a folder containing spaces and Unicode before media acceptance.
With an extracted archive's binary and desktop entry, this exercises the packaged
application. It never installs a launcher or changes a MIME default in the user's
session. A helper retains ownership of the launch process group for cleanup.

`--iced-media-only` and `--iced-media-formats-only` use the direct GStreamer
backend and Iced controls. They reuse the private audio setup without starting
the GTK accessibility observer. Native input, the visible Play focus outline,
actual PCM frequencies/levels and decoded pixels are the observation seam.
The four dark/light cases cover no-autoplay, rapid relative controls, seeks and
rewind, narrow floating previews, independent Browser selection, source changes,
corrupt media/Open fallback and cleanup before the last owning process exits.
They are migration acceptance cases; their presence in the matrix is not a pass.
Every repeated tagged-MP3 opening must reach enabled transport before closing;
VP8 and H.264 must show advancing pixels during playback as well as paused seeks.
The corrupt-media case verifies decoder failure and the private Open helper's
actual arguments. Its `media-error.png` and `media-error-focused.png` still need
rendered inspection before reporting that the concise explanation is visible.
The helper is explicitly registered for WebM in the private XDG directories.
Long paths are pasted through the normal location input because per-character
virtual-keyboard roundtrips can exceed the command deadline under nested software
rendering. These media cases establish no production input-latency budget.

`verify-files.py --reload-only` delegates to `files-reload.py` here. It edits only
the private session's source/theme files, checks actual repaint and keyboard
behavior, and retains stale-draft and explicit-reload screenshots. The inventory
includes dark and light at fractional scale. It never edits real user settings.

`verify-file-device-catalog.py` checks the headless helper's catalog protocol
against the same private GIO provider. It observes mounted and unmounted local
volumes, deduplication, disconnect/reconnect signals and EOF teardown, with no
GUI and no mount/removal request. It is a separate backend gate; Iced controls,
credentials, removal/queue coordination and physical storage remain required.
The catalog gate passes five checks against the actual private helper/provider;
the preserved old helper supplies its missing-catalog failure. Existing Iced
operation and initial headless-residency gates also pass on this implementation.

`verify-file-device-catalog.py --drives` selects private GDrive Stop/Eject
capabilities. Actual asynchronous GIO callbacks cover no-force dispatch, busy
errors, cancellation, disconnect/reconnect, stale refusal, published removal
and owner EOF waiting for teardown. The default provider has no drive and keeps
its existing catalog/Mount/Unmount behavior. All files and devices are simulated
in the private runtime; no block device or actual mount is accessed.

`verify-files-device-ui.py --removal stop|eject` exercises each capability
through the Iced sidebar and narrow Locations menu. It delays the actual native
request, closes every window, then releases it and checks notification delivery,
same-owner reopening, inspectable Operations results and explicit Quit. The
maintained inventory runs both capabilities in dark100/light125. These extend
the existing device/password/question scenarios; physical-drive behavior still
requires separate acceptance.

`verify-files.py --devices-only` delegates to `files-devices.py`. It compiles
`files-volume-monitor.c` into a temporary GIO extension module loaded only by the
private session's process environment. The provider exposes a local volume with
real fixture files, hotplug signals, asynchronous failures and deliberately held
requests. It never calls mount/unmount on an actual filesystem or accesses block
devices. Device discovery, narrow Locations, keyboard/preview behavior, active-job
guards and delayed-request lifetime are exercised through the normal Files UI.

`--devices-only --device-case safety` uses `files-device-safety.py` to queue work
through an alias during removal, change that alias, substitute another device
identity at the same root, explicitly retry on the original device and restart
with it unmounted. Assertions inspect real files and native outcomes, without
reading private storage or history databases. This proves request-preparation
behavior; in-flight mutation and physical unplug safety need separate cases.

`--devices-only --device-case identityless` repeats the replacement-media check
with no backend UUID. Retry cannot identify media across mount sessions and
therefore refuses it; a fresh request after reviewing the mounted location
succeeds. The fixture records native outcomes and verifies the actual file bytes.

`--devices-only --device-case clipboard` uses `files-device-clipboard.py` to
replace source media after native Cut or Copy, retain the old selection through
Retry, return the original media, and undo the successful transfer. Copy also
uses a browsing alias, then tests its independent retargeting. A local source
can still be copied to a destination deliberately reviewed after media changes.
Device UUIDs, exact clipboard payload, real bytes on two actual filesystems and
rendered Operations at 360 pixels establish the result. The secondary temporary
directory is removed after session shutdown. All media and directories remain private.

`--devices-only --device-case recovery` terminates the private Files process
during actual Undo, restarts with the fixture volume offline and verifies that
startup leaves interrupted data untouched. Remounting and explicitly retrying
from Recovery reconciles the journal; the resulting copy remains undoable.
The retained bytes, source bytes and completed native outcomes are checked.

`--devices-only --device-case inflight` copies actual files, disconnects the
private provider after observing real staged bytes, and verifies safe pause,
preserved completed/source files, exact continuation after remount and separate
Undo of the resumed portion and completed prefix. `--device-case sources` keeps
one source device offline while an independent local file completes, then
reconnects and retries only the failed source. Both use `files-device-transfers.py`;
their clipboard URIs, file contents and native outcomes are checked. The optional
provider `disconnect-unmount` marker models a volume that must explicitly mount
again after reconnect. It never unmounts an actual filesystem.

List cases with `python3 scripts/run-desktop-acceptance.py --list`. Run all cases
with `--output /tmp/canopa-desktop-artifacts`, or repeat `--case drawer-modes
--case panel-motion` to select cases. Use a new output directory. The runner retains
per-case logs, each fixture's screenshots/results, and a summary even on failure.
A failed case does not hide results from later cases. The dedicated self-hosted
runner remains manually dispatched; native dependencies are checked before builds.

The existing `Session` in `validate-desktop.py` owns private bus and process groups,
Weston/Niri startup, IPC/display/output readiness and observable scale application.
Its private bus has no installed service activation directories. `build_pointer`
compiles the shared input fixtures; `capture` obtains a synchronous Grim frame and
`screenshot` waits for a fresh Niri image. Scenario files retain their own compositor
configuration and assertions. Drawer, motion, placement and measurement fixtures
use this startup boundary and close the Session in `finally`.

`verify-files.py --undo-saved-only` delegates to `files-undo-saved.py`. It removes
saved previous versions in the private fixture and verifies Skip/Cancel, Finish
with protected Recovery preview/copy-out, source-only return of a moved item,
retryable retained-only restoration and approval after the saved file reappears.
It checks actual files and truthful completion counts in both maintained themes.

`verify-files.py --undo-cross-only` delegates to `files-undo-cross.py`. It verifies
Skip/Cancel, Keep both, Replace, protected Recovery preview/copy-out and stale
original-location review using two actual filesystem device IDs. Its secondary
private directory is removed after the isolated desktop shuts down. The fixture
requires a writable filesystem different from its temporary HOME; it uses the
workspace or `/dev/shm` and never changes a mount or actual user folder.

`verify-files.py --undo-cross-only --undo-cross-case current` delegates to
`files-undo-cross-current.py`. It checks changed-version Return/Keep, the Skip
default, cancellation, stale review, missing-version saved-only restoration,
Finish outcome counts, and protected Recovery preview/copy-out. Maintained dark
and light variants use the same two-filesystem isolation and cleanup contract.

`verify-files.py --undo-cross-only --undo-cross-case directories` delegates to
`files-undo-cross-directories.py`. Native Copy and a same-filesystem folder move
first create the relevant Undo entries. Replacing only the private, empty
original-parent location with an alias into the second filesystem then makes
the explicit Undo Merge return its children across actual device IDs. It checks
nested/empty folders, Skip/retry, folder Keep both, stable newer destinations,
older Copy ownership and Quick Look. This exercises Merge child returns; normal
cross-device directory-move Undo ancestry remains a separate requirement. Both
maintained themes use private temporary files and never change a real mount.

`verify-files.py --undo-cross-only --undo-cross-case moves` uses the same fixture
with native Cut/Paste moving directories across the two actual filesystems.
It checks recorded directory returns, root Merge and Keep both, nested and empty
folders, partial retry, Quick Look, and older file and directory Copy ownership.
The directory Copy case must remove the entire returned owned tree while
preserving the newer original folder. Both dark and light variants are maintained.

`verify-files.py --undo-cross-only --undo-cross-case retry` uses
`files-directory-retry.py`. A private FIFO fails during native directory Cut/Paste;
repairing it and invoking Retry must finish the original empty source hierarchy.
The fixture checks both Undo entries, source-container unavailability, stable
cut selection across partial updates, newer Copy/Cut and foreign clipboard
ownership, and floating Quick Look from a 360-pixel browser. Each failed request
also resizes Operations to 360 pixels and invokes its actual Retry with Alt+R.
Actual temporary
files span two device IDs; the private provider never changes a real mount.
Both maintained themes retain screenshots and public filesystem/clipboard
checkpoints.

`run-desktop-acceptance.py --case files-clipboard-protocol` builds the lean
Iced file-clipboard service test before starting its private HOME, bus and Niri.
Independent `wl-copy`/`wl-paste` peers verify GNOME/URI/KDE payloads, original Unix
filename bytes, partial Cut updates, superseding owners, input limits and stalled
peers. A private silent Wayland socket checks startup deadlines and shutdown.
These are transport checks; keyboard/menu invocation and transfer dialogs still
require the Iced application acceptance cases.

`run-desktop-acceptance.py --case files-action-host-headless` builds the
background-actions integration test before starting its private session bus.
It initializes no GUI or compositor. An actual script runs after its frontend
and observer are dropped; the standard notification receiver withholds its reply
until the runtime proves that pending delivery holds shutdown. Releasing the
reply must allow orderly shutdown. The ordinary no-GUI test separately covers
notification-startup refusal. The fixture never uses the user's session bus.

`verify-files.py --iced-operations-only --iced-operations-case clipboard` uses
`files-iced-clipboard.py`. The maintained dark and light/125% cases exercise real
keyboard and context-menu Copy/Cut/Paste in narrow Iced Browsers, floating file
conflicts, default Cancel, Keep both/Replace and shared Undo. Independent native
peers check original clipboard bytes, newer owners, delayed reads with a captured
destination and text-field copying. Closing every Browser keeps an owned file
selection alive until an external owner supersedes it; this does not establish
primary-instance reactivation. Background question notifications are checked
without allowing their dialogs to take focus. Transfer progress/details, safe
Retry/Continue, source leases, durable restart and the full PRD remain separate
acceptance gates.

`--iced-operations-only --iced-operations-case clipboard-source-copy` and
`clipboard-source-move` use `files-iced-clipboard-sources.py`. Each publishes an
owned selection, parks its original file, creates unrelated bytes at the old
pathname and pastes through native input. Both the first and a repeated Paste
must refuse the replacement while preserving both files and the intent. A
marker publication makes a subsequent deliberate reselection observable; fresh
Paste and shared Undo must act on the replacement's literal bytes. Dark100 and
light125 cases retain narrow-window captures. This is selected-entry identity
coverage, not descriptor-relative, content-freeze or durable continuation proof.

`--iced-operations-only --iced-operations-case source-leases` uses
`files-iced-source-leases.py`. It replaces an actual destination, finds the unique
old payload in the private destination tree, and copies that version through the
normal Browser and native clipboard. Rename of the selected saved version must
report refusal while preserving its bytes and clipboard selection. One hundred
actual New Folder operations expire the original Undo entry. The owned selection must retain its bytes and
remain pasteable. An independent clipboard owner then supersedes it; up to ten
additional public folder operations allow asynchronous release and cleanup.
The pasted copy, current source/destination and foreign clipboard stay intact.
Both dark100 and light125 are maintained. The test reads no private history
tables and does not prove consuming-source or restart execution authority.

`verify-files.py --undo-cross-only --undo-cross-case context` uses
`files-recovery-context.py`. It protects edited copied work, moves its containing
folder across actual filesystems, then moves and undoes an unrelated same-name
file between the old parents. Captures verify that Recovery keeps the archive's
location and filename. A stopped-app fixture prepares authentic v15 path maps;
restart upgrades them to qualified recorded-location hints. Keep both, Quick
Look and a real Recovery copy verify continued access without consuming saved
data or replacing the recreated source. Both themes retain location, preview and
copy-chooser screenshots. SQL is used only to prepare legacy input, never to
assert implementation output. The legacy input also adds an unprotected archive
sharing the protected record's identified saved container. Startup expiry must
retire that redundant ownership while the protected record remains previewable
and copyable before and after the containing folder returns.

`verify-files.py --hidden-navigation-only` delegates to
`files-hidden-navigation.py`. It opens an explicit hidden hierarchy, exercises
parent/child navigation at wide and 360-pixel tile widths, excludes hidden
siblings, preserves Quick Look selection through resizing, and renders the
explicit Hide hidden result. Maintained dark/light variants retain screenshots
for hierarchy/selection inspection without changing a real user folder.

The existing `click` Wayland fixture also accepts `x y move` to position the
pointer without clicking. Hidden-navigation captures use it to keep toolbar
hover tooltips away from selected rows during window resizing; normal left and
right click behavior is unchanged.

`verify-files.py --undo-folders-only` delegates to `files-undo-folders.py`.
It creates folders through the normal dialog and checks nonempty-folder
Skip/Cancel/Keep, previewable new work, retry once empty, stale Keep refusal,
and nested copy-created ancestor preservation with truthful outcome counts.
Maintained dark/light variants use private temporary files and a private desktop.

`verify-files.py --undo-folders-only --undo-folders-case locations` delegates to
`files-undo-folder-locations.py`. It checks changed-folder Keep/Skip/Cancel and
preview, missing retry/Finish, unreadable-folder Finish, stale choices and nested
ancestor continuation. Notifications distinguish folders not removed from moved
items not returned. The original nonempty-folder case remains the default.

`verify-files.py --undo-directories-only` delegates to `files-undo-directories.py`.
It returns a renamed folder with Keep both while a newer destination folder
contains a protected Recovery version. Both maintained themes verify that the
newer data stays available for Quick Look and copying out, and that both folder
versions remain intact. It also chooses directory Merge, skips a colliding child
after a disjoint sibling returns, retries that child with Keep both, checks the
unchanged destination root identity and previews the returned child. These
fixtures use ordinary native operations on private temporary files. Broader
native Merge interruption and cross-filesystem acceptance remain separate work.
The workflow also adds new source work during a child decision, reviews the
remaining folder, exercises the Skip default and Cancel, rejects stale Keep,
then keeps and previews the remaining work with truthful completion counts.
An additional workflow explicitly merges newly arrived contents, refuses a stale
answer, skips and retries a reused child filename, preserves earlier returns and
previews the additional file under its new name.
It also reviews a replaced Merge destination and a missing incoming folder,
checks Skip/Cancel and stale Finish decisions, finishes one blocked branch with
truthful counts, and retries the other after its original folder reappears.
Saved-version workflows remove the private retained file before Merge and
during its last child decision. They verify the safe Skip default, Cancel,
stale Continue refusal, owned empty-folder cleanup, one pending restoration,
later restoration without repeated child returns, and Quick Look of both
returned contents and the restored saved version.

`verify-files.py --recovery-directories-only` uses
`files-recovery-directories.py` to copy a protected saved directory, observe a
new child arriving at the output, cancel at its conflict and Continue the exact
remaining work through Operations. It exercises safe Keep both, nested bytes,
floating Quick Look with a 360-pixel browser and separate Undo entries that
preserve the new output. The dark/light inventory entries use actual private
files and do not mutate real user folders. The saved root and nested folder are
read-only (0555); actual modes must remain read-only after cancellation, Continue
and both Undo entries. The independent arriving-file writer waits for output
access without changing its permissions.

`--recovery-directories-case expiry` extends that workflow with105 real native
folder creations and an identified containing-folder Rename before Continue.
It checks that unfinished Operations rows survive completed-row pruning and
that source selection follows archived ownership before storage preflight.
The old prefix's Undo expires normally; new remaining work is still separately
undoable. Basic and expiry cases both check restoring the original output after
a refused continuation, preserving unrelated replacement-folder bytes.

`--recovery-directories-case tiling` uses an unbroken long filename and resizes
Operations 360→640→360 while Continue and Undo are both available. It checks the
actual window width and focus, wraps the action controls without hiding them,
and uses Alt+V to reopen a deferred conflict. Exact on-disk names, Keep both,
copy contents, floating preview and separate Undo remain part of the same
workflow. Maintained dark and light 1.25 variants retain the rendered states.

`--iced-operations-case focus` isolates the three-window return from transfer
inspection to its Operations parent. `focus-expired` scopes a compiled
`delay-activation.c` preload shim and Wayland diagnostics to Files alone. After
the real compositor reply arrives, it pauses delivery for1.2seconds before the
client dispatcher. Its control file names the inspector's actual surface, so
earlier unrelated activation replies are untouched. The fixture requires the
delay marker, exact native source retirement and the independent Browser's
continued focus. No production instrumentation or user desktop is involved.

`--iced-operations-case transfers` uses `files-iced-transfers.py` to perform a
real partial copy with a confirmed file, an unsupported FIFO, a later filename
conflict and unstarted work. It closes the Browser while the transfer waits,
inspects progress beside 360px Operations, explicitly cancels and reverses only
the confirmed prefix. A later 25-file copy verifies that the original inspector
stays bound to its job. Keyboard scrolling, twenty-entry result pages, section
reset and explicit older/newer job selection are checked against the same native
inspector, with a rendered response awaited after each input.

The `parents` variant closes Operations with both its inspector and unanswered
question open. Both child windows must retire without a completion notice or
filesystem answer. A separate Browser reopens Operations and the same question;
only explicit cancellation stops the transfer before shared Undo.

The `compact` variant isolates the stopped partial result at 320×240. Failed
remains focused while shrinking; Home/End must expose different recorded facts.
Tab reveals Sources horizontally, and activating it must retain the inspector
through compact→normal→compact resizing. Close remains keyboard-accessible.
The full transfer workflow repeats these compact checks after job switching.
All three variants have maintained dark and light/125% matrix entries and use
private files, HOME, compositor and D-Bus sessions.

`--iced-operations-case conflicts` uses `files-iced-conflicts.py` for actual
folder Copy conflicts. It reviews captured facts at 320×240 through Home/End,
Up/Down and Page Up/Down, retaining the same native question and action focus.
Keep both copies a nested folder, shared Undo removes only that copy, and folder
Cancel/Skip preserve both versions. Three further batches apply Merge to two
existing folders and separately apply file Keep both, Replace or Skip. They
verify actual source/destination bytes, unchanged destination-folder identities,
one question per kind and independent Undo results. Regular-file payload checks
leave retained-storage directories uninterpreted; ownership and cleanup have
separate acceptance. Both maintained themes have matrix entries.

The ordinary Iced operations workflow also reviews an actual Undo question at
minimum size with Home/End, then restores its size before the established Keep
item and queued-original-volume checks. Captures exclude decorations and actions
when comparing facts, and verify the exact native question identity.

The `question-focus` variant uses the conflicts fixture to open a second Browser
while an actual Copy question waits. Refocus the question and Escape; its exact
native window must retire, cancellation must preserve both trees, and focus must
return to its captured Browser rather than the independently opened window.

`--iced-operations-case unavailable` uses `files-iced-unavailable.py`. A real Copy
completes one file and waits at another file's conflict. Move its destination
directory aside, Skip the conflict and Retry while missing. Create an unrelated
folder at the original pathname and Retry again: the remaining file must stay
at its source and the transfer must expose another typed question. Restoring
the original directory allows completion; shared Undo removes only the two
actual copies and preserves the skipped destination. Original folder and prefix
inodes are checked throughout. Both themes run these variants in private HOME,
Niri and D-Bus sessions. Ordinary directory relocation does not establish actual
device unplug/reconnection acceptance.

The `move-conflicts` variant reuses the ordinary folder-conflict workflow with
actual Cut/Paste. It captures a new Cut selection after Undo before the next
request, checks atomic Keep both folder identity, verifies Merge's moved and
skipped leaves and completed-source cleanup, then checks original source bytes
and moved leaf identities after shared Undo. It preserves existing destination
folders and unrelated files. Dark and light variants remain separate from Copy.

The `mixed-copy` and `mixed-move` variants use `files-iced-mixed-conflicts.py`.
A batch file Replace precedes folder-over-file and file-over-folder collisions,
then another ordinary file collision. Require a fresh safe mixed question,
minimum-size Home/End metadata review and independent mixed Keep both batching.
The later ordinary file must still use Replace. Actual payloads, item kinds,
Move identities, two attention notices and shared Undo establish the result.
These checks do not interpret retained-container storage or grant Retry/restart
authority. Both maintained themes use private files and desktop sessions.

`files-operation-attention-headless` uses the same private-bus harness with
`--workflow operations`. It builds only the background-operations feature before
creating a bus, checks a held real Notify reply against direct queue shutdown,
then runs an unanswered Undo and queued folder creation without GUI initialization.
The exact notification sequence, actual directories and later file bytes establish
the result. It grants no daemon, primary-instance or restart-continuation claim.

`files-resident-headless` and `files-resident-activation` use
`verify-files-resident.py` with the actual executable, private state, bus and GIO
provider. Headless startup has no display variables. A browser request is refused
without killing that owner, and Quit must await its exact platform helper: the
fixture opens a pidfd before checking executable, arguments and parent, stops
that private helper, checks the owner remains alive, and always resumes it.

The graphical case starts with no Files windows, verifies later windows belong
to the resident PID, renames a raw-byte filename and undoes it after every window
closes. An unanswered Undo and queued folder creation survive all windows closing
and Quit; reactivation restores the same review and explicit Keep item resumes
work. Three warm openings and three-second CPU/RSS observations cover the owner
and its descendants. These are private debug-build measurements with the fixture
provider; summed RSS includes shared pages. They do not establish release
performance or safe execution continuation after a runtime crash.


`--iced-operations-case interrupted` uses `files-iced-interrupted.py` in both
maintained themes. It kills only its private Files process after a real Undo
quarantine, creates a newer occupant and restarts the same history. Startup
Recovery must appear once; dismissing it and attempting ordinary Undo must not
reopen it. Space previews the actual retained 16 MiB text file, and clipboard
inspection verifies its bytes. Recover a copy must preserve the primary pending
journal identity, quarantine and newer occupant. A second process restart and
actual 340-pixel Recovery layout precede explicit reconciliation: refuse the
occupied name, then restore after it is cleared. Independent Undo reverses
salvage and the original Copy while preserving newer work and source bytes.
The SQLite observer reads only the private journal body's digest, never its
execution payload. This is scoped interrupted-Undo coverage, with general
restart execution authority and startup reliability still separate gates.

`verify-files-startup.py` repeats fresh private application sessions without
changing the first-Browser readiness deadline. `--fixture operations` uses the
ordinary operation setup; `--fixture interrupted` uses the actual large-file
interrupted-Recovery setup and exits after the initial Browser. Maintained dark
and light cases run four attempts each for both setups. Capture timing includes
settling and screenshot work; this is neither resident warm activation nor a
system cold-cache benchmark. Passing repetitions establish no fix for earlier
unreproduced startup timeouts.


`verify-files-dnd-transport.py` uses the standalone `drag_transport` example,
built with `cargo build -p canopa-files --example drag_transport --locked --offline
-j1`. The normal `--binary` Files path locates the example in the same target
profile. This is Iced/native transport acceptance, separate from actual Files
row selection, targets, operations and Undo. Its GTK peer mutates only private
fixture files, records exact URI bytes and native source lifecycle, and can hold
completion while the Iced source window closes. Both clients retain wire logs.
`--cancel-control` repeats native cancellation with an independent GTK source;
strict cancellation currently reproduces a post-destruction Drop on Niri26.04.
Keep that case failing until resolved. Successful earlier checks remain in the
failure summary. No physical device, installed user service or user file is used.

`files-dnd-cancel-gtk-receiver` adds `--gtk-receiver`: both clients use the same
independent GTK peer, with distinct titles/application IDs and one data device
each; no Iced executable is started or required. `files-dnd-cancel-gtk-sway` runs
that same cancellation control under a private headless/pixman Sway session.
`--compositor sway` is allowed only for this GTK control at scale1. Both require
the exact started source's destroy/delete_id before release, trace pointer input,
and require receiver Leave without any native Drop. Niri26.04 reproduces a Drop;
Sway1.12 passes. This comparison isolates a compositor-path difference, not a fix
or permission to substitute Sway for required Niri acceptance. The ordinary Iced
clipboard was also disabled in an archived temporary diagnostic build; one device
still reproduced the failure. That instrumentation is removed from the adapter.


`files-iced-drop-exchange-dark` and `files-iced-drop-exchange-light` exercise the
actual Files application against that independent GTK peer, at 100% and 125%.
They verify raw URIs in both directions, a fresh read after Drop, refused foreign
cancellation, unchanged rendered operation facts after cancellation, text and
owned-file clipboard coexistence, and accepted receipts with every Files window
closed. A pending foreign conflict is reactivated in the same process, resolved
explicitly, and undone. An outgoing Move finishes after origin closure, without
a second deletion, then releases the ordinary owner. These use only private
files/compositor/bus and retain wire logs, per-check snapshots, captures and file
inventories. They complement `files-iced-drops-dark/light` for actual row gestures,
selection, scrolling and narrow columns; they do not replace the strict native
transport cancellation or full-product gates.


`files-iced-drop-choices-dark/light` use a private directory on a genuinely
separate filesystem and require distinct device IDs. They check Copy by default,
Shift Move and shared Undo, icon sources/current column headers, nested recursive
search sources, live sidebar bookmarks without navigation, and ancestor-folder
sources whose view becomes unavailable during the accepted operation. Hover may
change neither the filesystem inventory nor the notification count. Native
source/action negotiation, exact committed bytes and normal saved completion
remain required. Sidebar placement reserves enough logical width, locates the
source's painted selection band after toolbar reflow, and reveals both tiles
before centering (the compositor command only centers fully visible columns).
The secondary private directory is removed after session shutdown and its cleanup
is checked separately from the six gestures. Per-check namespace/native/notice
snapshots, geometry, device IDs, captures and final private inventories are kept.
The focused scope shortcut regression shares the actual query field; it appends
x and Backspaces before moving the nested match. Device/stale/delayed targets,
strict peer cancellation, broader coexistence and full-product gates stay open.


`files-iced-drop-stale-dark/light` check destination/source replacement during
actual held gestures and normal view refresh. They require a prepared native Move
before interference, preserve both versions on refusal, and verify that new
reviewed gestures and unrelated arrivals still permit Move/shared Undo. The five
checks per appearance use exact native source termination and offer retirement,
read-only private scheduling IDs/revisions/phases (never bodies), original inode
and content inventories, and rendered captures. Receipt observation starts after
the exact prepared target capture because wire object IDs can be reused on enter.
These cases do not establish mounted-device target, delayed geometry, strict peer
cancellation, Downloads or full-product acceptance. All paths/providers are private.


`files-iced-drop-devices-dark/light` use `--iced-operations-case drop-devices`
with the unchanged private native GIO provider and actual temporary files. Eight
checks per appearance cover mounted Open-row Move, Ctrl Copy and shared Undo
without navigation; removal-control rejection; held disconnect/replacement with
an unchanged root pathname/inode; a fresh reviewed replacement drag; an unmounted
row without implicit Mount; explicit GIO remount and another transfer; and the
mounted root in a 340px tile without a sidebar. The provider's replacement UUID
distinguishes storage identity from directory identity. Refused drops leave file
bytes/inodes, notification count and operation scheduling rows unchanged. The
fixture reads only scheduling IDs/revisions/phases and selects the returned source
with normal Home input before each new gesture. Geometry, native wire events,
per-check namespaces/provider events, captures and final inventories are retained.
These cases do not establish physical-device behavior, strict native cancellation,
broader retained-offer interaction or complete product acceptance.

The maintained `files-iced-prepared-rename-dark/light` cases keep a real F2 draft
open while replacing its selected entry. Submission must preserve both objects.
Returning the original reuses the same prepared draft, and real shared Undo
returns its inode and bytes while preserving the independent replacement. This
covers draft-period authority, not atomic kernel mutation or restart execution.
The same prepared-Rename cases also disconnect and replace the private GIO
provider while the draft is open, keeping the actual file/directory inode. The
old intent must refuse the new device; a fresh F2 selection can use it and Undo.
This uses no host mount or physical-device operation.
