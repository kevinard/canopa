# Shared asset validation

Local validation on 2026-09-27 against baseline `c25b293`, scoped by
[ADR 0005](adr/0005-shared-assets.md). Builds use Rust 1.95.0,
`CARGO_TARGET_DIR=niribar/target`, `CARGO_INCREMENTAL=0`, and one Cargo job.

The new public asset interface has nine targeted tests covering caller format
isolation under Cargo feature unification, image/encoded/pixel budgets, cancellation,
nonblocking FIFO rejection, symlinks, immutable prepared pixels, thumbnailing,
indexed size/scale and parent selection, first-index precedence, shell SVG support,
indexless hicolor compatibility, malformed indexes and root exhaustion. Existing
Notify tests continue to exercise its detailed image/icon limits, EXIF transforms,
fresh frame metadata, worker lifetime, supersession and display acknowledgment.
Niribar adapter tests cover actual SVG selection and its smaller Center limits,
thumbnailing and GIF support.

Review found an allocation-accounting difference between the former image entry
points. The initial extraction omitted the output reservation made by
`ImageReader::decode`. The fixed policy preserves that reservation for Center and
retains Notify's former decoder-only accounting. A two-by-two GIF regression proves
the distinction: a 15-byte decoder-only allowance accepts its 16-byte output, an
output-inclusive allowance rejects it, and a 16-byte inclusive allowance accepts
identical pixels. The targeted regression passed after the fix.

| Review axis | Remaining findings | Resolved issue |
| --- | --- | --- |
| Standards | 0; no remaining material issue or meaningful heuristic smell | Preserve the Center's destination-buffer allocation reservation |
| Spec | 0; no remaining material issue | Keep the distinct allocation policies instead of silently relaxing Center limits |

Completed checks:

- Workspace type-check passed.
- `bash scripts/check.sh --core`: **354 tests passed**, none failed or ignored.
  Its strengthened dependency check confirms the headless Notify graph excludes
  `canopa-assets`, image codecs, GUI and audio adapters.
- The eight initial asset tests and the additional allocation regression passed.
  Logs: `/tmp/canopa-assets-tests.log`, `/tmp/canopa-assets-check.log`,
  `/tmp/canopa-assets-core.log`.
- `bash scripts/check.sh`: **1,666 workspace tests passed**, none failed or ignored,
  across 49 test executables. This excludes one nested subprocess rerun of a shell
  test. Four local-fork tests and seven updater tests also passed, along with
  formatting, strict Clippy, the 551-package attribution inventory, all 42 contrast
  pairs, configuration CLI fixtures, companion-unit ordering and provenance checks.
  Log: `/tmp/canopa-assets-full.log`.

The expanded desktop fixture first passed against the previous packaged binaries.
Its baseline screenshots show the same local PNG in banner and Center, an indexed
PNG banner icon, and a named SVG icon in a rich dmenu entry. It also exercises
native sender actions, keyboard focus, missing-image alternatives, palette reload,
DND and independent process restarts. Baseline artifacts:
`/tmp/canopa-assets-baseline-desktop-allowed`. These are private Niri/Sway sessions;
the sandbox-only first attempt could not bind its D-Bus socket and was rerun with
the existing desktop-fixture permission.

The rebuilt dark/scale-1.0 desktop fixture passed in `/tmp/canopa-assets-dark`.
Its PNG and SVG color regions have identical pixel counts and bounding boxes to
the baseline screenshots; the rendered images and icons were also inspected.
Comparison evidence: `/tmp/canopa-assets-visual-comparison.json`.

The initial light/scale-1.25 case exposed a fixture assumption: the added image
pushed the fourth Center action below the viewport, where AT-SPI correctly omitted
it. The same failure reproduced on the previous binaries. The fixture now sends
Page Down when the action is offscreen before seeking native activation. No
production scrolling or accessibility change was needed.

The corrected light/scale-1.25 fixture passed against both builds:
`/tmp/canopa-assets-baseline-light-scrolled` and
`/tmp/canopa-assets-light-scrolled`. Both report actual action focus, native sender
activation after scrolling, PNG/SVG rendering, palette reload, DND and independent
restart recovery. The rebuilt light screenshot was inspected as well. This closes
the original failure reproduction without changing production code or weakening
the action assertion. All desktop runs used Niri 26.04 and Sway 1.12.

Packaging and `scripts/smoke-release.sh` passed for the rebuilt debug binaries.
All four executables launch, the bundled configuration validates, and the Wi-Fi
provider handshake and close succeed. The archive includes the new asset crate's
MIT notice with both original contributor attributions. Archive:
`/tmp/canopa-assets-package/canopa-x86_64-unknown-linux-gnu.tar.gz`; logs:
`/tmp/canopa-assets-package.log` and `/tmp/canopa-assets-package-smoke.log`.
This validates package layout using debug binaries, not an optimized release build.

No user configuration, installed service or live desktop session is changed. The
fixtures do not establish full Orca usability, XPM decoding, global icon-cache
invalidation or hard filesystem/memory deadlines.
