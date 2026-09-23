# Desktop acceptance and measurement record

Automated socket/D-Bus fixtures do not establish hardware compatibility or
whole-desktop accessibility. Record actual results here or in a dated follow-up;
do not convert unexecuted steps into passing evidence.

## Current evidence boundary

The 2026-09-23 remediation is covered by headless unit, transport and process
tests. The following desktop checks have **not been executed for that change**.
No CPU/RSS/startup comparison or whole-application accessibility certification
is claimed.

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

Nix packaging remains unprovided and explicitly unsupported in the README.
The optional Nix CI path should become required only with an implemented,
validated flake and an agreed support commitment.
