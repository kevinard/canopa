# Idle CPU and memory

This pass targets the default Shell bar and graphical Notify while idle. The
baseline is workspace commit `4c6470e`; the candidate adds event-driven volume
observation and idle banner heap reclamation. Both use the workspace's release
profile (thin LTO, one codegen unit, stripped symbols, abort on panic).

## Changes

Shell's default-output volume observer previously checked its Pulse connection
every 50 ms. Pulse state changes and receiver cancellation now wake a parked
connection-owning thread. Volume changes still arrive through subscriptions;
initial connection deadlines and audio panel device observation retain their
existing behavior. See [Shell ADR 0022](shell/adr/0022-event-driven-volume-observation.md).

Notify requests best-effort release of free GNU allocator pages two seconds after
the last banner surface closes. A new stack cancels that delay; retired-window
identity and current surface state reject stale requests. One blocking-executor
job may be admitted at a time, including queued work. Once admitted it can finish
while new content arrives. It does not block the UI thread or run periodically
while idle. See [Notify ADR 0053](notify/adr/0053-idle-banner-heap-reclamation.md).

## Reproduce

Run from the Cargo workspace root on Linux with the native desktop fixture
dependencies plus PipeWire, pipewire-pulse, pactl, pw-metadata, and Python Gio.
The fixture uses private Weston/Niri, D-Bus, audio, and XDG paths. Shell and Notify
binaries must share a directory. It starts a graphical Notify daemon, two null
audio sinks, a remapped source, and the bundled default Shell bar. It clears any
inherited `ICED_BACKEND` override.

```sh
cargo build --release --locked -j2 -p canopa-shell -p canopa-notify \
  --bin canopa-shell --bin canopa-notify
python3 scripts/measure-idle.py --binary target/release/canopa-shell \
  --label candidate --output /tmp/canopa-idle-candidate
python3 scripts/measure-idle.py --binary target/release/canopa-shell \
  --label candidate-after-banners --churn-count 10 \
  --output /tmp/canopa-idle-candidate-after-banners
```

Use separate new output directories for each run. Save baseline binaries before
building the candidate; use the same config, machine, and fixture arguments.
Alternate baseline and candidate runs, with no compilation or other measurement
fixture running concurrently. Defaults are a 60-second settling period and
60-second observation with one sample per second. `--churn-count` renders and
closes each real banner stack before settling; these notifications are transient.

Each run retains binary/config hashes, platform information, fixture logs, and
per-second samples in `metadata.json` and `result.json`. CPU is process CPU time
as a percentage of one logical core, using `/proc` clock ticks. RSS is resident
memory; PSS apportions shared pages, so RSS values should not be summed as unique
memory usage. Private memory is `Private_Clean + Private_Dirty` from smaps_rollup.
Observed voluntary context switches count scheduler sleeps, including I/O waits;
they are not CPU time. The switch count is a lower bound when workers retire
between samples. Thread identities include start time to avoid reusing counts.

## Scope and limits

The recorded comparison uses three alternating startup baseline/candidate pairs,
then one pair after ten banner cycles. Each run settles for 20 seconds and samples
for 60 seconds; the reproduction commands above retain the longer default settling
period. The host is an Intel Core i5-6200U with four logical CPUs and about 7.6 GiB
of RAM. Validation and compilation finish before these measurements begin.

The native fixture deliberately uses WGPU with Mesa software GL
(`WGPU_BACKEND=gl`, `LIBGL_ALWAYS_SOFTWARE=1`). Hardware GPU drivers, real audio
devices, loaded panels, session services, and notification history can have
different footprints. A reclamation probe returned about 33 MiB of free heap
after Notify's startup probe surface closed on this machine; that is evidence
for the chosen optimization, not a universal saving. `malloc_trim` is GNU/Linux
specific and can only return eligible unused pages.

The graphical renderer and surface transparency remain part of acceptance.
Shell keeps its persistent GPU-backed bar and Notify keeps its renderer connection
available. This pass does not establish a minimum possible footprint. Further
renderer or clipboard lifetime changes require separate measurements and native
behavior checks.

## Validation

The audio sleeping regression failed with the original 50 ms polling loop and
passed with event-driven waiting. The private Pulse transport suite covers volume
changes, sink removal/reappearance, server recovery, and prompt cancellation.
The banner grace-period regression failed before reclamation was added. Surface
tests cover cancellation when content returns, duplicate closes, stale delayed
requests, first-frame readiness, and output fallback.

