# Desktop, accessibility and vendor follow-up — 2026-09-23

This continues the [audit remediation](2026-09-23-remediation.md). Nix packaging
was explicitly deferred by the owner; no Nix installation or package was added.

## Changes

- Published layer-shell 0.18.0 archives were verified against Cargo registry
  checksums and compared file by file. The complete inventory and offline CI
  drift check are in [vendor maintenance](../vendor-maintenance.md).
- Default muted-volume and expanded-module contrast failures were corrected.
  Module/menu focus states and active workspaces now have contrasting outlines.
  Module frames reserve an inset so workspace fills cannot cover focus borders.
- Real compositor screenshots exposed empty framed rows and invisible keyboard
  targets. Bar/panel rendering, navigation and tooltips now share the module's
  content policy. Unavailable sources, empty script presentations and
  output-mismatched workspaces leave no frames or gaps; recovery restores
  content. Source loss also retires stale hover, focus and tooltip state.
- [ACCESSIBILITY.md](../../ACCESSIBILITY.md) records 21 passing default color
  pairs, the calculation method, AT-SPI evidence and explicit coverage limits.
  CI checks the numeric color pairs on every change.
- `scripts/validate-desktop.py` creates an isolated compositor session, records
  CPU/RSS samples and layer registration timing, captures screenshots, and
  checks repeated panel open/close using IPC and virtual-keyboard Escape.

## Measurement method and evidence

The workload uses the unchanged repository default configuration: workspaces,
focused window, volume, network, battery and clock. Weston supplies a headless
output to a nested Niri instance. Both use private runtime/config directories
and session D-Bus; system D-Bus and audio are unavailable. Mesa software rendering
and the GL WGPU backend are requested. This tests a reproducible degraded-service
workload and does not establish normal hardware performance.

Both binaries use the same compiler and Cargo release profile (thin LTO, one
codegen unit, stripped symbols, abort-on-panic). The baseline is audit commit
`0dbd2697f79211bb8ab6900037cce2f333dcd52d`; the candidate includes `5e50352` and
this follow-up's contrast, visibility and keyboard fixes. Binary/config hashes
and the compiler label are recorded with the raw results.

Host: Arch Linux, Linux 7.0.12, Intel i5-6200U (2 cores / 4 threads),
7.6 GiB RAM, Mesa 26.1.2, Weston 15.0.1, Niri 26.04 (`8ed0da4`),
and wtype 0.4. The virtual output is 1280×800 at scale 1. The host desktop
remains running; CPU load is not controlled beyond avoiding concurrent builds.

Each process settles for 60 seconds, then samples `/proc/PID/stat` once per
second for 60 seconds. CPU is percent of one core, calculated from total process
CPU ticks divided by measured wall time. RSS excludes compositor and separate
GPU allocations. Layer registration is observed through Niri IPC polling;
it is not the time to the first visible, fully populated frame. The five baseline
runs precede the five candidate runs. Caches and CPU frequency are not pinned,
so small timing differences should not be interpreted as causal improvements.

An [initial attempt](data/2026-09-23-desktop/baseline-initial/summary.json)
stopped on the second run's keyboard-close check. Only the screenshot cycle had
waited for panel rendering; later cycles sent input immediately after layer
registration. The harness now uses the same one-second panel settling interval
for every cycle, after CPU/RSS sampling. The initial attempt is retained and
excluded from the comparison below. Both optimized binaries then passed
[five short timing probes](data/2026-09-23-desktop/keyboard-timing-probes.json)
each, including ten panel cycles per binary, before the full measurements.

A [second attempt](data/2026-09-23-desktop/baseline-idle-timeout/summary.json)
passed two runs, then Niri stopped answering layer queries after the session had
been active for more than five minutes. Weston's installed manual documents a
300-second default idle timeout; this is the likely cause in the input-free
parent compositor. The harness now explicitly passes `--idle-time=0`. This
attempt is also retained and excluded from the final comparison. With idle
timeout disabled, the full ten-minute baseline completed all five runs and ten
panel cycles, including the previous failure point.

| Metric: median (range across five runs) | Baseline | Candidate |
| --- | ---: | ---: |
| CPU, percent of one core | 0.017% (0.017–0.033%) | 0.100% (0.083–0.117%) |
| RSS | 152.5 MiB (147.5–161.3 MiB) | 153.6 MiB (153.5–163.0 MiB) |
| Layer registration | 106.7 ms (96.0–273.1 ms) | 103.2 ms (99.0–258.6 ms) |

