# Implementation review

Review baseline: `18cd5bc702ea651dc5f529945fb2392b6cc89481`.
Initial implementation: `40e58b6`.

## Standards

The original diagnostic-offset finding is fixed: combined validation now maps
main-file offsets back from its captured theme reference, while theme-relative
offsets remain unchanged. A direct-versus-captured regression covers this. A private
snapshot directory and short relative reference also avoid increasing input size.
The standards reviewer rechecked the correction and found no further issue there.
The final command-line checks also caught an opt-in syntax-source diagnostic
regression. Malformed main bytes now reach the component validators unchanged;
safe diagnostics remain content-free. The existing configuration integration check
passes, and a focused standards recheck found no additional issue.

The reviewer also suggested a typed enum for the five state slots as a possible
maintainability improvement. The fixed-size slots and documented composition order
remain; this is not a functional defect or a required architectural change.

## Spec

Review corrections preserve local panel/menu precedence, distinguish real pointer
hover from keyboard focus, and reserve configured borders in frames and sizing.
Follow-up checks corrected menu-row width, close-button border height and explicit
Notification Center top padding. Local menu gap and main-file diagnostic offsets
have regressions; notification layout tests exercise larger typography and borders.

No material scope creep or outstanding functional findings remained after recheck.

## Validation

Completed 2026-10-03. Every stage of the normal `scripts/check.sh` entry point passed,
resuming the relevant stages after corrections rather than repeating unaffected
checks. This included workspace formatting and strict linting, dependency/vendor
checks, preset contrast, updater and companion-unit checks, all **1,845 workspace
tests across 62 suites**, Settings portal integration, combined configuration checks,
and the vendor regression tests. The final CLI diagnostic correction additionally
passed CLI lint, all three CLI tests, and the configuration integration script.

Seven maintained desktop scenarios passed using the final application binaries:

- `theme-customization`: all four edges and three alignments at 1.25 scale,
  percentage/fixed lengths, window reservation independent of alignment, atomic
  theme-only reload, invalid-candidate retention, recovery, transparent host and
  rounded-corner click-through, and a painted panel beyond the short bar.
- `configuration-reload`: existing live-reload regression coverage.
- `drawer-modes`: both side edges and both modes at 1.25 scale, unchanged client
  geometry, input restoration, outside dismissal and empty-desktop cases.
- `desktop-dark` and `desktop-light`: Shell/Notify smoke tests, with fractional
  scaling in the light scenario.
- `menu-scrolling` and `menu-customization`: 200 entries, prepared icons,
  keyboard redraw, exact 180-pixel touchpad travel for steady and bursty input,
  and a 3,600-pixel scroll round trip. The custom variant uses larger monospace
  fonts, padding, margins, gaps, borders and radii.

Native artifacts are in `/tmp/canopa-theme-acceptance-final` (reload),
`/tmp/canopa-theme-acceptance-recheck` (drawers),
`/tmp/canopa-theme-acceptance-rendering` (dark/light), and
`/tmp/canopa-theme-acceptance-metrics` (final theme and menu scenarios).
Earlier failed fixture attempts in those directories are superseded by the named
passing runs: output dimensions now come from the compositor, GTK uses Wayland,
and pointer focus is settled before pixel comparisons. No product assertion was
weakened. Panel and custom-menu screenshots were inspected visually.
