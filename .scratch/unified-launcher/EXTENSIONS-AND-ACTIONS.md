# Bundled extensions, shortcuts and action search

Status: implemented and verified

User requested the proposed web/URL extension, date/time tools, aliases/pins,
Settings extension management, and searchable actions, all using existing panels.

Implemented:
- Bundled Web and Date/time providers are executable modes of canopa-shell using
  the existing bounded JSONL provider path. Web constructs typed HTTP(S) actions;
  matching performs no network request. Explicit keywords/search templates and
  pasted URLs are supported. Date/time uses local calendar/Unix conversions and
  installed IANA zones, with GNU date formatting in the bounded provider group.
- Recognized bundled instant answers precede fuzzy application matches.
- Tab/Search actions opens a searchable child mode of the same Panel Menu.
  Existing search ranking puts exact action labels before fuzzy matches; Back and
  Escape restore parent queries. Stale targets retain session/generation/revision
  guards and closed windows retire their action mode.
- Pin/Unpin and Set/Edit alias support stable application/file/URL/control/command
  targets. Original Unix bytes survive persisted shortcuts. Exact/prefix matches
  retain precedence over weaker pinned matches. Pin and alias edits preserve each
  other, and clear selection history leaves deliberate shortcuts intact.
- canopa-launcher shares validated private preferences and read-only discovery,
  with locking, field-level edits, bounded intake and atomic durable writes.
- Existing Settings adds --page launcher, enable/keyword controls, search templates,
  observed capabilities and safe failure status. Saves preserve concurrent Shell
  shortcut edits and reject stale extension/search drafts. Shell polls off-thread
  and cancels routes affected by preferences. Executable argv stays in source KDL.

Decisions: root ADR 0038, following Shell ADR 0023 and root ADR 0037. No new launcher
placement or preview owner. System runtime dependencies add coreutils and tzdata.
New shared package retains both workspace license texts and packaged documentation.

Validation:
- Initial shared preference/Settings concurrency tests passed.
- Shell unit suite passed 1,305 tests before two additional ranking regressions.
- Native run found bundled answers below fuzzy apps; corrected priority and added
  a regression. A second run reached action search and found a fuzzy action ahead
  of exact Pin; switched to existing rank_matches and added a file-action regression.
- Fixture now limits application discovery to its own desktop entries and uses a
  local URL handler, avoiding unrelated application launches/network requests.
- Full workspace tooling prelude passed; Clippy findings were corrected. Broad
  test continuation was interrupted to rebuild the native-discovered action fix.
- Final focused checks/build passed: workspace/all-target Clippy with warnings
  denied, 36 launcher tests, three shared preference tests and the Settings
  concurrent-save regression. The full Shell suite previously passed 1,305;
  two more native-discovered ranking tests are included in the final workspace run.
- Complete native launcher acceptance passed at scale 1.25 after ranking fixes.
  A scale 1 repeat also passed actual Settings keyboard keyword edit/save,
  enable/disable saving, live Shell routing and concurrent pin/alias preservation.
  Both runs cover explicit URLs/web searches (fake browser handler), custom search
  keywords, date/time, action search, pins/aliases and safe status/capability discovery.
- Existing Settings native regression passed after adapting Tab navigation for
  the fifth page: input save, display confirmation/reversion and crash rollback,
  appearance/lockscreen saving, wallpaper picker and unavailable power backend.
  Optional Stasis-specific branches were not run. The script now uses native
  screenshots and a standard-library PNG fixture, removing its Pillow dependency.
- Final workspace continuation passed: 1,307 Shell tests, all workspace/all-target
  tests, Settings portal activation/reload checks, supported-session executable
  builds, configuration checks and vendor/native-pipe checks. Workspace/all-target
  Clippy passed with warnings denied. The tooling prelude passed in the original
  --workspace invocation; subsequent native-discovered fixes were covered by the
  final Clippy, focused/native tests and full remaining gate.
- Desktop acceptance manifest/runner tests passed (eight tests). Final documentation
  and whitespace checks passed. Earlier failures/interrupted logs remain as evidence.
- Offline documentation: 58 files, 81 resolving links.

Evidence is under `.scratch/artifacts/unified-launcher/`, with extensions-*.log,
extensions-native-* directories (failed runs retained), and extensions-documentation.
