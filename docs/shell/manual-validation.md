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
6. On a laptop running the installed Canopa session, pair Bluetooth headphones,
   disconnect and reconnect them, and confirm audio routing. Repeat pairing
   with a passkey device and close the panel during confirmation. Adjust
   brightness, then verify a hardware brightness key is reflected when it
   changes the same backlight. Switch among supported power profiles and
   verify the active profile after reopening Quick settings.
7. Save a keyboard shortcut and verify that the new combination runs its
   action while the old combination is released. Enable larger text and
   reduced motion, dock and undock, then suspend and resume into the lock
   screen. Check readability and keyboard navigation at both output scales.

## Focused-window rotation regression

Build Canopa Shell with `cargo build -p canopa-shell --bin canopa-shell`, then run:

```sh
python3 scripts/verify-content-rotation.py \
  --binary target/debug/canopa-shell \
  --output /tmp/canopa-shell-content-rotation \
  --scale 1.0 1.25 1.5 2.0
```

This disposable Weston/Niri session requires Python Pillow, Python GI with GTK 3,
and `dbus-daemon`. It checks real focused-window text and application icons on
left/right bars against an unrotated top-bar control. Each scale renders two
different titles, including accented characters, with two frames per title.
The check requires grayscale glyph coverage and visible icons, and waits for
title changes to appear on every bar. Without `--scale`, it checks 1 and 1.25.
Screenshots and `result.json` remain in the output directory. This checks the
software WGPU backend; it does not establish all font or hardware GPU behavior.

On 2026-09-27, the original rotated outline rendering produced only foreground
and background colors. Enabling renderer antialiasing increased this to five
coverage levels, but letters remained uneven. Rasterizing with Iced's normal
font engine at the physical output scale, then rotating pixels without filtering,
passed all 48 samples above. Side-bar text had 236–250 coverage levels across the
two titles and four scales. The per-widget raster is refreshed for content,
style, scale, and size changes; placement is snapped to physical pixels. Unit
tests also check exact glyph-pixel preservation through both quarter turns and
cropping of constrained titles.

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
  --binary target/release/canopa-shell \
  --label 'COMMIT release; RUST_VERSION' \
  --output /tmp/canopa-shell-measurements
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
