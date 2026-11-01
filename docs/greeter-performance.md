# Greeter compositor measurements

Canopa's `session --compositor cage|weston` launcher supports either compositor.
This comparison measures the same release greeter and packaged configuration
inside a private, fixed-size Wayland desktop. See [login setup](greeter.md) for
deployment and selection.

## Results and default choice

Measured on 2026-11-01 using Cage 0.3.1 and Weston 15.0.1, with Canopa's normal
release profile (thin LTO, one codegen unit, stripped symbols). The host was an
Intel Core i5-6500 with four logical CPUs and approximately 15.6 GiB RAM.

| Metric | Cage | Weston |
| --- | ---: | ---: |
| Startup median to first frame callback | 125.0 ms | 96.3 ms |
| Startup range, five runs | 92.3–129.6 ms | 73.2–104.2 ms |
| Idle CPU, percent of one logical core | 0.033% | 0.027% |
| Combined RSS median | 52.4 MiB | 77.6 MiB |
| Range of per-run RSS medians | 48.3–52.5 MiB | 69.6–77.9 MiB |

**Cage remains the packaged default; Weston is explicitly selectable.** Cage's
median combined RSS was 25.2 MiB (32.5%) lower. Weston started about 29 ms faster
by the median, with overlapping startup ranges. Memory usage is the clearest
separation in this fixture and supports retaining Cage for the lightweight default.

Each compositor accumulated 300 seconds of idle observation. Cage recorded ten
CPU ticks and Weston eight, at 100 ticks/second. One aggregate tick represents
about 0.0033 percentage points of one core. The two-tick difference does not
establish a meaningful general CPU advantage. Neither result is a zero-CPU claim.

This choice uses the controlled software-rendering evidence below. Deployment
still defaults to the compositor's automatic renderer selection; these results
do not establish a hardware/DRM ranking.

[Results](performance/data/2026-11-01-greeter/summary.json),
[metadata](performance/data/2026-11-01-greeter/metadata.json),
[complete samples](performance/data/2026-11-01-greeter/trace.json.gz) and
[exact measurement sampler](performance/data/2026-11-01-greeter/sampler.py.gz)
retain the measured conditions and SHA-256 fingerprints. The sampler snapshot
includes a harmless Pillow deprecation warning corrected in the maintained script
after this run; pixel checks and measurement calculations are unchanged.
These measurements predate automatic desktop-session and AccountsService user discovery. The exact
[measured greeter configuration](performance/data/2026-11-01-greeter/greeter.kdl)
is retained; current builds/default configuration include discovery startup work
and can show a different installed-session catalog. The recorded results refer
to the archived binary/configuration hashes.

## Reproduction and methodology

Finish builds and acceptance checks before measuring. Use the normal root target
directory, an optimized release greeter, installed Cage/Weston and the native
desktop fixture's dependencies (including Python Pillow). No host display-manager
or PAM changes are involved.

```sh
cargo build --release --locked -j2 -p canopa-greeter --bin canopa-greeter
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

Before this comparison, both launch modes passed the complete 22-check native
greeter matrix, including password/OTP, pointer/keyboard, cancellation, retries,
failure handling and compositor exit after authenticated handoff. Weston testing
exposed a buffer-lifetime compatibility issue; the measured frontend uses a slot
pool that destroys active buffers only after release. An early pilot with a black
Weston kiosk was excluded and is not a performance result.

[Validation outcomes](performance/data/2026-11-01-greeter/validation.json) record
the native matrices, focused tests and tooling checks. The prior unrelated
workspace test failures remain recorded in the local greeter work log.
