# Selectable greeter compositor and comparative measurement

Status: implemented; both native matrices and comparative measurements passed

Expose Cage and Weston as explicit choices in Canopa's greetd launcher. Preserve
the direct Wayland frontend and authentication boundary. Validate full interaction
and compositor termination/handoff under both. Benchmark the same release greeter
and fixed configuration under both compositors, alternating runs with identical
software rendering/output dimensions. Record startup-to-first-frame-callback,
whole compositor/greeter CPU time and RSS/PSS/private memory. Retain raw samples,
versions, binary/config hashes, environment and uncertainty. Use those results to
choose a packaged default while retaining explicit override. Do not alter host
display-manager or PAM configuration.

## Implementation

- `canopa-greeter session --compositor cage|weston` explicitly selects the kiosk.
  The launcher execs it, passes configuration/arguments literally, and retains the
  direct Wayland frontend. Optional executable/argument overrides and renderer
  auto/software selection support deployments and controlled nested measurement.
- Packaged greetd examples cover both; Cage remains the measured default.
- Weston acceptance exposed a black frontend with immediately destroyed raw
  buffers. The frontend now uses SCTK's SlotPool and defers destruction until
  release. Weston 15 also returns zero for a failed child; acceptance checks the
  explicit frontend failure and absence of handoff instead of inferring success
  from that compositor exit status. Authentication authorization remains greetd's.
- A private readiness socket records the first frame callback without tracing or
  authentication data. The sampler independently rejects empty/black pixels,
  validates stable process-tree identities, and saves all samples and fingerprints.

## Measurement

Five alternating pairs, one excluded warm-up per compositor, fixed release
frontend/configuration, 1280×800, Pixman, five-second settling and 60-second idle
observation per session. Parent Weston/Niri, sampler, D-Bus and fake greetd are
excluded. No compilation or other fixture ran during the final comparison.

| Metric | Cage 0.3.1 | Weston 15.0.1 |
| --- | ---: | ---: |
| Median startup to first frame callback | 125.0 ms | 96.3 ms |
| Startup range | 92.3–129.6 ms | 73.2–104.2 ms |
| Idle CPU, one core | 0.033% | 0.027% |
| Median combined RSS | 52.4 MiB | 77.6 MiB |
| Range of per-run RSS medians | 48.3–52.5 MiB | 69.6–77.9 MiB |

Cage recorded ten CPU ticks and Weston eight over 300 seconds each. One tick is
approximately 0.0033 percentage points of one core; the difference is too small
to establish a general CPU advantage. Cage's 25.2 MiB lower median RSS (32.5%) is
the clearest separation; its median startup was 29 ms slower, with overlapping
ranges. This supports retaining Cage as the packaged default and leaving Weston
explicitly selectable.

The full report and reproducible commands are in `docs/greeter-performance.md`.
`docs/performance/data/2026-11-01-greeter/` retains summary, metadata, compressed
complete samples and the exact sampler snapshot. `benchmark/` contains original
logs, screenshots and per-run JSON. Release greeter SHA-256:
`b4d07a3299b97e76596d46834db7d678c5f55e1231f331724061e3212d21c6ef`.

An early black-Weston pilot was excluded; `pilot-fixed/` verified the corrected
sampler before the full comparison. The exact comparison sampler emits a harmless
Pillow deprecation warning. The maintained script uses the supported API with a
compatibility fallback and saves its own source; no measurement calculation
changed. The final short sampler smoke run is separate from the reported data.

## Validation

- Greeter focused tests: 8 passed; the real-greetd transport test remains ignored
  in ordinary Cargo runs (already accepted under its private runner in the prior
  greeter work). `focused-final.log`.
- Full native interaction/failure/handoff matrices: 22 passed under Weston and
  22 under Cage via the actual launcher. `weston-accepted/`, `cage-accepted/`.
- Strict `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- Desktop runner tests: 8 passed. Check-selection tests: 8 passed. Unified CLI
  forwarding/dry-run selection passed for both compositor choices.
- Documentation staging: 38 files and 56 resolving offline links. New Python
  sources compile; packaging/release shell syntax, Rust formatting and diff
  whitespace checks passed.

No shared Rust crate, dependency fork, authentication protocol or other application
implementation changed in this follow-up. The full workspace test gate was not
repeated; its prior Files/host AT-SPI failures remain recorded in
`.scratch/greeter/PRD.md` and are not claimed resolved.

These are nested software-rendering measurements, not DRM/seat/GPU/cold-boot or
installed PAM acceptance. RSS sums can double-count shared pages; greeter memory
hardening prevents unprivileged PSS sampling. Automatic compositor rendering
remains the production default and needs target-machine measurements for a
hardware ranking. The host login manager, configuration and PAM were not modified.
