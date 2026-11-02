# Session follow-ups and Stasis integration

Status: complete (implementation and scoped validation; physical-seat limitations recorded)

User objective: implement the follow-ups proposed after the session tools,
greeter, polkit and pinentry work, using Stasis for idle policy in item 1.
Base commit: 65661e6. This plan preserves all six requested follow-up areas.

## Requirements and completion evidence

1. Stasis integration: optional external Stasis supervisor/service tied to the
   selected graphical session; Canopa/Niri Rune defaults; ordinary idle lock,
   monitor power and suspend actions; actual compositor readiness before requested
   suspend; logind delay-inhibitor coordination for externally initiated sleep;
   no false acknowledgement or duplicate locker on concurrent requests; reconnection,
   cancellation and suspend/resume tests. Shell status/pause/resume uses Stasis's
   streaming watch interface. Measure CPU, wakeups and RSS with optional detectors
   disabled/enabled against a pinned upstream revision. Verify with real Stasis and
   private Wayland/logind fixtures, not only a test double.
2. Shared authentication controls: cursor movement, selection, Backspace/Delete,
   Home/End, shared keyboard-layout selection, screen-reader/AT-SPI semantics and
   actions, and complex-script shaping. Apply to lockscreen, greeter, polkit and
   pinentry; test editing erasure, keyboard interaction, rendered text and the
   accessibility tree/actions through independent native clients.
3. Greeter convenience: persist only the last successfully authenticated user and
   per-user desktop selection; policy-checked shutdown/restart controls; refresh
   accounts/sessions/avatars when sources change. Preserve manual overrides,
   authentication generations and bounded discovery. Verify success-only state,
   live catalog changes and authorization behavior in model and native tests.
4. Wallpaper: visual picker with selected/all-monitor previews and saved assignments;
   atomic persistent state; event-driven image/config watching; optional bounded
   slideshow. Preserve zero-periodic-work default, transactional image admission,
   dynamic hotplug behavior and compatibility with the lockscreen. Verify native
   selection, restart persistence, changed files, failure rollback and slideshow.
5. Authentication refinements: best-effort locking of owned secret storage into RAM;
   explicit expired-password change/recovery in the locker respecting PAM policy;
   bounded identity-name/avatar lookup for polkit's permitted identities; pinentry
   quality feedback/constraints and generated passphrases. Keep secret transport,
   erasure, cancellation and policy ownership intact; verify independent PAM,
   polkit and Assuan/GnuPG peers including errors/cancellation.
6. Deployment validation: release archive build/smoke; current greeter compositor
   measurements; multi-monitor/hotplug and suspend/resume acceptance; VT/DRM/session
   handoff and actual distribution PAM policy/device acceptance where the runner
   can provide the necessary seat/hardware. Reproduce and resolve the pre-existing
   Files recovery-catalog workspace failure with a minimal related fix, then run
   the required workspace gate. Record environmental limitations honestly; missing
   physical/privileged acceptance is not established by synthetic fixtures.

## Execution order

- [x] 1: Stasis supervisor, lock readiness/control and sleep coordination.
- [x] 2: Shared editing, keyboard-layout selection, accessibility and shaping.
- [x] 3: Greeter state, power actions and live catalogs.
- [x] 4: Wallpaper picker, persistence, watching and slideshow.
- [x] 5: Authentication refinements and complete shared input adoption.
- [x] 6: Release/native acceptance, updated measurements and workspace repair.

Use root target/incremental debug builds for development. Run focused tests while
editing, and one workspace gate after the shared/cross-application changes settle.
Keep existing local artifacts and historical measurement settings as evidence.
Do not enable host services, replace the display manager or change host PAM/GnuPG
configuration as part of private validation.

## Evidence

Current Stasis source reviewed at d84e75e22cc00329712fb1caa83445d2f12ce921
(version 1.7.0). It runs foreground lock commands and observes LockedHint, exposes
JSON watch state, and has optional polling detectors plus a 200 ms ticker. Its
PrepareForSleep hook is not itself a compositor-readiness/delay-inhibitor bridge.
The initial source checkout is /tmp/canopa-stasis-review.Mg498Y; validation must
record a reproducible executable/source identity independently of that path.

Implementation in progress:

- Stasis native acceptance: `stasis-shell-native-4/summary.json`, 18 checks passed
  with real upstream 1.7.0 (recorded SHA-256), including shell controls and real
  logind inhibitor FDs. Core/UI/CLI focused checks, unit graph, updater (20),
  manifest/runner (8), release documentation checks passed. CPU/RSS/wakeup
  measurements, final packaging/regression and documentation polish remain.
