# Wallpaper and lockscreen

Status: complete (implementation and scoped acceptance); full workspace gate has
existing Files/Notify fixture failures listed below.

## Delivered

- `canopa-wallpaper`: independent software-rendered background layer-shell daemon,
  exact monitor/all-monitor selection, live control socket, per-output startup
  overrides, configuration reload and hotplug reconciliation. Runtime commands do
  not rewrite configuration. No redraw timer, image polling, Iced or GPU device.
- `canopa-lock`: on-demand ext-session-lock-v1 locker, PAM authentication and account
  checks, compositor-confirmed ready FD, clock/date/password/caps/layout UI,
  configurable appearance, wallpaper inheritance and safe solid-color fallback.
- `canopa-widgets`: shared battery/media models and UPower adapter used by Shell and
  Lock, read-only observation of Shell's selected player, bounded local weather
  snapshots. Lock also supports username/avatar. Consumers own presentation and
  actions; no shell actions enter the locked session. Veila is a product reference;
  its GPL source was not copied.
- Product CLI, configuration namespaces/validators, default Lock action, user unit,
  release packaging, distro PAM examples, CI dependencies and setup documentation.

## Validation

Pass:

- Strict `cargo clippy --workspace --all-targets --locked -- -D warnings`, formatting,
  dependency attribution and vendor provenance checks.
- Session configuration, image fit/alpha, IPC exclusion/admission, secret handling,
  PAM-conversation and fresh-weather tests.
- Affected Shell paths: 8 media, 3 UPower, 40 battery tests and session footer action.
- Both configuration integration targets; all five product validators and migration;
  Settings portal private-bus activation, read/read-all and reload signals.
- All 19 updater tests, 8 check-selection tests, companion-unit dependency checks,
  acceptance manifest registration, packaging shell syntax and staged offline docs.
- Vendor library/input/pipe tests. Its temporary metadata step needed online cache
  population for optional iced_beacon/bincode entries; the root-locked active graph
  guard and offline library test commands remained in force. No vendor files changed.
- Production debug binaries from the combined workspace pass all 14 private
  Weston/Niri checks: selected/all live wallpaper changes, invalid image/reload
  retention, daemon exclusion, fractional scale, idle resource use, lock readiness,
  duplicate-lock refusal, missing lock image fallback, failed password, Escape,
  account rejection, PAM success, desktop hiding after locker death and authenticated
  replacement-locker recovery. Fixture PAM stays inside a bubblewrap mount namespace.

Latest measurement: one virtual 1280x800 output, combined debug graph, 13,584 KiB
VmRSS and zero CPU ticks across two idle seconds for Wallpaper. The earlier
standalone debug graph measured 10,580 KiB after decoder scratch release. These are
local observations, not release or physical multi-monitor benchmarks.

The full `bash scripts/check.sh --workspace` gate is not green:

1. Files `recovery_catalog::pages_remain_loaded_and_saved_identity_survives_active_undo_expiry`
   times out with “Recovery catalog did not settle”; the named target reproduces it
   in isolation (3 other tests pass). No Files source changed.
2. The non-Files continuation reaches Notify's
   `native_accessibility_exposes_content_actions_and_retires_old_controls` fixture;
   the host AT-SPI launcher rejects its arguments and the connection times out
   (154 other Notify unit tests pass). No Notify source changed.

See `validation-notes.md`, `evidence/summary.json` and retained logs/screenshots.

## Boundaries

This records the original delivery scope. Logind integration, interactive multi-step
PAM and custom widget/layout support are subsequently delivered in
`../lockscreen-integration/PRD.md`; the setup guide is the current product reference.

Install the distro-appropriate `/etc/pam.d/canopa-lock` before using the Lock button.
Host installation and service activation were not performed. Weather collection,
idle/suspend coordination, multi-step OTP/password-change conversations, AT-SPI and
complex-script shaping are outside this feature. Physical multi-monitor/hotplug
acceptance remains unverified; native checks use one virtual output and fractional
scale. Startup configuration persists wallpaper choices; live assignments last
until reload/exit.
