# Canopa display profiles

Status: implemented. Focused and desktop acceptance recorded below.

- Save named display drafts and switch them with the existing Keep or Revert flow.
- Offer a unique matching profile when the connected monitor set changes.
- Allow explicit automatic switching and configure monitor selectors, layouts,
  priorities, startup, and notification timing.
- Preserve the default Niri configuration and refuse ambiguous or stale matches.

The implementation decision is recorded in [ADR 0040](../../docs/adr/0040-display-profiles.md).

## Validation

- Focused Settings tests passed with 21 tests. Two additional native-parser
  tests passed against the pinned Niri build. The profile cases cover matching
  after a connector rename, ambiguous matches, precedence, stale edits, and
  rollback when automatic policy changes.
- The staged package and its systemd user-unit graph passed the system installer
  tests. Dependency attribution checks passed after updating the lockfile hash.
  The workspace gate passed through formatting and Clippy, then stopped on the
  stale attribution hash. After regeneration, the remaining gate steps passed,
  including all workspace tests, the Settings portal, configuration and vendor
  tests. Focused Clippy passed on the final Settings sources.
- An isolated Weston/Niri session verified live profile Keep and Revert and
  saving a named layout in Settings. A private Niri IPC peer simulated docking
  and undocking, including delayed notification startup, stale offers, review in
  Settings, recovery with all displays off and automatic switching. The default
  display include was identical after the profile switches. Selected evidence is
  in `.scratch/artifacts/display-profiles/`.
- The general Settings desktop fixture passed its display and input scenarios,
  including recovery after the Settings window was killed. It later timed out
  at the wallpaper picker step. Physical dock behavior remains unverified.