Candidate median CPU increased by 0.083 percentage points of one core
and median RSS by 1.1 MiB. The CPU increase is explicit: the baseline
stops failed data sources, while the candidate retains recovery retries. The
missing system bus is retried every two seconds and failed module sources back
off to 30 seconds. This is consistent with additional background work, but the
benchmark does not isolate its exact CPU contribution. One process CPU tick
over a 60-second window is about 0.017 percentage points; small differences
also reflect that counter resolution. No healthy-service performance or Waybar
acceptance result is assigned from this experiment.

All ten measured runs passed two panel cycles each, including keyboard Escape.
The optimized candidate screenshot has the clock/calendar content without the
five blank rows visible in the baseline.

Raw evidence: [baseline summary](data/2026-09-23-desktop/baseline/summary.json),
[candidate summary](data/2026-09-23-desktop/candidate/summary.json),
[baseline metadata](data/2026-09-23-desktop/baseline/metadata.json), and
[candidate metadata](data/2026-09-23-desktop/candidate/metadata.json). Each directory
contains all five per-second sample files, compositor/application logs, and
screenshots. Compare the [baseline panel](data/2026-09-23-desktop/baseline/panel.png)
with the [candidate panel](data/2026-09-23-desktop/candidate/panel.png).

This comparison does not establish the PRD's CPU/memory targets relative to
Waybar. GPU rendering, large application directories, busy tray traffic, open
external providers, real service recovery and mixed-scale outputs still require
matched workloads on the target desktop.

## Accessibility and desktop boundary

The native Canopa AT-SPI regression passed against a private accessibility bus
and registry. It checks semantic names/roles, rich and styled text, image
alternatives, bounds, hyperlink URIs, programmatic focus/state, action delivery,
replacement, rejection of stale actions and retirement. The test is recursive:
its parent and isolated child print two passing lines for one logical test.

The compositor harness waits one second after each panel registers, sends Tab
and verifies Escape removes the open Panel layer. This checks interaction after
settling, not immediate keyboard readiness. Tab delivery alone does not prove
correct traversal order. Screenshots can demonstrate the tested bar and panel
appearance, not every interaction state.
Orca, general bar/menu accessibility, complete keyboard traversal and focus-area
requirements have not been accepted by an operator.

No real Wi-Fi connection was changed, no user compositor/audio service was
restarted, and no release was published. The remaining
[desktop matrix](../manual-validation.md) stays open for hardware acceptance.

## Verification and review

- `cargo test --locked --all-targets -j 2`: **1,179 tests pass** (17 library,
  1,144 application, 6 Canopa transport, 2 provider protocol and 10 Wi-Fi backend
  tests). Three new regressions cover empty presentation, service loss/recovery,
  and Escape after loss of the focused module, including the last visible target.
  Older navigation fixtures now explicitly supply ready module state.
- Rust 1.95 formatting and `clippy --locked --all-targets -- -D warnings`: pass.
  This checks every test target against the declared minimum/CI compiler. Runtime
  tests and optimized builds use installed Rust 1.97 nightly
  (`14196dbfa`, 2026-04-12), not the CI release compiler.
- Vendor verification passes both offline and against the published archives.
  Deliberate changed, missing and extra-file probes were rejected.
- All 21 numeric contrast pairs pass. Formula probes cover equal colors, black
  against white, and grays immediately above/below the small-text threshold.
- The short compositor smoke check passes and its panel screenshot confirms
  unavailable/empty modules no longer leave blank rows.
- `cargo build --locked --release --bins -j 2`: passes with the actual optimized
  profile. The packaged archive was extracted and tested with a clean PATH;
  both binaries execute, and the Wi-Fi provider handshake and Close complete.
  The local archive is `target/audit-release/niribar-x86_64-unknown-linux-gnu.tar.gz`.
- [Verification output](data/2026-09-23-desktop/verification.txt) and the separate
  [native AT-SPI run](data/2026-09-23-desktop/at-spi.log) are retained.

Standards review found incomplete process-group cleanup, missing SIGTERM
cleanup, focus borders that opaque workspace pills could cover, and Escape
failing to release keyboard ownership after its focused module disappeared.
Spec review also identified the cleanup issue and the low-contrast
active-workspace state.
All were corrected and reviewed again without further findings. The cleanup
probe confirmed that a SIGTERM-resistant helper is retired even after its
leader exits; a timed termination probe also stopped the private compositor.

Review totals: Standards 4 findings addressed; Spec 2 findings addressed.