- Shared editing/shaping/accessibility: focused libraries and strict Clippy passed.
  Native results: `greeter-input-native` (35 checks), `polkit-input-native-2` (22),
  `lock-input-native` (23), `pinentry-input-native-5` (18), all passed including
  protected AT-SPI input/actions. Multi-layout keymap and final renderer/native
  checks remain. Owned secret buffers now use private mappings, best-effort mlock
  and MADV_DONTDUMP; selection/deletion erases unused bytes.
- Screenshot environment: temporary `/tmp/canopa-followups-python` venv provides
  Pillow 13 with system GI packages. System Python has no Pillow. Native fixtures
  use this isolated interpreter, without changing host packages.
- AT-SPI 2.62.1 removed `--a11y=1` while retaining it in the usage string. New
  session fixtures use `--launch-immediately`, memory-only GSettings and the
  private IsEnabled property. Existing test-desktop launcher still needs the same
  compatibility fix before deployment validation.

The six phases remain open until all their requirements/evidence are addressed.

Further evidence:
- `pinentry-layout-native/summary.json`: 20 checks passed, including a real
  us/fr keymap, F6 local selection, Ctrl+A, AT-SPI editing and real private GnuPG.
- `greeter-followups-native/summary.json`: 15 checks passed for successful-choice
  persistence, restart/per-user recall, live AccountsService labels/new users,
  avatar replacement, installed/removed desktops, active-auth cancellation,
  confirmation and fresh logind power policy. Focused File/Greeter libraries
  (2 + 19) and strict Clippy passed.
- User has a Solo FIDO key (0483:a2ca), currently enrolled for SSH only. Plan
  temporary PAM registration/mapping and a user-presence device check; do not
  modify system login policy or SSH enrollment. No fingerprint device found in
  USB inventory yet.

Wallpaper implementation underway:
- Persisted/temporary assignment overlays, private atomic state, event-driven image/config
  watching, transactional output admission, a 256 MiB combined surface admission
  budget and opt-in slideshow CLI are implemented. Focused session library tests
  (9), all-target check and strict session/UI Clippy passed.
- `wallpaper-runtime-regression/summary.json`: 14 native checks passed, 0 CPU ticks
  over a 2-second idle check and 18,252 KiB RSS in this debug fixture (not a broad
  performance claim). Final single-decode changes built successfully.
- `verify-wallpaper-followups.py` adds private two-output persistence, source changes,
  hotplug, rollback and slideshow acceptance; registered in desktop manifest.
- Sway is not system-installed. Official Arch Sway 1:1.12-4 was staged under
  `/tmp/canopa-sway-tools`, archive SHA256
  `5704cfc9eba9804116c665cb4450352bf5a240c1a8300b36aca3218116648443`.
  It uses the existing `/tmp/canopa-greeter-tools/usr/lib` wlroots0.20 dependencies.
  No host packages were installed. Run the fixture with that bin directory on PATH
  and that private library directory in LD_LIBRARY_PATH.
- Visual picker is next; no picker source exists yet. Slideshow timer/cursor and
  source-watch initial/target-subscription race handling are implemented.

- `wallpaper-followups-native-2/summary.json`: 17 checks passed, including two
  Sway outputs, saved/temporary isolation across restarts, image/config source
  events, invalid-candidate rollback, hotplug, optional timer advancement, invalid
  slide skipping, stopping/persisting the current slide and 0 idle CPU ticks/2s.
- Current implementation now decodes each distinct candidate image once for all
  active output buffers. Shared background rendering exposes `render_source`.
- Picker groundwork: `SessionView` has optional accessible action labels, a
  consumer interaction epoch and public passive `draw_label`; wallpaper status
  includes output scale and cloneable output metadata. No picker UI source yet.

- Wallpaper picker implemented in `picker.rs` and `picker_wayland.rs` with a
  bounded latest-request preview worker, shared software presentation/AT-SPI,
  selected/all-monitor controls, accurate aspect/scale/color previews and saved
  apply. `canopa wallpaper pick` accepts optional files/folders.
- `wallpaper-picker-native-2/summary.json`: 23 checks passed including all prior
  wallpaper behaviors plus visual previews, no-change cancellation, selected-monitor
  apply, fit controls and invalid-preview rejection through native accessible UI.
  Manually inspected `picker-all-preview.png`: landscape/portrait proportions and
  text render correctly. Tiny hint wrapping/filename sanitation polished afterward.
