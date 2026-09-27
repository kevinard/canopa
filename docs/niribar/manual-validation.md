# Desktop acceptance and measurement record

Automated socket/D-Bus fixtures do not establish hardware compatibility or
whole-desktop accessibility. Record actual results here or in a dated follow-up;
do not convert unexecuted steps into passing evidence.

## Current evidence boundary

The 2026-09-23 remediation is covered by unit, transport and process tests.
An isolated compositor harness now covers real bar mapping and repeated panel
open/close; see the dated follow-up report for executed measurements. The wider
hardware matrix below still requires operator acceptance. Whole-application
accessibility certification is not claimed.

See [executed results](audits/2026-09-23-desktop-validation.md) and the
[accessibility assessment](ACCESSIBILITY.md) for the current evidence.

## Desktop matrix

Record distribution, compositor version, Rust/build revision, GPU/driver,
output geometry/scales, network daemon version, and audio daemon version.

1. Start with missing D-Bus/audio/Niri services, then restore each. Confirm
   unavailable presentation, recovery, and no duplicated subscriptions.
2. Restart a tray application; change its status/icon/tooltip repeatedly and
   update an open menu twice. Confirm immediate updates and no ghost items.
3. Exercise NetworkManager and standalone iwd separately: scan, saved/open
   joins, correct/wrong personal credentials, hidden personal joins, permission
   denial, multiple adapters, and closing during activation. Reopen to verify
   successful credentials were saved. Never switch the machine's active network
   without the operator's agreement.
4. Remove/reconnect outputs with mixed scaling; check popup placement and focus,
   rotated modules, keyboard traversal, Escape, and launcher discovery during
   panel replacement. Launchers must respect user Hidden desktop overrides.
5. With a screen reader and AT-SPI inspector, inspect Canopa roles/names/state,
   focus changes, long text, live content replacement, action invocation, and
   keyboard-only use. Record broader bar/menu coverage separately.

## Performance procedure

Use the same release profile, configuration, compositor, output arrangement and
desktop workload for baseline and candidate. Record at least five runs.

- Measure idle CPU and RSS after a 60-second settling period, then sample for
  60 seconds (`pidstat -p PID -u -r 1 60`, if installed).
- Record wall time from launch to the first visible, populated bar with an
  external observer/compositor trace. Process lifetime is not startup latency.
- Repeat with a large desktop-entry directory, busy tray updates and open
  external panels. Record sample count, median/range, and the raw logs.
- Include the baseline commit and candidate commit in the report. Investigate
  regressions before assigning a performance acceptance result.

### Repeatable isolated compositor check

Linux prerequisites: Python 3, Niri 26.04 or compatible, Weston with a headless
backend, Mesa software rendering, `wtype`, and `dbus-daemon`. Build release binaries first,
then run from the repository root:

```sh
python3 scripts/validate-desktop.py \
  --binary target/release/niribar \
  --label 'COMMIT release; RUST_VERSION' \
  --output /tmp/niribar-measurements
```

The output directory must be new. Defaults are five runs, each settling for
60 seconds and sampling for 60 seconds. The script starts private Weston/Niri
and session D-Bus instances, uses the repository's default configuration,
disables Xwayland and Weston's idle timeout, and points system D-Bus and
PulseAudio to unavailable private paths. It does not use the running desktop's sockets or network daemon.
Child services and their process groups are retired on exit.

Results include binary/config hashes, compositor and kernel versions, output
geometry, raw `/proc/PID/stat` samples, per-run CPU/RSS measurements, compositor
layer registration time, and bar/panel screenshots. CPU percent is relative to
one core; RSS is the process resident set, not total graphics memory. Each run
also checks two panel open/close cycles through real IPC and compositor layer
queries, sends Tab with a virtual keyboard, and verifies Escape closes the first
panel. Every panel cycle waits one second after registration before input;
this verifies interaction after settling, not immediate keyboard readiness.
Registration timing includes compositor polling and is **not** a
measurement of the first visible populated frame. Screenshots need visual review.

Use the same compiler, profile, configuration and hardware for both binaries;
avoid builds or other changing loads while measuring. Software rendering and
unavailable services are a repeatable diagnostic workload, not a substitute for
the hardware matrix, busy-tray/launcher workloads, or screen-reader acceptance.
`--config` can select another reviewed configuration; commands in it will run.
For a quick harness smoke check, set `--runs 1 --settle-seconds 1 --sample-seconds 2`.

Nix packaging is deferred at the owner's request (2026-09-23).
