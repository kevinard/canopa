# Supported installation and Canopa Settings v1

Status: implemented; private acceptance passed. Clean-chroot installation and
physical-seat acceptance remain separate requirements.

User request: implement roadmap items 2 and 3, preserving the completed build
disk guard and unrelated scratch housekeeping. Arch Linux x86-64 is the first
package target. No host packages, display-manager configuration or services were
installed, enabled or replaced during this work.

## Delivered scope

- [x] Dedicated Canopa session, missing-file initialization, managed/custom Niri
  includes, system installation staging, PAM/defaults, startup/logout recovery,
  graceful signal cleanup and an event-driven session wait without idle polling.
- [x] Standalone shared-theme Settings: displays, XKB/repeat/mouse/touchpad,
  appearance, existing wallpaper picker, lockscreen presentation and Stasis
  status/pause/resume/profiles/native timeouts.
- [x] Independent timed display confirmation and rollback on timeout, GUI EOF,
  GUI SIGKILL, early disconnect before readiness and partial application failure.
  Exact source validation, stale-edit checks, atomic bounded saves and preservation
  of unrelated namespaces, custom options, comments and idle policy.
- [x] Three Arch packages, a private pinned compatible Niri executable, source and
  binary provenance, 612 application and 315 compositor dependency inventories
  with original notices; CLI/desktop/release/updater/docs and CI integration.
- [x] Focused, workspace, native, package and portable validation below. Generated
  evidence stays under `.scratch/artifacts/desktop-alpha/`; Cargo caches remain
  under the guarded root target with ordinary incremental debug checks.

## Acceptance evidence

All paths below are relative to `.scratch/artifacts/desktop-alpha/`.

- `workspace-tests-retry.log`: complete workspace/all-target tests pass. The first
  full gate stopped on an unchanged Files Recovery race (`Finished` vs `Failed`);
  `recovery-exact-retry.log` passes with the exact same executable, and the whole
  test suite then passes with failure continuation. No Files code was changed.
- `clippy-ci-final.log`: workspace/all-target Clippy passes with warnings denied.
  Configuration, portal and vendor checks pass in `configuration-tests-final.log`,
  `portal-tests.log`, and `vendor-tests-final.log`. The earlier configuration check
  ran before its CLI prerequisite was built; the documented-order rerun passes.
- `backend-portable-ci-final.log`: 14 portable Settings backend tests pass with
  the native validator explicitly unavailable. `backend-native-parser-final.log`:
  both separately selected native parser tests pass with the verified compositor,
  covering generated defaults/displays, input and rejection of the exact invalid
  replacement candidate. Desktop CI runs those two tests explicitly.
- `install-tests-complete.log`: ten installation/tooling tests pass, including
  real process-boundary cleanup for SIGTERM, SIGINT and SIGHUP, unit graph cycles,
  isolated session paths, provenance, inherited notices and source-archive docs.
  Eight script-contract tests also pass in `check-script-tests-final.log`.
- `native-package-verified/summary.json`: ten native checks pass against the actual
  extracted Arch Settings executable. They cover batched keyboard navigation,
  live input save, display confirmation and reload, Escape and GUI-crash rollback,
  repeated appearance/lockscreen saves, real wallpaper picker launch, unavailable
  idle backend, and real Stasis pause/resume/profiles/timeout reload preserving
  custom policy. Stasis 1.7.0 SHA-256:
  `6a428db147872889353ffa73ca75028563aa98b12099979305bdecb7b4c654ca`.
- `compositor-cancellation-final/summary.json`: the unchanged strict independent
  GTK source/receiver cancellation gate passes on the actual Arch-flags compositor.
  Its SHA-256 is
  `46ae707e5229ff12017dddcffd78ee523e3ac548e8c754e0b587ad80835392df`.
- `niri-resume-tests.json`: pristine sources resume, while modified/extra files
  and a forged manifest receipt are rejected against independently checked input.
- `package-source-tests.json`, `compiled-source-check.json`, `package-checks.json`:
  reproducible source snapshots exclude scratch/cache/Git data; compiled source
  bytes match the archive; all three packages extract privately; all 16 ELF
  payloads resolve native libraries; session/desktop entries, 77 offline doc links,
  dependency notices, binary provenance and actual CLI initialization validate.
- `portable-verified-smoke.log`: the fifteen-binary companion archive executes
  and passes configuration, linked Copy/Move/Undo and v1/v5 provider checks.

## Failures found and corrected

Earlier logs and executable/source snapshots are retained. Actual native parsing
caught presence-flag and KDL version issues; an invalid-candidate regression caught
an include replacement retaining stale formatting. Repeated saves exposed lost
KDL node separators, now preserved with widget comments. Stasis's watch/info
schemas and exit-zero error replies are handled explicitly. Native keyboard tests
caught focus loss when inserting notices; the notice tree remains stable. Picker
startup errors are reported in Settings and the fixture supplies its real owner
and image library.

The first packaged native run caught a GUI-crash window before helper readiness:
`println!` panicked on the closed pipe before rollback. Fallible writes now route
through restoration; a deterministic closed-reader-before-request test and the
actual release native rerun pass. Session signal termination was separately
regression-tested and fixed to stop the owned target, reap its client and clear
activation variables.

## Remaining external acceptance

The private makepkg build used `--nodeps`: the host lacks clang and cmake packages.
A checksum-verified official Clang library/resource tree was staged privately.
This establishes executable/package validation, not a clean-chroot dependency
build. Physical DRM/VT/login-manager/PAM, hardware suspend/resume/touch and actual
package-manager installation/upgrade/downgrade need a separate test installation.
The entire hosted native matrix has not been run; scoped results above do not
claim complete Files or desktop product acceptance.

Unrelated `.gitignore`, issue-tracker and session-followup housekeeping is
preserved separately from the installation and Settings implementation commit.

## Final artifact identity

The final three packages are in `arch-final/`. The final source archive SHA-256 is
`ac71a47081c5fa495386118523709c0fe486f1d88a40d0634d24c83270b7d6ae`.
All 1,980 extracted source files match it. The final metadata refresh changed
only workflow/tests/README/ADR records: every executable is byte-identical to the
payload accepted in `native-package-verified/summary.json`. Settings SHA-256 is
`7d964f82efba5143585425432850ebe89c32ac55f892bf592f316edbd9a146ba`.
Final payload inspection passes in `package-checks-complete.log` and
`package-checks.json`; source/runtime identity passes in `compiled-source-check.json`.