- Source watcher now acknowledges admission snapshots and coalesces events while
  a candidate is being prepared. This removed redundant full-image decodes and
  closes image changes during admission; failed candidates acknowledge their
  observed version so a later valid write can retry. Focused session tests (11)
  include this, preview proportions, stale preview rejection and worker shutdown.
- Shared presentation now carries a consumer interaction epoch; lock and polkit
  publish their auth generations. Visible custom context also invalidates queued
  accessibility actions. Consider auditing the tiny pre-publication action window
  during final auth refinements (consumer state checks currently remain the final
  guard).

Authentication refinements implemented:
- Locker expiration recovery uses an explicit consent event after authentication
  and PAM_NEW_AUTHTOK_REQD, then the same transaction's pam_chauthtok, fresh prompts,
  account recheck and pam_end. Example Arch/Debian policies include password stacks.
  `lock-expired-native/summary.json`: all 25 checks passed, including cancellation,
  complete recovery and failed change remaining locked.
- Polkit identity helper resolves only selected permitted UIDs via individual AS
  properties/NSS, uses shared bounded process ownership and raster admission.
  `polkit-identity-native-3/summary.json`: 23 native checks passed including display
  name/avatar without account enumeration. Avatar screenshot visually inspected.
- Shared passive helper ownership moved from Greeter into Session UI, with its
  cancellation/reaping test retained. Current presentation is published before
  validating queued accessibility actions in all four auth consumers.
- Pinentry agent inquiries/constraints/generation are implemented; focused tests
  (10) and strict Clippy passed. Native inquiry test reached generation/reveal
  successfully; correcting fixture role assertion (`entry`) before continuing.
  Real GnuPG regression and final native cases still pending.

- `pinentry-refinements-native-4/summary.json`: complete native suite passed,
  including quality score, CHECKPIN rejection/approval, GENPIN reveal/hide/accept,
  cancelled unresponsive inquiry, and real GnuPG encryption/decryption. The summary
  still lists the original 20 checks; four inquiry checks were added to its script
  immediately afterward (the code paths were executed in this run).
- `auth-final-libraries.log`: all five focused authentication libraries passed.
  `auth-refinements-clippy-3.log`: strict all-target auth checks passed before the
  final cancellation framing adjustment; final workspace gate still required.
- Physical Solo test helper prepared as `scripts/test-lockscreen-device.py`.
  Official Arch pam-u2f1.4.0-2 staged in `/tmp/canopa-fido-tools`, SHA256
  6c0f22c619c1f639466e5d51410b2c2e67f4b688973ffe3607d4b7247ae88d54.
  `fido2-token -L` confirms `/dev/hidraw0` Solo4.1.5. Waiting for user readiness for
  two touches; no enrollment initiated and no host policy/SSH changes.
- Old Files failure reproduced. Backtrace points to setup's `answer_replace`,
  before Reader creation. Diagnostic job: Interrupted, “operation step exceeds its
  storage budget” for a 270-source durable queue plan. The catalog test now seeds
  its real Copy replacements with the public engine (as the neighboring large-page
  test does), preserving the active Undo/expiry behavior without altering production
  record limits. `files-recovery-fixed.log`: all four recovery_catalog tests pass.
- `scripts/test-desktop.py` AT-SPI launcher updated to the compatible command and
  private IsEnabled property/memory GSettings; native validation still pending.
- `scripts/measure-stasis.py` compares whole supervisor/backend/watch process trees,
  detector sets and Tokio workers. Context switches are scheduler sleeps, not
  hardware wakeups. Measurement begins after builds/acceptance fixtures stop.

Final validation progress:
- `stasis-final-native/summary.json`: all 18 integration checks pass with the new
  single-Tokio-worker default, including shell controls and real FD sleep ordering.
  Explicit TOKIO_WORKER_THREADS is preserved for the external backend and clients.
- `stasis-measure-debug-2/summary.json`: eight isolated 30-second samples, two per
  detector/worker combination with order reversed on the second pass. Detectors
  off: stock14 threads/0.182% CPU versus single8/0.149%, similar RSS/PSS. Detectors
  on: stock15/1.723% versus single9/1.459%. Debug builds, whole supervisor/backend/
  readiness-watch tree; compositor/listener excluded. Context-switch rates are
  scheduler sleep proxies. Reviewed source snapshot, trace and summary archived in
  docs/performance/data/2026-11-01-session/. Do not claim production CPU estimates.
