# Wallpaper and lockscreen

Status: in progress

Add two independent lightweight Wayland executables in canopa-session. Wallpaper
uses background layer-shell surfaces, CPU image preparation, compositor-owned
shared-memory buffers and an event-driven command socket. Commands select an exact
output name or all outputs; changes take effect without a daemon restart. New
outputs inherit the active default and configured per-output overrides.

The locker uses ext-session-lock-v1, PAM, software rendering, shared Canopa
appearance, a clock/date/password view, caps-lock feedback and optional wallpaper.
It is launched on demand; compositor lock acknowledgement is the readiness boundary.
Failure, Escape, signals and a crashed authentication worker never authorize unlock.
Veila is a product reference, without copying its GPL source. Passive battery,
media, keyboard, username, avatar and weather widgets are included. Idle policy
and suspend orchestration remain external.

## Validation evidence

- Focused Rust checks: canopa-session, canopa-widgets, canopa-config and canopa-cli
  compile. Configuration/rendering/secret/PAM-conversation/socket/fresh-weather tests
  pass. Strict Clippy passes for the new crates and CLI.
- Native Weston/Niri acceptance passes against production debug binaries using a
  bubblewrap-isolated PAM fixture: live image changes, failed candidate retention,
  duplicate daemon/lock exclusion, fractional scaling, readiness, failed password
  and account rejection, successful authentication, and authenticated recovery after
  locker death. See evidence/summary.json and retained logs/screenshots.
- Standalone native measurement after allocator scratch release: 10,580 KiB VmRSS and
  zero CPU ticks over two idle seconds, one 1280x800 output, debug build. This is a
  scoped local observation, not a release benchmark or multi-monitor hardware result.
- Initial native sampling mistakenly overlapped a scale repaint; the fixture now
  waits for buffer submission before sampling. Initial recovery expected refusal,
  but Niri securely permits replacing dead lockers. Updated to verify desktop
  hiding and fresh PAM authentication for the replacement.
- Full workspace gate is running. Earlier attempts stopped at the missing native
  manifest entry (fixed). An updater test run caught an optional-service iteration
  error (fixed); the current gate passed all 19 updater tests and all 8 check-script
  tests, plus the companion-unit dependency verification. Workspace Clippy passes;
  the complete workspace test build is running.

- Combined-feature Clippy initially exposed a non-Sync Wayland C-backend insertion
  error that standalone builds did not instantiate. The boundary now converts that
  insertion error to an owned diagnostic; the workspace gate resumes from Clippy.
- Release staging includes both executables, the wallpaper unit, PAM examples and
  all session/widget licenses. Packaging shell syntax and staged offline Markdown
  links (30 files, 46 links) pass. No release archive or host installation was made.

## Boundaries

Runtime wallpaper commands do not rewrite startup configuration. Weather uses a
local snapshot provider; idle/suspend policy stays external. The lockscreen accepts
one password challenge, with no OTP/password-change flow. Software text has no
AT-SPI bridge or complex-script shaping. Private native validation covers one Niri
output and fractional scale; physical multi-output/hotplug acceptance is pending.

## Workspace regression result

The broad `cargo test --workspace --all-targets --locked` run stopped at
`canopa-files --test recovery_catalog`, test
`pages_remain_loaded_and_saved_identity_survives_active_undo_expiry`: its existing
10-second wait reported “Recovery catalog did not settle.” Re-running that named
target with the same workspace graph reproduced the timeout (3 other tests pass).
No files under crates/canopa-files were changed. The full workspace gate cannot be
reported green. Remaining non-Files packages and the gate's configuration/portal/
vendor checks are being completed separately.

Final review added solid-theme fallback for unavailable lock wallpapers; a missing
image must not stop acquisition of the session lock. The private native fixture now
exercises this path in addition to authentication and compositor crash recovery.

The non-Files workspace continuation stopped at Notify's existing
`native_accessibility_exposes_content_actions_and_retires_old_controls` fixture:
`at-spi-bus-launcher --launch-immediately --a11y=1` rejected its arguments and the
fixture's accessibility connection timed out (154 other Notify unit tests pass).
No Notify source was changed. Affected Shell battery/media/footer paths and session
tools are checked explicitly; these two wider failures remain reported.

Explicit affected-path checks pass: Shell's 3 UPower tests, 40 battery tests,
8 media tests, bundled session footer test, both configuration integration targets,
all five product configuration validators, and the Settings portal frontend/DBus
activation/reload test. Strict whole-workspace Clippy passes after the final lock
background fallback and duplicate-widget-update suppression.

The final vendor test initially stopped because its offline temporary graph needs
an iced_beacon registry entry absent from this fresh cache. Vendor source/provenance
verification itself passes, with no vendor changes.

Final native acceptance also passes against the combined-workspace production
debug binaries (all 14 checks, including unavailable lock wallpaper fallback).
This combined graph measured VmRSS:	   13584 kB and
0 CPU ticks over two idle seconds. The native
PAM fixture and compositor stayed isolated from the host.
