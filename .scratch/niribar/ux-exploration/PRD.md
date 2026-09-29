# Shared niribar panel design and motion

Status: implemented and validated (2026-09-29), working-tree change against `55bb76c`.

Approved by the user after comparing the interactive proposals and requesting
consistency between drawers and popovers. The two are supported presentations of
one product, not alternative visual designs.

## Acceptance

- Both presentations share titles and close controls, typography, spacing, search,
  menu rows, hover/selection/focus treatments and semantic palette roles.
- Apply the treatment to module details, overview, selectable menus and provider
  content. Preserve provider control identities, accessibility, focus and scrolling.
- Presentation differences are placement, sizing policy and outer frame geometry.
  Keep existing explicit dimensions, caps, edge fill and drawer overlay/push-bar
  behavior. Account for common chrome in content-preferred sizes.
- Add bounded directional slide/fade opening and closing. Use the existing per-bar
  `panel.animation` settings (180 ms open and 160 ms close by default). Zero durations
  disable motion, providing a reduced-motion configuration.
- Keep Foreground responsible for window identity and presentation lifetime. Close
  logical interaction and provider sessions promptly; exit content is inert. A stale
  completion cannot remove new content. Replacing content must remain responsive.
- Request frames only during transitions, without resizing surfaces or exclusive
  zones each frame. Bound temporary capture allocations. Support all four edges and
  scaled outputs. Keep menu virtualization and fixed scroll pitch coherent.

## Validation

Lifecycle regression tests cover disabled motion, early close, interrupted motion,
rapid reopen, stale completion, origin removal and immediate interaction cleanup.
Use the private Weston/Niri desktop harness for placement, input and scrolling.
Run the normal workspace checks, `bash scripts/check.sh`.

`menu-panel-directions.html` is design evidence, with sample content. Its compact
Applications popover and synchronized cross-preview state do not change production
sizing or session lifetime contracts. Browser timings do not establish native
performance; verify native rendering.

## Results

- Native `cargo build --locked -p niribar`: passed with the production renderer.
- Niribar unit/integration tests: passed, including 1,198 binary tests.
- Full `bash scripts/check.sh`: passed (formatting, strict workspace Clippy,
  shared theme contrast, workspace tests, configuration, portal and vendor checks).
- `verify-panel-motion.py`: all seven cases passed at 1x and 1.5x. Both vertical
  presentations, top/bottom popovers, disabled motion, intermediate entry/exit
  frames, rapid reopen and prompt dmenu cancellation were exercised. Each sampled
  idle interval recorded 0.0% CPU for the fixture process.
- `verify-drawer-modes.py`: left/right overlay and push-bar placement, transparent
  space click-through, bar toggles and Escape passed at 1x and 1.5x.
- `verify-menu-scrolling.py`: native icons, idle keyboard redraw, 180-pixel travel
  from 60 pixels of touchpad input, and a 3,600-pixel virtualized-row round trip passed.
- Independent standards/spec review findings were resolved; see `REVIEW.md`.

Native artifacts are under `/tmp/canopa-panel-motion-final-1x`,
`/tmp/canopa-panel-motion-final-1_5x`, `/tmp/canopa-panel-drawer-1x` and
`/tmp/canopa-panel-drawer-1_5x` and `/tmp/canopa-panel-scroll-1x`. Full validation log:
`/tmp/canopa-panel-ux-full.log`.

Motion frame sampling warms a private software-rendered Weston/Niri session and
uses one-second transitions to observe intermediate frames. Default timings are
exercised by the drawer and scrolling fixtures; the samples are not an FPS claim.
The live desktop and installed binary were not restarted or replaced.

## Follow-up: provider popover height

The user reported that Network and Notifications popovers were too short to use.
Both used the generic provider fallback of 180 pixels plus the 56-pixel header,
regardless of how much scrolling content they contained. The motion fixtures used
explicit dimensions, so they did not catch this default-sizing usability problem.

Provider popovers now prefer the existing 600-pixel fallback height. Side popovers
cap that preference to the known vertical bar span; horizontal bar thickness is
not mistaken for output height. Explicit heights and maximum heights still apply.
The viewport remains stable across provider content replacements.

Added sizing regressions for both providers on all edges, small side outputs,
height overrides/caps and unknown surface dimensions. A private desktop fixture
with sample provider content verified 600-pixel popovers at 1x and fitting the
available height at 1.5x; screenshots/results are in
`/tmp/canopa-provider-height-1x` and `/tmp/canopa-provider-height-1_5x`.

Follow-up validation passed: native build, all 12 sizing tests, formatting, strict
workspace Clippy, and the complete workspace test suite (including 1,200 Niribar
binary tests). Configuration, settings portal and vendor checks also passed. The
first full check encountered an unrelated notification legacy-import subprocess
timeout; that exact test passed in isolation, then the complete workspace suite
passed with serialized tests. Workspace rerun log:
`/tmp/canopa-provider-height-workspace-recheck.log`.

## Follow-up: softer panel motion

The user found the movement/fading unnatural, especially on close. The original
quadratic accelerating exit retained 19% opacity at 90% elapsed time, then cut to
zero at full speed. Use smoothstep on exit to taper movement and opacity before
removal, shorten shared travel from 12 to 8 logical pixels, and increase the
default closing duration from 120 to 160 ms. Opening retains its 180 ms cubic
ease-out. Both presentations use the same motion; configured durations and zero
motion remain supported.

Extend the native fixture to record the entire exit and optionally sample default
timings. Regression coverage checks monotonic fading and a nearly transparent
tail, including closing before entry finishes. Software compositor screenshots
are too slow to assert every frame at normal speed; slowed transitions establish
the native sequence and deterministic curve tests cover the tail.

Native validation passed all seven slowed/disabled cases at 1x, the drawer at
1.5x, and a default-timing popover run. The last sampled nonzero frame in the
slowed popover exit dropped from 23% of the open frame's pixel difference in the
baseline to 3% after tuning; this is visual evidence, not an FPS claim. All three
motion widget tests passed, including inverse hit testing on every edge. The
interactive HTML preview now uses the same travel, durations and easing curves.
Artifacts: `/tmp/canopa-motion-before-curve`, `/tmp/canopa-motion-soft-1x`,
`/tmp/canopa-motion-soft-1_5x`, and `/tmp/canopa-motion-soft-default`.

Workspace validation completed: formatting, strict Clippy, shared integration
tests, portal, configuration and vendor checks passed. The full check initially
exposed a KDL test fixture with a missing separator: its expected closing value
had silently come from the old default. Corrected the fixture to exercise explicit
duration overrides and asserted the new defaults. All Niribar targets then passed,
including 1,201 binary tests. Logs: `/tmp/canopa-motion-soft-check.log` and
`/tmp/canopa-motion-soft-niribar-recheck.log`; remaining check logs share the
`/tmp/canopa-motion-soft-` prefix. The production binary was rebuilt; the live
desktop was not restarted.
