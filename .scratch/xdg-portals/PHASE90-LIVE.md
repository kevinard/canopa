# Phase90: Explicit portal configuration generation

Goal ACTIVE; Print deferred and opt-in. The previous goal turn was concrete
progress: phase89 folder memory was completed and shipped. Its gates are DONE.
This turn addressed a delivery gap rather than repeating them: users previously
had to merge complete portal configurations manually to select Canopa safely.

New scripts/portal-config.py ships as bin/canopa-portal-config. It reads an
explicit --base and generates a complete review file, defaulting to Settings and
FileChooser. Repeated --interface chooses exactly the named set. Descriptors
determine supported interfaces and backend IDs, including canopa-notify for
Notification. Extended optional interfaces need the extended descriptor; Print
is not advertised or selected by default. --list shows descriptor capabilities,
not an assertion about the running session.

Selected keys retain configured explicit/default fallback order. Unselected
routes, default, comments and unrelated sections survive unchanged. Repeated
groups/keys use effective last values; CRLF/no-final-newline is preserved.
Unpreservable syntax and unsupported interfaces fail before writing. --output
creates a new0600file and refuses existing files/symlinks; otherwise stdout.
No installation or service/compositor/routing change. The helper is Python stdlib
like packaging tools; backend/UI implementations remain Rust/Iced with no GTK.
Guides and ADR0115 describe the explicit base and optional-companion limits.

All current validation handles TERMINAL0:

- Generator unit/CLI11tests PASS, config-tests-01.log, handle75484. Includes real
  source metadata, Notify ID, fallback/untouched routing, comments, line endings,
  repeated groups, malformed input, exclusive output and relocated optional
  metadata. No test downloads or application builds.
- Check-script15tests PASS, check-script-tests-01.log, handle94930. The explicit
  workspace gate now includes the generator regressions. Python AST and bash -n
  on affected scripts PASS; no Cargo command/build was run for tooling changes.
- Real private frontend PASS, frontend-01.log, handle13172. The fixture uses the
  generator on CRLF/repeated groups/keys, then verifies actual backend activation,
  public Settings ReadOne/ReadAll and reload signals. No UI/capture opened.
- Host-namespace audit PASS, host-cleanup-01.json: no private frontend processes
  or directories remain. Source guard proves1357Rust/Cargo/vendor runtime inputs
  unchanged from accepted phase89 state. Backend stays dfdf49c5.
- Single existing archive refreshed, package96.56s/full smoke10.92s, handle30568.
  Smoke executes the relocated helper with shipped descriptors and independently
  checks Notify routing plus preserved ScreenCast/Print selections.41docs/94links.
- Streaming verification PASS, handle66596: all8apps+2companions and both policy
  files match phase89. The executable7309byte helper matches source exactly; both
  updated guides ship. No retained extraction. All pinned source inputs unchanged.

Combined: target/portal-selection-90/result-01.json.
Archive: target/portal-release-package-check/canopa-x86_64-unknown-linux-gnu.tar.gz
Bytes209600475; SHA256bf292b64da1776a92e0a3d72c7c944e9e0d1416d92969577f7058f83b97e2e31.
Helper SHA256d804334b93af7a435cfcaf9760aa408d97f286adb1e797a8f66484a96ebae359.
Delivery temporary parent removed; root remains above3GiB free. No fresh target,
SDK/runtime, cache cleanup, companion build or executable snapshots.

This cohort is DONE. Latest full workspace test gate remains phase86, full Portal
library verification phase89. Do not relabel them as fresh or repeat completed
matrices/package merely on continuation. Continue concrete missing non-Print
capability/client integration from the original map, after checking current
source. Keep optional touch/physical-USB/client-distribution limits honest without
turning them into automatic new matrices. No external blocker or missing user
answer. Full goal ACTIVE; live session selection unchanged.