- `pinentry-final-library.log`: 11 tests passed after fixing absolute deadlines,
  preallocated erased inquiry buffers (no growth/reallocation of secret bytes),
  terminal error framing and malformed-reply refusal. Both short/long constraint
  hints are retained independently. Final broad Clippy/workspace/native pending.
- Dependency attribution inventory refreshed (606 dependencies, lock hash only).
  Offline documentation staging/check passes (47 files, 67 links); desktop runner
  tests pass (8). Updated AT-SPI desktop launcher native run started.

Space/physical input dependencies:
- Root filesystem has only 1.5 GiB free; target/debug occupies187 GiB (43GiB is
  incremental). Do not start broad/release builds that could fill it before room
  is available. /tmp is separate tmpfs with5.1GiB free; caches have not been moved
  or deleted.
- Proposed cleanup manifest `proposed-build-cache-cleanup.json`:279 older hashed
  executables,64.61GiB. Retains newest executable per target name, all outputs from
  this goal, hard-linked shipping executables, libraries and all incremental data.
  User was asked for explicit AGENTS.md no-cache-cleanup exception; NO APPROVAL
  YET, NO DELETION PERFORMED. Do not treat elapsed time as consent.
- Separate pending question asks whether user is ready for two Solo touches. No
  enrollment/test started. `scripts/test-lockscreen-device.py` now has isolated
  empty pam-u2f config as well as private policy/device mount and temp mapping.
- Still required: workspace gate (not run yet this goal), optimized release build
  and archive smoke, current Cage/Weston measurements and their documented choice,
  final packaging/native checks; actual seat/distribution suspend/VT/DRM deferred
  to test installation and documented honestly. Physical test waits for readiness.

- `desktop-launcher-native-2` passed the existing complete desktop smoke check.
  Besides the removed AT-SPI flag, its old artifact-root runtime caused Unix
  socket names to exceed sun_path with long output directories (Sway log and
  invalid AT-SPI address reproduced). It now keeps IPC in a short-lived private
  /tmp runtime while preserving requested artifact/state/config/data locations.
- Final doc staging/check passes after README wiring; device helper help, affected
  Python syntax, Cargo formatting and whitespace checks pass. No disk cleanup or
  physical enrollment occurred. Full workspace/release/greeter measurements remain
  pending disk space; first goal turn encountering this blocker. Pending async
  questions: explicit cleanup exception and Solo touch readiness.

Continuation / approval resolution:
- User explicitly approved deleting the listed279 obsolete executables. The full
  list was revalidated (regular file, link count1, size, nanosecond mtime and older
  than goal start) before deleting. Result: build-cache-cleanup-result.json; root
  free space rose to66GiB. No broader cleanup authorization is inferred.
- User was ready for two touches. Default pamu2fcfg required a PIN at credential
  creation; no PIN was requested through chat or read. Added opt-in U2F enrollment
  through libfido2 with non-resident/presence-required semantics, verifying its
  attestation and writing only a temporary native pam-u2f mapping. This leaves PIN
  policy and SSH credentials unchanged. Initial test config used a reserved widget
  name; pre-validation now catches it before enrollment. Next isolated PAM attempt
  found pam-u2f's root-owned configuration requirement; hiding global config in an
  empty /etc/security mount correctly uses defaults without host policy changes.
- `solo-u2f-native-3/summary.json`: physical Solo test PASSED (5 checks), real
  pam-u2f1.4.0-2 module, actual compositor lock, AT-SPI cue, pending before touch,
  real device unlock and temp mapping removal. Native seat/host PAM policy is not
  claimed. Screenshot captures the preceding frame because AT-SPI publishes before
  presentation; inspected it. Future helper waits500ms for screenshot presentation.
  No additional touches are needed from the user now.
- Review caught that cursor edits/empty text reset pinentry's generated flag and
  could bypass reveal. Fixed: caret/selection keep state, partial changes preserve
  the reveal guard; clearing or complete replacement starts ordinary input. Native
  and library regressions prove this. `pinentry-reveal-regression.log`:12 tests;
  `pinentry-reveal-native/summary.json`:24 native checks, including real GnuPG.
- Kernel wakeup instrumentation probe: wakeup-instrumentation.json. Real host
  tracepoint ID is permission-denied at perf_event_paranoid2; /proc/self/sched has
  no nr_wakeups counter. Existing measurements are scheduler-sleep proxies only,
  never actual hardware wakeups. No kernel settings or tracing permissions changed.
