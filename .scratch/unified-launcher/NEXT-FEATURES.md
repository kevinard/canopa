# Launcher follow-ups

Status: ready-for-human

The requested next features are implemented in the existing Applications Panel
Menu. Native windows are matched by title/application/workspace and focused by
acknowledged Niri identity. Closed-window/context changes invalidate displayed
actions. Search `windows` or `settings` to scope native results; an existing
extension using either keyword retains precedence.

Native results merge by match quality, recent/frequent identity, then stable
source order. Private history keeps at most 256 identities/counts/order values;
queries and copied text are excluded. One worker serializes loading, writes and
clear. `history #false` opts out. Clear history waits for durable completion.

File rows add reveal through standard FileManager1, an in-panel Open with chooser
using GIO MIME recommendations/file-capable applications, and Files-owned Quick
Look. Original Unix path bytes survive every handoff. Quick Look waits for the
source browser's exact selection and geometry; unavailable paths never preview a
neighbor. Navigation/tab changes retire pending intent. Existing Files preview
ownership and dismissal apply. Settings supports explicit initial pages without
writing preferences until Apply.

Calculator grammar adds postfix percentages, pi/e, and a closed set of built-in
math functions, including function-valued conversion amounts. Qalculate owns
numeric evaluation; trigonometry uses radians.

Decisions: Shell ADR 0023, root ADR 0037, Files ADR 0158. The executable extension
protocol and launcher placement/foreground owners remain as previously defined.

Validation so far:
- Shell unit suite: 1,298 passed.
- Files named activation/presentation/previews: 21 passed, three private helper
  tests intentionally ignored by the harness and exercised by their parent tests.
- Files source-owned preview admission controller regression: passed.
- Focused Shell/Files/Settings Clippy with warnings denied: passed.
- Private Weston/Niri native acceptance at scale 1.25: passed, including Open with,
  reveal, existing-window focus, Quick Look owner/focus return, percentages/functions,
  private history restart/clear and existing audio panel.
- Final scale 1 repeat including Settings page handoff: passed. The fixture waits
  for panel input readiness and verifies the complete query before activating;
  earlier loaded-run failures captured partial query input, not history failure.
  A separate loaded run exceeded Files startup readiness; its deadline is now
  bounded at 45 seconds. Final verification ran after workspace checks.
- Offline documentation: 55 files, 80 resolving links.
- Workspace gate: completed successfully, including formatting, workspace/all-target
  Clippy with warnings denied, all-target unit/integration tests, Settings portal,
  supported-session binaries, configuration tests and vendor checks. The gate first
  caught the new unregistered native script and stale dependency-inventory hash;
  both were updated, then the remaining gate commands completed successfully.
  The retained continuation reports 193 successful test groups / 3,033 reported
  passes (including private helper and vendor test results), zero failures.

Generated evidence: `.scratch/artifacts/unified-launcher/next-features-native-125/`,
`next-features-native-final-1/`, and `next-documentation/`. Earlier failed native
runs remain alongside these with failure screenshots/history diagnostics. Tooling
logs are in `next-checks/` and `workspace-continued.log` under the same artifact
root. Offline documentation and whitespace checks pass.
