# Canopa greetd greeter

Status: implemented; scoped model, native UI and real-greetd transport acceptance passed

Deliver a software-rendered Canopa login interface backed by greetd, sharing the
lockscreen presentation without inheriting its current-user PAM/session-lock code.

- Shared renderer, layout, secret input and prompt presentation; existing wallpaper
  and lock behavior stays covered by regression checks.
- Dedicated greeter binary with system configuration/assets, manual/configured
  user selection, configured session selection, keyboard and pointer input.
- Bounded greetd framing, multi-step visible/secret/info/error conversations,
  cancellation, retry, timeout, disconnect handling and authenticated session handoff.
  No success/handoff after stale/cancelled operations. Greetd owns PAM and privileges.
- Fullscreen xdg-shell surface, software buffers, output/scale changes and kiosk
  compositor integration. No privileged display-manager implementation.
- CLI/config tooling, release/updater integration, example greetd/Cage and Canopa
  session configuration, setup documentation and architecture record.
- Focused model/protocol/config tests; private compositor and greetd protocol fixture
  acceptance; existing lock regression; required workspace gate and failure record.

Host display-manager/PAM configuration and the live login session are not changed.

## Result

`canopa-greeter` uses greetd for authentication and session lifetime, runs as an
unprivileged Cage/Wayland client, and shares presentation and passive widgets
through `canopa-session-ui`. Session retains its existing lock/wallpaper module
paths and roles. CLI, configuration validation, updater, release packaging,
greetd/Cage examples and the Canopa desktop-session entry are integrated.

Interactive authentication supports fresh secret/visible prompts, information,
errors, OTP and passwordless-device flows. Generation checks gate handoff;
cancellation requires an acknowledged reset and drains pending replies. A real
greetd 0.10.3 test exposed a dead-worker cleanup error after failed authentication;
the client now requires a further successful reset before retry. Commands are
quoted for greetd's shell launch and bounded with their environment to 8 KiB.

## Validation (2026-11-01)

- `cargo test --locked -p canopa-greeter -p canopa-session-ui -p canopa-session
  -p canopa-widgets --lib --test configuration --test greetd_transport`: **20
  passed**; one real-backend test intentionally ignored in the ordinary runner.
  Includes frame bounds, stale-generation refusal, deadline/cancel scheduling,
  source/config isolation, literal shell argv and secret erasure. See
  `focused-final.log`.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
  `cargo fmt --all -- --check` and `git diff --check`: passed. Application debug
  builds for greeter, session tools and CLI passed using the shared root target.
- `scripts/verify-greeter.py --cage ... --output
  .scratch/greeter/accepted-cage`: **22 passed**, using private Weston/Niri and
  real Cage 0.3.1 with a test-owned greetd protocol peer. Covers keyboard/pointer,
  repeat editing, users/sessions, password/visible OTP, failed-worker cleanup,
  device cues, late success after cancellation, timeout/retry, launch refusal,
  malformed/disconnected peers and scoped command cleanup. Screenshots,
  protocol metadata and summary are in `accepted-cage/`.
- `scripts/test-greetd-transport.py --greetd
  /tmp/canopa-greeter-tools/usr/bin/greetd`: **passed** against greetd 0.10.3
  (Arch package 0.10.3-2). Uses private user/PID/network namespaces, private `/run`,
  fixture account records/PAM and an initgroups shim. Verified actual PAM
  wrong-password retry, post-greeter session execution and literal argv/env.
  Binary hashes and logs are in `real-backend/`. This does not establish real
  privilege/seat or installed PAM-policy acceptance.
- Existing native lockscreen/logind regression: **23 passed**, including actual
  loginctl wire calls to the private service; see `lock-regression/summary.json`.
- Existing native wallpaper/lock regression: **14 passed**, including selected/all
  dynamic updates, scale and fail-closed locking; see
  `wallpaper-regression/summary.json`. Wallpaper idle sample: 0 CPU ticks over
  two seconds, 12,108 KiB RSS (debug build, this fixture only).
- Updater tests (20), check-selection tests (8), desktop-runner tests (8), session
  unit checks, configuration/migration/privacy checks, portal private activation,
  vendor checks and locked dependency inventory checks passed during the workspace
  validation. Documentation staging: **37 files, 53 offline links**. Final desktop
  entry passes `desktop-file-validate`; new Python sources compile.

## Workspace failures and acceptance limits

`bash scripts/check.sh --workspace` ran after shared edits. Its initial stale-lock
preflight was corrected; the actual gate then passed preflights, formatting and
strict Clippy, built all targets, and stopped at the previously observed Files
`recovery_catalog::pages_remain_loaded_and_saved_identity_survives_active_undo_expiry`
failure (`Recovery catalog did not settle`; three sibling tests passed). See
`workspace-preflight.log` and `workspace-check.log`.

A no-fail-fast continuation excluding Files completed all remaining targets.
Only Notify's native accessibility test and Shell's native Center accessibility
test failed with the host AT-SPI launcher's rejected command line/connection;
the other 154 Notify and 1268 Shell tests passed. See
`workspace-continuation.log`. These failures remain unresolved, so this work does
not claim a successful full-workspace gate.

Host activation is an administrator deployment step described in
`docs/greeter.md`. Physical VT/DRM, real device enrollment, privilege switching,
logind setup and the installed distribution's greetd/PAM stack remain target-machine
acceptance. The current interface has no AT-SPI tree or complex-script shaping.
Greetd/PAM may serialize cancellation behind a blocked device operation; the UI
fails closed when its bounded cancellation is not acknowledged, while backend
cleanup remains subject to module timeouts. No host login policy was modified.
