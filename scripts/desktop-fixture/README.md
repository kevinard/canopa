# Disposable desktop input fixture

`click.c` injects a pointer click into the private compositor used by
`scripts/test-desktop.py`, with coordinates normalized to its 1600×1000 output.
It is test-only code under the project license. The runner compiles its Wayland
bindings into the artifact directory; generated code is not checked in.

`keyboard.c` is a test-only persistent virtual keyboard under the project license.
It reads Linux evdev key codes (optionally prefixed with `ctrl `) from stdin and
acknowledges each released key on stdout. ScreenCast acceptance keeps one full US
XKB keymap throughout sequential
choosers, avoiding per-command synthetic keymap replacement. Its protocol XML is
copied unchanged from `wayland-protocols-misc` 0.3.12, Wayland RS revision
`a3d7927d87799b2955bf491b51c7c2a3a82da661`, with its embedded MIT notice retained.
The upstream file is [virtual-keyboard-unstable-v1.xml](https://github.com/Smithay/wayland-rs/blob/a3d7927d87799b2955bf491b51c7c2a3a82da661/wayland-protocols-misc/protocols/virtual-keyboard-unstable-v1.xml).

The Python fixture creates one shared Canopa configuration, checks banner and Center
native actions, sends Tab/Escape through `wtype`, and checks both consumers' palette
pixels. Reloading the shared configuration switches both palettes; the fixture
reopens the Center after Shell retires its foreground panel during reload.
Run `--color-scheme dark` and `--color-scheme light --scale 1.25`
with separate new artifact directories. Screenshots and `result.json` are retained.

`scripts/test-print-dialog.py` checks the private Print catalog and Iced draft
against an explicitly supplied disposable CUPS socket and passthrough fixture
queue. It pins the application/compositor checksums, checks saved instance options
and actual paper geometry, then exercises approval, cancellation, concurrent drafts
and retirement of metadata blocked on a private unresponsive socket. Run dark/1.0
and light/1.25. Supply the existing parent fixture with `--parent` and its checksum;
transient checks use its actual exported handle. `--harness` and its checksum select
the current Cargo JSON-selected Portal lib executable: its ignored native fixture
calls the production Rust parent adapter, receives the real file descriptors and
verifies their pins after UI exit. There is no extra standalone Rust driver build.
`--file-only` expects an empty scheduler and exercises the same-window Files Save
picker, late target appearance, explicit overwrite and unchanged existing bytes.
It does not submit print jobs or establish public Print, file publication,
rendering or physical-printer acceptance. Mutable CUPS state and queue
setup belong to the caller's isolated fixture; never point it at an installed
scheduler. Results and owned-process exits are recorded in `summary.json`.

`wlr-virtual-pointer-unstable-v1.xml` is copied without modification from
`wayland-protocols-wlr` 0.3.12's bundled wlr-protocols source (Wayland RS revision
`a3d7927d87799b2955bf491b51c7c2a3a82da661`). Its embedded MIT notice credits
Josef Gajdusek, 2019. The upstream protocol lives in
[wlr-protocols](https://gitlab.freedesktop.org/wlroots/wlr-protocols/-/blob/master/unstable/wlr-virtual-pointer-unstable-v1.xml).

## Acceptance matrix and shared session

`acceptance.json` is the native acceptance inventory. The desktop workflow runs it
through `scripts/run-desktop-acceptance.py`; `bash scripts/check.sh --workspace` validates its
coverage without starting a compositor. Adding a `verify-*.py` fixture requires a
manifest entry. The matrix includes both appearances, fractional placement, fork
lifetimes, drawers, transitions and close flashes, scrolling, content rotation,
focused icons, and idle inhibition on both Sway and Niri.

`greeter-login` runs the frontend directly under the private Niri. `greeter-cage`
and `greeter-weston` run the same 28-check authentication/input/lifetime matrix
through the product `session --compositor` launcher. Install Cage and Weston 15+
with kiosk-shell for those cases. Weston can exit zero on a failed frontend;
the fixture verifies its explicit failure log and absence of greetd handoff.
All authentication uses the test-owned protocol peer, never host accounts/PAM.
Sessions are discovered from fixture-owned desktop entries, including literal
argument/environment handoff; an empty catalog must not begin authentication.
User discovery uses a private Gio AccountsService peer and private system-bus
address. Its roster includes human, system, root, non-local, invalid and missing-
metadata records. Only the four permitted properties may be read; account mutation,
password hints and home/avatar data fail the fixture. Native cases verify
AccountsService-only selection, canonical greetd handoff, preservation of typed
usernames on late replies, and manual authentication after denied/slow discovery.
`scripts/measure-greeter.py` is a separate alternating performance measurement;
finish acceptance/builds before running it. See `docs/greeter-performance.md` for
metrics, rendering conditions and reproduction.

Select a Niri executable explicitly with `--niri PATH`; `--niri-sha256 HEX`
requires its exact checksum before any desktop starts. Otherwise the runner
resolves Niri from PATH once. A private executable copy in
`OUTPUT/compositor/bin/niri` supplies every Niri server and IPC command in the
batch. Rebuilding the original cannot change a later scenario. The batch summary
records both paths, version, SHA-256 and the final unchanged-snapshot check.
Sway comparison cases still run Sway. `--list` only validates the inventory and
does not select or execute a compositor.

For the source-cancellation fix, build the pinned kevinard/niri fork (or
`--ref canopa/dnd-source-lifetime`) with `scripts/build-niri.py` as described in
`docs/vendor-maintenance.md`, then take the checksum from its `canopa-build.json`
receipt:

```sh
python3 scripts/run-desktop-acceptance.py --niri target/niri-fork/debug/niri \
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

## Private RemoteDesktop acceptance

`scripts/test-remote-desktop-portal.py` exercises input-only production sessions
through real Iced consent and controls. `scripts/test-remote-desktop-media.py`
adds the standard XDG frontend and real monitor pixels through its restricted
PipeWire FD. Both require explicit `--niri`, `--portal`, and `--harness` paths with
matching `--NAME-sha256` values, plus a new `--output` directory. The harness is
the Portal library test executable; it serves only the ignored, environment-guarded
`screencast::remote::production::tests::native_backend_fixture`.

Run dark/100% and `--scheme light --scale 1.25` separately. The fixtures use a
private bus/compositor and harmless Wayland receiver with a French keymap. They
send real libei input only after fresh approval and verify held-key retirement
on controls/session/client/caller/backend loss. The combined fixture also checks
view-only reduction, both source/device selection orders, actual asymmetric
monitor pixels, producer removal and public session closure. It archives its
exact fixture sources before execution and verifies their hashes afterward.

The combined fixture writes a private opt-in portal descriptor. These tests do
not advertise RemoteDesktop in shipped metadata or establish legacy Notify,
pointer/touch, physical-input or compatible compositor distribution acceptance.

During the Files Iced migration, `--iced-browsing-only`, `--iced-search-only`,
`--iced-preview-only` and `--iced-actions-only` exercise the corresponding ported
interfaces. `--iced-interface-only` checks actual caret/range preservation across
wide/narrow toolbar layouts, draft focus through external-source notices, stale
save refusal, and prepared raster/SVG action icons in toolbar/context menus.
It also verifies shared symbolic foregrounds on reload and executable text
fallback when an icon is missing. Run each in dark100/light125; snapshots and
summaries identify their scope. The scoped Iced checks do not establish that
the complete product matrix has passed.

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
Scenario flags are mutually exclusive; conflicting `--*-only` choices are
rejected before launch. Without a scenario flag, `verify-files.py` runs the
default browsing walkthrough (`files-dark/light`): keyboard navigation, floating
Quick Look, image/PDF pixels, tabs, views, search, live updates, default-app
launching, name prompts with shared Undo, then native clipboard Copy/Move across
windows and the shared Operations window.

The GTK reference workflows (`--undo-*`, `--recovery-*`, `--interrupted-only`,
`--devices-only`, `--media-only`, `--media-formats-only`, `--preferences-only`,
`--drops-only`, `--actions-only` and `--transfers-only --transfer-case
lifetime`) were removed on 2026-11-14. They drove the former GTK frontend's
contracts: GApplication actions, Alt mnemonics, GTK dialog titles, the
Locations popover, notification actions and wording, and an AT-SPI media
observer. None ran against the Iced frontend (ADR0018). The `files-iced-*`
cases cover those areas natively; see
`.scratch/artifacts/files-acceptance-repair/A-report.md` for the behaviour each
removed case covered that still lacks an Iced native case.

`--iced-media-formats-only` additionally generates MP3, FLAC, Opus, AAC/M4A and
H.264/AAC MP4. Audio files change tone partway through; actual seeking and rewind
must decode the expected tone on either side. MP4 acceptance checks its distinct
AAC track, advancing H.264 pixels and paused rewind. Playback needs GStreamer
and the distribution's decoder plugins; see `docs/files.md`. Fixture generation
also needs FFmpeg's libmp3lame, FLAC, libopus, AAC and libx264 encoders.
The maintained cases also pass `--desktop-entry` to register the actual launcher
and choose it as the folder handler inside the fixture's private XDG directories.
GIO cold-launches a folder containing spaces and Unicode before media acceptance.
With an extracted archive's binary and desktop entry, this exercises the packaged
application. It never installs a launcher or changes a MIME default in the user's
session. A child-subreaper helper adopts the launched process, so its own exit
status is the application's and cleanup owns the launch process group.

`--iced-media-only` and `--iced-media-formats-only` use the direct GStreamer
backend and Iced controls with the private audio setup from `files-media.py`.
Native input, the visible Play focus outline,
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
through the Iced sidebar and a narrow window's toolbar Places menu, whose button
it invokes through Files' own accessibility tree on a private AT-SPI bus. It delays the actual native
request, closes every window, then releases it and checks notification delivery,
same-owner reopening, inspectable Operations results and explicit Quit. The
maintained inventory runs both capabilities in dark100/light125. These extend
the existing device/password/question scenarios; physical-drive behavior still
requires separate acceptance.

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
reopen it. Per ADR0142 Recovery lists the interrupted Undo's whole work folder
(`Working files`); Space previews it, and its Recover a copy must contain the
exact retained 16 MiB bytes while preserving the primary pending journal
identity, quarantine and newer occupant. A second process restart and
actual 340-pixel Recovery layout precede explicit reconciliation: refuse the
occupied name, then restore after it is cleared. Independent Undo reverses
salvage and the original Copy while preserving newer work and source bytes.
The SQLite observer compares the private journal body's digest; on a mismatch
it saves both bodies to `journal-mismatch.json` as failure evidence. This is scoped interrupted-Undo coverage, with general
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


The `files-iced-clipboard-lifetime-{dark,light}` cases perform Copy without file
operations: ownership must survive every Browser closing, then a foreign native
clipboard owner must release Files within ten seconds. The longer
`files-iced-clipboard-{dark,light}` cases also cancel transfers; that unfinished
work intentionally retains the process after the clipboard is superseded. Keep
these lifetime owners separate when changing native acceptance.

## Printer recovery reconnect regression

`scripts/test-print-cups-reconnect.py` requires `--binary`, two explicitly owned
Unix sockets (`--scheduler` and `--replacement`), the same disposable `--queue`
on both, and `--receipt`. Use fresh schedulers with matching job-ID sequences.
The relay closes the checked connection at the actual libcups cancellation
boundary and directs a new connection to either scheduler. Five cases verify
stable cleanup, benign reconnect, unrelated reused-ID preservation, three-attempt
termination and multiple receipts sharing one guard. No document is uploaded;
cleanup cancels only fixture-created jobs. The caller owns scheduler/queue setup.
This complements public Iced/crash/restart acceptance and the worker thread-group
lifetime regression; see [ADR0088](../../docs/adr/0088-checked-cups-cancellation-connections.md).

## Printer outcome review

`--recover-print` provides Files/Printers review with metadata-only confirmed
record removal and a separate cleanup action. Native review acceptance uses
controlled valid journal entries with four real incoming jobs on a private CUPS
queue, plus an owned live worker claim. It verifies that record removal leaves
jobs held, stale confirmations fail, cleanup cancels only its exact incomplete
job, a missing match stays unconfirmed, and printer-only startup opens Iced review.
Both appearances also exercise retained-file copy and record-only dismissal.

The retained sources and command replay receipts are referenced by
[the review evidence](../../.scratch/xdg-portals/print/printer-review-evidence.json).
They reuse the existing compositor/pointer/scroll/CUPS executables and the current
Cargo Portal harness; no separate Rust driver is compiled. The accepted-record UI
fixture is not an actual final document acknowledgement or physical delivery.
See [ADR0089](../../docs/adr/0089-iced-printer-outcome-review.md) for policy and scope.

## Ordinary Qt print dialog routing

`print-qt-native.cpp` is a disposable application-side probe using only public
QPrintDialog/QPrinter APIs. It renders three numbered pages after approval and
reports cancellation, selected printer and Qt version. A 60-second timeout fails
the probe. It contains no explicit portal requests; keep its results separate
from `print-qt-render.cpp`, the existing explicit D-Bus/PDF control.

The private Qt routing fixture forces the xdgdesktopportal theme, monitors actual
Print calls, checks loaded libraries, and validates loopback PDF output. On the
installed Qt6.12.0 configuration the automatic cases remain mismatches; the
explicit control reaches Canopa Iced. See
[ADR0090](../../docs/adr/0090-automatic-qt-print-routing-gap.md) and
[evidence](../../.scratch/xdg-portals/print/qt-routing-evidence.json). One small
executable is retained in the checksum store; artifact directories reference it.

## Custom printer paper

`print-pdf-custom.ppd` extends the PDF passthrough fixture with a 3 × 5 through
8.5 × 14 inch custom range. It is for an explicitly owned private CUPS scheduler;
no installed queue is required. The actual CUPS API supplies printable margins.
`ippeveprinter -P` cannot be combined with its legacy `-f`/`-2` mode. Its PPD mode
does not establish a generally working variable-range IPP device.

The custom-paper checks exercise exact worker geometry, unsupported dimensions,
altered margins and conflicting named/custom choices before Create-Job, Iced
confirmation and edit invalidation, saved media spellings, default selection,
actual prepared GTK and explicit Qt callers, and capabilities changing between
PreparePrint and Print. They reuse the existing stored compositor/CUPS/toolkit
executables and the validated normal backend. A direct A4 source explicitly uses
50% scale to fit the smaller custom sheet. The GTK probe waits for a fresh receiver
marker; an earlier job's receipt must not terminate a later caller prematurely.
Qt prepared delivery is compared with its retained source PDF's physical sizes,
including its own whole-point quantization; no second layout is applied.

The legacy CUPS path's actual downstream media is recorded independently from the
approved setup and PDF dimensions. Its measured rounding remains an open device
acceptance issue. Exploratory attribute-file receiver and driverless attempts are
retained as failed or limited evidence, with their small sources archived rather
than installed. See [ADR0091](../../docs/adr/0091-exact-custom-printer-paper.md) and
[the precision follow-up](../../.scratch/xdg-portals/issues/02-custom-paper-device-precision.md).

`scripts/test-print-custom-media.py` checks cold exact lookup through the normal
worker against an already configured, explicitly owned private Unix CUPS socket.
It sends only CheckPrinter metadata requests. Repeat `--size WIDTHxHEIGHT` and
`--reject WIDTHxHEIGHT` in integer hundredths of a millimetre; supply `--binary`,
`--scheduler`, `--queue` and `--receipt`. Optional `--instance`, `--config-home`
and `--sides` exercise saved settings and duplex. To select the private CUPS image,
use `--library` with `--expected-library-sha256`; the test seals it and verifies
the actual worker mapping. This does not test delivery or physical output.

`print-paper-document.py --output FILE.pdf` creates a four-page A4 PDF with black
centered calibration marks and page labels. Those marks remain visible when a
smaller sheet crops the source at 100% scale. Inspect actual nonblank content as
well as dimensions; identical blank pages cannot establish content preservation.

`--annotation print|screen|print-hidden` adds a deterministic stamp to the first
page. `scripts/test-print-render-annotations.py --binary PATH --output NEW_DIR`
exercises that source through the actual isolated copy-only worker, with one and
two collated copies. It verifies printable/Print+NoView appearances, omission of
screen-only stamps, exact page boxes and page order. Optional `--width` and
`--height` are PDF points for fractional-size regression. No CUPS server or printer
is contacted; writable font caches are private and removed after the run.

Phase71's stock configured-filter loopback checks now preserve exact fractional
paper, margins, duplex and saved instances, plus prepared stamps and copy counts.
Those initial receipts use the diagnostic prototype. The subsequent maintained
source/configuration/loader result is recorded in
[ADR0108](../../docs/adr/0108-maintained-ipp-receiver.md); physical output remains
separate. See
[the paper/appearance decision](../../docs/adr/0107-exact-paper-and-print-appearances.md).

The phase71 public continuation uses the standard frontend, normal opt-in backend
and actual Iced consent in both appearances. After a real width edit and fresh
paper check, one-copy simplex and two-copy long-edge stamped PDFs retain their
exact geometry and page order through stock filters. Nondefault logical settings
are not applied again to the prepared input. Initial saved defaults are already
validated automatically, and PreparePrint intentionally returns one rendered copy
while retaining the approved physical copy count. The initial fixture assumptions
about these two behaviors remain failed receipts. See
[public paper evidence](../../.scratch/xdg-portals/print/public-paper-evidence.json).

The maintained test-only receiver comes from the pinned kevinard/cups fork
(`canopa/ippeveprinter-*` topics). Build it with
`scripts/build-print-ipp-receiver.py` from a verified matching client build
receipt. `print-custom-ipp.conf` declares the owned PDF receiver's
range, margins and duplex capabilities. `scripts/test-print-ipp-receiver.py`
checks eleven validation/read-only cases without jobs. See
[the CUPS fork builds](../../docs/vendor-maintenance.md#cups-client-ipp-backend-and-receiver).


## Advanced printer controls

The owned `print-pdf-advanced.ppd` fixture declares source/type, quality and
symmetric/asymmetric resolution choices, a Manual/Glossy conflict and a smaller
manual-tray custom limit. The public destination API omits those two constraints;
the worker checks its driver in addition to individual IPP support. The bounded
public-PPD runtime test is intentionally separate from ordinary tests because it
requires the installed CUPS runtime. Its failed thread-exit case is retained as
the regression for process-lifetime library ownership.

Reuse the normal workspace/all-target build to select that runtime regression:

```sh
cargo test --workspace --all-targets --locked --offline driver_mapping_conflicts_and_conditional_limits -- --ignored
```

The phase51 source-only fixtures reuse the stable compiled graph and existing
compositor, frontend, pointer, GTK/Qt and CUPS programs. The selected-resolution
native case uses a private PDF-to-raster driver with the installed gstoraster and
Ghostscript; no new executable is compiled or copied. It correlates the accepted
Create-Job's source/type/quality to the receiver job ID and inspects every actual
raster header for 600 × 1200 DPI. The source PDF has four pages. Dark100/light125
cover saved defaults, a blocked conflict, correction through a real Iced dropdown,
scrolling and a changed driver before job creation. Six custom-paper cases per
appearance also cover real GTK and explicit Qt prepared clients, saved instance
geometry, explicit current-option confirmation and capability changes.

The receiver declares fixed A4/source/type/resolution capabilities explicitly.
A narrow source-only relay retains the first declared MIME attributes when the
installed ippeveprinter appends duplicates. Raw/forwarded metadata, original job
requests, replies and program hashes remain available. It neither invents
capabilities nor changes print requests. The first PPD-import PDF passthrough run
and CLI experiments remain separate evidence; a correct CUPS option does not
prove physical output. See [ADR0092](../../docs/adr/0092-printer-controls-and-driver-constraints.md),
[controls evidence](../../.scratch/xdg-portals/print/controls-evidence.json) and
[device work](../../.scratch/xdg-portals/issues/03-advanced-device-delivery.md).

Phase52 also verifies application preferences over neutral queue defaults, missing
remembered printers, per-field fallback and equivalent resolutions. The prepared
GTK/explicit Qt regressions intentionally retain their application A4 preference
over a saved custom default. Explicit Printer default restores the custom size;
explicit initial custom paper still requires fresh device validation. Earlier
phase51 receipts preserve their original default-only scope. See
[preference ownership](../../docs/adr/0093-initial-print-preferences.md).

## Print supervisor and descriptor retirement

`print-supervisor-client.rs` links the current normal workspace libraries and
retains the real Client/Broker/Dialog parent. `print-supervisor-helper.py` supplies
controlled worker and private-credential messages. The helper verifies the sealed
image but never loads CUPS or contacts a scheduler. The parent's real60/300-second
budgets and10-second stop grace are used unchanged.

`scripts/test-print-supervisor.py` runs controls, 42 retained-parent resource rounds
or four concurrent deadline cases. It records descriptor baselines, cancellation
EOF, response timing and process identity/retirement proofs. Keep its executable
once in the shared checksum store and preserve source-only replay and receipts.
See [the supervision guide](../../docs/print-supervision.md) for invocation and
scope; this is separate from actual Iced, scheduler and device acceptance.

## Print staging stream completion

`print-handoff-inherit.py` wraps the actual normal file worker. For PrepareFile it
forks a child that holds only the setup socket with CLOEXEC set. The controlled
copy stays live until its owning test controller releases it. The actual worker
still performs all filesystem setup and sends all publication metadata unchanged.

Use the existing normal workspace harness and executable:

```sh
python3 scripts/test-print-handoff.py --binary target/debug/xdg-desktop-portal-canopa --harness PATH_TO_CURRENT_PORTAL_LIBRARY_HARNESS --artifacts NEW_DIRECTORY
```

The ordinary held-copy case must pass while the copies are live. `--copy closed`
is the control; `--expect-would-block` records an explicitly selected old-worker
reproducer. The controller adopts only its descendants, tracks identities and
removes private files after retirement. It reuses the existing six-group native
file-client fixture and creates no native executable or installed service.
See [the completion decision](../../docs/adr/0105-print-staging-stream-completion.md).

## Matching IPP backend resolutions

`print-advanced-ipp.conf` declares an owned PDF receiver's source/type/quality and
three DPI resolutions plus unequal DPCM axes. `scripts/test-print-ipp-backend.py`
uses that receiver and the existing advanced PPD to check actual backend option
conversion, including no invented resolution for an absent option. It creates no
system queue or service. The DPCM case invokes the backend ABI directly and is
separate from configured-queue, Canopa/Iced and physical/raster acceptance.
See [the matching backend](../../docs/adr/0110-matching-ipp-backend-resolution.md).

## Real Flatpak Print caller

`print-flatpak-client.c` is an application-side GIO client with one persistent
portal connection. It requires the real Flatpak identity
`org.canopa.PortalPrintProbe` in `/.flatpak-info` and an existing host-only sentinel
path that must be inaccessible inside the sandbox. It implements `prepared`,
`direct` and `cancel` modes; the second argument is the sentinel's absolute path.
Successful printing sends an open, unlinked sandbox PDF through the public Print
FD API. Cancellation must return response 1 without sending a document.

Compile with the installed `gio-unix-2.0` SDK, `-std=c11 -D_GNU_SOURCE -Wall
-Wextra -Werror`, and retain the executable once by checksum. Link it into the
owned app as `/app/bin/probe`, with the four-page stamped fixture document at
`/app/share/document.pdf`. The client deliberately refuses ordinary host execution.
Its GTK-free minimal test runtime needs only its recorded ELF library closure.

Native checks use a private Flatpak installation/repository, session bus, normal
Canopa backend and nested compositor. No host filesystem, network or CUPS socket
permission is granted to the app. Keep the installation/config/cache/home paths
private, use only the local fixture remote, and bound storage before export.
The scoped run disables automatic accessibility/Documents-portal setup; Print
document transfer itself still uses the real filtered session bus and FD API.
This establishes actual Flatpak caller identity and Print file delivery, separate
from a standard application runtime, Documents exports, printer devices and
installed desktop routing. See [the Print guide](../../docs/print-portal.md).

## Automatic GTK Print in Flatpak

`print-flatpak-gtk.c` is a separate application-side GTK3 consumer of
`GtkPrintOperation`. The Canopa backend and consent helper continue to use
Rust/Iced. The app requires the real `org.canopa.PortalGtkPrintProbe` Flatpak
identity, checks an existing host-only sentinel is inaccessible, and rejects a
`GTK_USE_PORTAL` override. Automatic toolkit routing must reach the public portal.
It exports a real Wayland parent and renders four labeled pages with Cairo.

Arguments are `basic`, `layout`, `cancel` or `withdraw`, followed by the sentinel
path. The layout case requests pages2–4, two-up, reverse sheets and75% scale;
expected physical sides are `[4]`, then `[2,3]`, as in ADR0076. The controller
handles consent, cancellation or caller termination on the private desktop.

The app stays in its event loop until its owned stdin controller sends `quit`.
GTK's `done` signal can precede its asynchronous spool writer: the controller
waits for public Request responses and host publication before releasing the
app. It then hashes its own completed PDF. File delivery must preserve those
exact bytes; GTK performs the prepared layout. This observation channel grants
no host filesystem access to the app.

Compile once with `-std=c11 -Wall -Wextra -Werror -O1` and
`pkg-config --cflags --libs gtk+-3.0`; publish the executable by checksum. The
private runtime adds the recorded GTK ELF closure, file print backend, DejaVu
font, Fontconfig configuration, GLib schemas and verified evdev/PC105/US XKB
data. Only the Wayland socket is exposed. Keep private installation paths,
storage caps, system-renderer PDF checks and process-retirement evidence as for
the GIO caller. These checks cover an actual toolkit in an owned minimal runtime;
standard-distribution applications, Documents exports and printer devices retain
their separate acceptance gates.

## Flatpak prepared-token ownership

`print-flatpak-tokens.c` is a controlled GIO consumer for the real Flatpak IDs
`org.canopa.PortalPrintOwner` and `org.canopa.PortalPrintOther`. Arguments are the
expected ID and the inaccessible host-only sentinel path. Compile with the same
GIO flags as `print-flatpak-client.c`, adding `-O1`, and retain one executable by
checksum. Both private apps use that image and the existing four-page PDF in the
shared runtime; they receive no filesystem, network or unrestricted bus grants.

The owned stdin protocol accepts `prepare LABEL`, `print LABEL TOKEN`,
`print_extra LABEL TOKEN`, `print_held LABEL TOKEN`, `release LABEL` and `quit`.
The extra command uses another bus connection in the same process. Separate app
instances distinguish a new process with the same app ID from a different app ID.
Public request handles, authenticated IDs and connection names are observed at
the standard frontend. A foreign, consumed or stale token must open fresh Iced
consent; the controller cancels that consent and expects response 1. Foreign
attempts must leave the original owner's approval usable.

The held command passes a real readable pipe but withholds its PDF bytes. The
controller observes the production snapshot worker holding that exact pipe inode
before submitting a competing request with the same token. The second request
must require fresh consent while the first remains pending. Only after canceling
the second does the controller release the first input and verify exact delivery.
This tests overlapping one-use requests without production hooks or clock changes.

Caller and frontend loss must release unused approval directory descriptors
before another Print request. The separate restart profile keeps the original
app and bus connection alive through private frontend restart and backend SIGKILL/
restart with the same recovery store. Old tokens require fresh consent, and a
new approval must still print. These checks concern unused approvals, separate
from interruption during publication, power loss or physical printer jobs.

## Normal-service publication interruption

The [source-only replay](../../.scratch/xdg-portals/print/publication-crash-replay/README.md)
uses the same controlled Flatpak caller, standard frontend and normal backend.
Its Linux x86_64 tracer owns the backend process, follows its descendants and
observes real worker syscalls. It holds a successful journal `fsync` or publication
`linkat` exit without changing worker input, replies, memory or journal contents.
Only the main backend is then killed; the normal parent-death contract must retire
the held publisher. No alternate publisher or transaction harness supplies the
filesystem operations.

Reviewed replacements cover a completed stage before claiming, Claimed,
Publishing before the link, a successful link before its Published record, and a
synced Published record. After death, a normal discovery worker and restarted
normal service must preserve the original hardlink and retained objects. A newer
unrelated occupant is created when the interrupted target is absent; startup must
leave it unchanged. Iced recovery classifies the retained work and saves document
and displaced-file copies through fresh picker destinations. It must not restore,
overwrite, retry or discard retained work automatically.

The [staging continuation](../../.scratch/xdg-portals/print/publication-stage-replay/README.md)
also covers new files at the completed-stage, Publishing, linked and Published
boundaries. Empty setup, partial writing and full bytes without a completion
record are checked for both new and existing destinations. A valid larger PDF
uses the unchanged GIO client in an explicit private test branch: the actual
publisher writes only its first 65,536 of 80,516 bytes before main-process death.
Recovery must classify it as incomplete and disable export. Complete bytes without
the journal's completion marker must remain unavailable too. The temporary branch
is removed after all cases exit; original apps/runtime remain unchanged.

The fixtures reuse existing binaries and the shared Flatpak runtime, with a
separate 16 MiB evidence cap per continuation and the existing 3 GiB free-space
floor. They change no system routing, installed services, queues or physical
printer. This is local SIGKILL/restart acceptance at the recorded boundaries;
power loss, kernel-stalled I/O and cross-restart quotas remain separate.

`session-keyboard.c` supplies named keys, typed text and alternate keymaps for
the session tool fixtures.

`nss.c` is a test-only libc enumeration provider, active only in the isolated
`internal-nss-users` helper role. Synthetic records cover UID/shell filtering,
GECOS privacy, duplicate/invalid identities, enumeration/output budgets and blocked
or failed providers. CLI tests check automatic source precedence, authoritative
empty AccountsService responses, parent-death cleanup and timeout reaping. A
supervisor unit test checks prompt cancellation. `scripts/test-nss-discovery.py`
uses real libc in private Bubblewrap namespaces with synthetic passwd/nsswitch/
login.defs files. No host NSS policy or account files are changed. Native greeter
acceptance now includes NSS-only selection, automatic fallback and blocked NSS
with manual login and helper cleanup, on either Cage or Weston (31 checks).


Greeter avatar acceptance uses synthetic, solid-color PNGs (including extensionless
AccountsService/NSS files). It checks the selected account's rendered pixels,
canonical greetd username, image persistence during password prompts, explicit
per-user override priority, NSS directory fallback and disabling avatars. The
expanded matrix has 35 checks per compositor. Unit/integration tests cover crop,
alpha, placeholders/static fallback, image budgets/nonregular files, optional
IconFile metadata, coalescing/cancellation, cache eviction and command-free preview.
No host avatar files or user homes are modified.


`polkit.py` is a private authority, with no host grants or action execution.
`polkit-session.c` independently models the native session signals and records only
synthetic identity/status events, issuing receipts solely to that private authority.
`verify-polkit.py` covers 22 actual Wayland/session lifecycle cases.
`test-polkit-helper.py` uses real native polkit and its helper in isolated user/PID/
mount/network namespaces with test-only account/PAM files; host rules and PAM are
untouched. These complement the native caller-filter/stale-response tests and
do not replace deployed policy/enrollment acceptance.

`pinentry-dialog` runs `verify-pinentry.py` with the selected pinentry executable.
It checks native input/repeat/confirm/cancel/timeout/EOF behavior and a real GnuPG
symmetric encrypt/decrypt operation in a temporary GNUPGHOME. Requires `gpg`,
`gpg-agent` and `gpgconf`; host keys and GnuPG configuration are not accessed.
