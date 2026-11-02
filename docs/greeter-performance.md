# Greeter compositor measurements

Canopa's `session --compositor cage|weston` launcher supports either compositor.
This comparison measures the same release greeter and packaged configuration
inside a private, fixed-size Wayland desktop. See [login setup](greeter.md) for
deployment and selection.

## Results and default choice

Measured on 2026-11-02 using Cage 0.3.1 and Weston 15.0.1, with Canopa's normal
release profile (thin LTO, one codegen unit, stripped symbols). The host was an
Intel Core i5-6500 with four logical CPUs and approximately 15.6 GiB RAM.

| Metric | Cage | Weston |
| --- | ---: | ---: |
| Startup median to first frame callback | 100.5 ms | 85.5 ms |
| Startup range, five runs | 92.2–131.5 ms | 82.7–112.8 ms |
| Idle CPU, percent of one logical core | 0.213% | 0.243% |
| Combined RSS median | 45.1 MiB | 71.0 MiB |
| Range of per-run RSS medians | 45.0–45.3 MiB | 70.4–74.6 MiB |

**Cage remains the packaged default; Weston is explicitly selectable.** Cage's
median combined RSS was 25.9 MiB (36.4%) lower. Weston started about 15 ms faster
by the median, with overlapping startup ranges. Memory usage is the clearest
separation in this fixture and supports retaining Cage for the lightweight default.

Each compositor accumulated 300 seconds of idle observation. Cage recorded 64
CPU ticks and Weston 73, at 100 ticks/second. One aggregate tick represents
about 0.0033 percentage points of one core. Both aggregate CPU results were below
0.25% of one core; this single-host comparison does not establish a general CPU
ranking. Neither result is a zero-CPU claim.

This choice uses the controlled software-rendering evidence below. Deployment
still defaults to the compositor's automatic renderer selection; these results
do not establish a hardware/DRM ranking.

[Results](performance/data/2026-11-02-greeter-followups/summary.json),
[metadata](performance/data/2026-11-02-greeter-followups/metadata.json),
[complete samples](performance/data/2026-11-02-greeter-followups/trace.json.gz) and
[exact measurement sampler](performance/data/2026-11-02-greeter-followups/sampler.py.gz)
retain the measured conditions and SHA-256 fingerprints. The imported
[desktop fixture](performance/data/2026-11-02-greeter-followups/desktop-fixture.py.gz)
and [greetd peer](performance/data/2026-11-02-greeter-followups/greetd-peer.py.gz)
are archived too. The updated greeter includes automatic session/user discovery,
live catalog/avatar sources, saved choices, shared shaping/input/accessibility
and policy-checked power controls. Its exact
[packaged configuration](performance/data/2026-11-02-greeter-followups/greeter.kdl)
is retained; the installed-session catalog depends on the host.

[Earlier measurements](performance/data/2026-11-01-greeter/summary.json) predate
these features and use a different binary/configuration. Keep them as historical
evidence rather than combining their samples with this comparison.

## Reproduction and methodology

Finish builds and acceptance checks before measuring. Use the normal root target
directory, an optimized release greeter, installed Cage/Weston and the native
desktop fixture's dependencies (including Python Pillow). No host display-manager
or PAM changes are involved.

```sh
cargo build --workspace --bins --locked --release -j2
python3 scripts/measure-greeter.py --cage /usr/bin/cage --weston /usr/bin/weston \
    --runs 5 --settle-seconds 5 --sample-seconds 60 \
    --output /tmp/canopa-greeter-comparison
```

The script starts a private Weston/Niri parent and a test-owned greetd peer. Both
candidate kiosks use Pixman, one fullscreen 1280×800 output, the same greeter
binary, fonts and configuration, and a private session bus. The packaged clock,
keyboard and battery configuration is retained; the fixture has no system bus or
physical battery. No custom widget commands or authentication attempts run during
measurement. The compositor's automatic renderer policy remains the deployment
default; explicit software rendering makes this comparison controlled.

One warm-up per compositor is excluded. Five measured pairs alternate order:
Cage/Weston, Weston/Cage, Cage/Weston, Weston/Cage, Cage/Weston. Each session starts
fresh, settles for five seconds, and samples for 60 seconds at one-second intervals.
No compilation or other acceptance/measurement fixtures run concurrently.

Startup is wall time immediately before spawning the product launcher until a
private Unix datagram reports the greeter's first surface frame callback. This
includes launcher configuration validation, compositor startup and frontend
initialization. It uses a warm page cache and is a compositor scheduling readiness
measurement, rather than physical scanout. A separate screenshot assertion rejects
black/empty kiosks before any idle sample is accepted. Protocol tracing is disabled.

CPU includes all processes under the candidate compositor, including the greeter
and all its threads. Process identities must remain stable throughout sampling.
The parent compositors, fixture D-Bus/greetd and sampler are excluded. Percentages
use one logical core, with aggregate CPU ticks divided by aggregate observation
time across the five runs. The clock redraws during the observation windows.

Memory is the sum of process RSS, reported as the median of each run's samples,
then the median across runs. Shared pages may be counted in both processes; this
is not unique resident memory. The greeter keeps its normal non-dumpable process
protection, so the unprivileged sampler cannot read its PSS or private-memory
accounting. That protection is retained during measurement.

## Scope

These are nested, software-rendered measurements on one machine. They do not
measure DRM/KMS startup, display modesetting, seat acquisition, physical GPU
drivers, real PAM/device latency or a cold boot/page cache. The same private
parent desktop is used for both candidates. Distribution versions, fonts, display
size and renderer policy can change the result. Startup ranges and raw samples
should accompany any interpretation of small differences.

Before this comparison, both release launch modes passed the complete 35-check
native greeter matrix, including AccountsService/NSS discovery, avatar overrides,
password/OTP, pointer/keyboard and protected AT-SPI input, cancellation, retries,
timeouts, failure handling and compositor exit after authenticated handoff.

[Validation outcomes](performance/data/2026-11-02-greeter-followups/validation.json)
record the native matrices, complete workspace gate, optimized build and the
broader session follow-up coverage. Initial test-peer isolation failures and their
resolution remain recorded; no application binary changed during this comparison.