- Full required `bash scripts/check.sh --workspace` started after cleanup. Live
  exec session9966, log workspace-final.log. No need to restart it: poll the same
  handle / inspect liveness. Early tooling and full workspace Clippy passed; all-
  target test compilation is underway. Current root free space58GiB as of last
  check. Optimized release build/archive and current greeter benchmark still next.

Workspace continuation details:
- First full gate (`workspace-final.log`, former exec9966) actually exited143
  (SIGTERM) during all-target test compilation. Tool returned terminal status;
  separate host process inventory confirmed no surviving cargo/rustc/linker.
  Tooling and strict workspace Clippy had passed; no Rust test failure was logged.
- Resumed the required gate with identical root target/features/debug/incremental
  selection and CARGO_BUILD_JOBS=2 to reduce memory pressure. Long-running wrapper
  `run-workspace.py` is now active: runnerPID792487, gatePID792489, start_ticks13048167.
  Poll /proc for identity/liveness AND workspace-final-2-result.json for termination;
  workspace-final-2-process.json alone is not proof of liveness. Latest verified
  state is S, same start identity, child dbus-run-session/Cargo all-target tests.
  Logs workspace-final-2.log; second tooling/Clippy/attribution checks passed,
  remaining application test executables are being linked. Root free space21GiB
  on last inspection. Do not restart a live job or launch measurements concurrently.
- `run-release-build.py` is prepared but NOT STARTED. It builds workspace bins
  optimized using existing target/release, one compile job, normal release profile.
  Start only after the workspace gate finishes; then package-release/smoke and
  isolated current Cage/Weston measurements remain. No build-profile flags carry
  into ordinary validation.
- User approved cleanup exception and completed actual Solo touches; these are
  resolved dependencies. No need to ask again. No host PIN/PAM/SSH config changed.

Second space recovery / current job:
- Full test compilation in gate2 exhausted the46GiB-to-go margin. Validation group
  was SIGSTOPped, then SIGINT/SIGCONT cancellation sent. Host inventory subsequently
  confirmed no surviving process in that group. No additional code failure proven.
- User explicitly approved the second reviewed list:102 previously retained but
  now superseded pre-goal executables,45.84GiB. Every file's path, type, link count,
  size, mtime, device and inode was verified before deletion. Evidence:
  additional-obsolete-executables.json and additional-cleanup-result.json.
  Actual free space recovered to46GiB; no cache relocation was performed.
- Current gate is `run-workspace-guarded.py`, source/profile/features/root cache
  unchanged, CARGO_BUILD_JOBS=1. Runner827894, gate827896, start_ticks13126651.
  Status/results/log files are workspace-final-3-process.json,
  workspace-final-3-result.json and workspace-final-3.log. A10GiB free-space guard
  cancels the child group automatically and records the reason. Do not restart
  while its /proc identity matches; do not mistake the process JSON for liveness.
  Latest verified S/same-start, progressing through early Python checks. The two
  earlier invocations passed strict workspace Clippy but never completed tests.
- Pending physical-touch and cleanup questions are all answered and completed.
  No further hardware actions needed now; real Solo/pam-u2f5 and pinentry native24
  passed. Current long-run need is workspace -> optimized release/archive smoke ->
  updated isolated Cage/Weston benchmark. Release runner is prepared, not started.


Final gate continuation (2026-11-02):
- Gate3 finished with seven parent failures in undo_unavailable_moves. It exposed
  previously masked canonical-binding errors for saved-only Rename returns;
  related focused tests then exposed the source-only Rename direction as well.
  The 19-line production fix preserves the complete original row digest and
  projects only the selected return portion. ADR0157 records the binding rules.
  Final focused run: recovery_catalog, undo_destinations, undo_replace,
  undo_unavailable_moves and undo_unavailable_saved all pass (36 parent tests,
  plus four intentionally ignored crash helper entry points).
- Gate4 is active with the final fixes; strict workspace Clippy passed and the
  complete Rust tests are running. Evidence: workspace-final-4-process.json,
  workspace-final-4.log and, on completion, workspace-final-4-result.json.
- The optimized workspace binary build is also active, normal root release
  profile, one build job. Evidence: release-build-process.json, release-build.log
  and release-build-result.json on completion. Both jobs have 10GiB disk guards.
  Do not treat a process record as proof of liveness or restart a matching process.
- All native authentication/Stasis/wallpaper suites and the existing desktop
  launcher smoke passed. User/device and exact-list cleanup dependencies above
  are resolved. Only deployment gate/build, current isolated greeter measurements
  and archive smoke remain. Benchmarks must wait until compilation/tests end.