Both `bash scripts/check.sh` and `bash scripts/check.sh --core` passed. The final
release binaries passed the selected native acceptance cases: quick-settings-dark,
desktop-dark, desktop-light (scale 1.25), fork-lifetimes, and content-rotation
(scales 1.0 and 1.25). This covers real audio controls, banner and Center actions,
native accessibility and keyboard focus, appearance reload, independent restarts,
source/body image pixels, and renderer lifecycle/rotation behavior.

Two stale fixture assumptions were corrected: shared appearance reload changes
both consumers and requires reopening the Center, and the rotation fixture must
force its GTK client onto Wayland. The first appearance attempt also exposed a
filtered configuration log target; enabling that target made both appearance
cases pass. These outcomes and rerun details are preserved in the validation data.

## Recorded startup results

Across the three startup pairs, use the median of each run's measurements:

| Metric | Baseline | Candidate |
| --- | ---: | ---: |
| Shell CPU, percent of one core | 0.271% | 0.223% |
| Shell observed voluntary context switches/s | 42.86 | 23.05 |
| Shell RSS | 170.0 MiB | 170.3 MiB |
| Shell PSS | 112.7 MiB | 112.8 MiB |
| Notify RSS | 134.6 MiB | 105.1 MiB |
| Notify PSS | 77.8 MiB | 48.2 MiB |
| Notify private memory | 58.7 MiB | 29.1 MiB |

Notify returned about 29.5 MiB of resident memory (22% of its baseline RSS).
Shell's scheduler sleeps fell by about 19.8/s (46%). Its median CPU time fell by
about 18% relative, or 0.048 percentage points of one core; absolute idle CPU was
already low. The baseline CPU range was 0.271–0.302%, versus 0.207–0.223% for the
candidate. `/proc` has 100 CPU clock ticks per second on this host, so one tick
over a roughly 63-second window represents about 0.016 percentage points.

No Notify CPU ticks were recorded in these settled observation windows in either
build. The one-shot startup reclamation occurs during settling and is outside
that CPU measurement. Shell memory did not materially improve in this pass.

## After ten banner cycles

One additional pair rendered and closed ten separate banner stacks, settled for
20 seconds, then observed idle usage for 60 seconds:

| Metric | Baseline | Candidate |
| --- | ---: | ---: |
| Shell CPU, percent of one core | 0.285% | 0.207% |
| Shell observed voluntary context switches/s | 43.0 | 23.1 |
| Notify RSS | 172.6 MiB | 118.3 MiB |
| Notify PSS | 115.2 MiB | 60.7 MiB |

Notify's post-activity RSS was about 54 MiB lower (31%). Both builds recorded zero
Notify CPU ticks during this settled window. This is one workload pair, so its
size should not be generalized to all banner content or drivers.

The first banner-cycle attempt was excluded before sampling: the fixture treated
the already-bound control socket as readiness, but Notify binds it before startup
and D-Bus export finish. The fixture now probes `GetServerInformation` until the
API is ready before sending notifications. It never retries a timed-out `Notify`
mutation. Startup measurements did not send notifications and were unaffected.

## Evidence

[Run summaries, hashes, and excluded attempts](performance/data/2026-10-04/runs.json)
include eight successful release runs, hardware/profile information, and source
fingerprints. Each summary links to a gzip-compressed complete trace containing
the metadata and per-second samples. The original startup sampler source is also
retained compressed; the per-run sampler hashes distinguish the readiness fix.
[Validation outcomes](performance/data/2026-10-04/validation.json) preserve the
standard check commands, test totals, native results, and the appearance rerun.

The measured binaries have these SHA-256 fingerprints:

| Binary | SHA-256 |
| --- | --- |
| Baseline Shell | `8dcefbf49fa0bd3d39fa464e90a05cbd1fae1955f161c082a255c2ad1312c406` |
| Candidate Shell | `f961257594d1c0ef6ab52e3a1382aec1ba8fcf6505a949d600a86f7a638fee8d` |
| Baseline Notify | `6ab2af0b57dc120656e140507e346a8de2a7ea7e6357c0fa1bf1ae98c11fe43d` |
| Candidate Notify | `c87a0a64c9c5fc1a9668bc2dc0165a8cce856f5646ef9d1357ecf1755c0ae4d3` |