- Gate4 passed all Files tests, then exposed the same removed AT-SPI CLI option
  in Notify's independent accessibility fixture. The shell and Files media
  fixtures also retained that option. Updated all three to --launch-immediately,
  memory-only GSettings and org.a11y.Status.IsEnabled on the private bus.
  No assertions or production accessibility behavior changed. The focused Notify
  test passes (notify-a11y-compat.log); its smaller package selection emitted the
  existing vendor iced_layershell unused-import warning, not a workspace Clippy
  failure. The required gate is now running as workspace-final-5 (runner946581).

- workspace-final-5 completed successfully (returncode0, 2026-11-02T00:15:09Z):
  tooling, strict workspace Clippy, all Rust targets, settings portal,
  configuration and vendor acceptance all passed, including both updated AT-SPI
  tests and all Files return cases. This is the required full workspace gate.
- The first optimized build failed at 00:18:52Z while writing an object file.
  The entire root target directory subsequently proved absent in both sandbox
  and native inspection; free space jumped from28GiB to188GiB. No agent command
  removed that directory: the only authorized deletions were the two exact old
  executable manifests recorded above. Sources/logs remain intact. Asked whether
  the cache was moved or another cleanup is active; do not infer its cause.
  Preserve the completed workspace/native evidence; cache loss does not undo it.

- Confirmed the missing target directory on the native filesystem as well as in
  the sandbox. No cargo/rustc/linker processes survived;10.4GiB RAM was available.
  Restarted only the required optimized build at the normal root target path,
  same workspace/bin/features/profile/lock selection, two jobs. No new cache
  location or ordinary-check measurement flags. Retry runner1002719; evidence
  release-build-2-process.json, release-build-2.log, release-build-2-result.json.
  It is compiling successfully. The prior workspace gate remains a recorded pass;
  source changes are unchanged. Do not rerun that gate just because outputs were
  lost. Native release greeter checks (both compositors), isolated comparison,
  updated report/default choice, final archive smoke remain after this build.

- Optimized build retry PASSED at2026-11-02T00:43:26Z (20m47s), all14 shipping
  executables available. Normal workspace release profile, root target directory,
  two jobs. Release Cage native acceptance is running (greeter-release-cage.log).
  Next Weston native acceptance, isolated five-pair comparison and updated report,
  final archive packaging/smoke. No source changes or repeated workspace gate.

- Release greeter matrices initially timed out at avatar-override under both Cage
  and Weston. Isolated avatar override passes. Root cause is the private greetd
  fixture's deliberately blocked10-second cancellation from the preceding case:
  the frontend exits within its deadline, but the old test peer request later
  cancels the next case's global authentication state. Added client lifecycle
  accounting and an explicit zero-live-client drain after that scenario, scoped
  from before Escape. Assertions and application binaries are unchanged.
  Re-running native release matrices with independent scenario cleanup.


Final outcome (2026-11-02):
- All implementation phases complete; required full workspace gate passed.
  Final release binaries built successfully. Cage and Weston each passed the
  35-check native greeter matrix after draining stalled fixture connections.
- Controlled release comparison passed: five alternating pairs,60seconds idle
  each, no concurrent builds/acceptance. Cage100.5ms median startup/0.213% of
  one core/45.1MiB summed RSS; Weston85.5ms/0.243%/71.0MiB. Cage retained as
  default for25.9MiB(36.4%) lower RSS; startup ranges overlap. Updated report and
  full samples/metadata/source snapshots live in docs/greeter-performance.md and
  docs/performance/data/2026-11-02-greeter-followups/. Both frames inspected.
- Final archive built and smoke PASSED:14 executables, configuration, private copy
  backend, provider close protocols,47 documentation files/68 offline links.
  The smoke reader was corrected to parse every v5 JSON line and validate ordered
  revisions instead of assuming a single JSON response; binaries were unchanged.
  Archive greeter hash matches the measured executable. Checksum stored alongside
  archive; machine-readable final result is validation-summary.json.
- Physical Solo/pam-u2f unlock passed on a private graphical/PAM seat; no host
  PAM, SSH, services or display-manager configuration changed. Physical DRM/VT,
  real kernel suspend/resume and distribution policy remain test-installation
  acceptance as documented. Direct Stasis scheduler-wakeup tracing was unavailable
  due to tracepoint permissions; context switches are explicitly only a proxy.
- Source, documentation and maintained fixtures are committed; local execution
  artifacts remain outside the changeset. Historical scratch/cache-deletion
  evidence is preserved locally.
