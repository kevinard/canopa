Status: ready-for-human

# Add local file and folder name search

## Parent

[Unified Shell Launcher](../PRD.md).

## What to build

Select and integrate a file/folder name backend behind the common provider
contract. Use the bounded native memory cache selected in ADR 0023, with
explicit root/exclusion settings and eventually refreshed coverage.

## Acceptance criteria

- [ ] Record bounded-scan behavior, create/rename/delete refresh, cancellation,
  failure handling, and the native backend ownership decision.
- [ ] The accepted adapter meets an explicit name-matching contract and measured
  budgets; any substring/fuzzy retrieval tradeoff is resolved and documented.
- [ ] Configured roots/exclusions control search coverage. Hidden directories,
  build trees, overlapping roots, symlinks, permissions, and mounts have explicit policy.
- [ ] Queries never walk all roots on the UI thread or on every keystroke. Updates
  respect query generation and bounded result/output/work budgets.
- [ ] Names, paths, category, and availability are distinct. Index updating,
  unavailable backend, truncated results, and no matches have appropriate states.
- [ ] Files and folders open through a typed session-owned boundary; Copy path
  preserves text paths verbatim and labels escaped non-UTF-8 representations.
  Search results never implicitly execute a found file.
- [ ] Temporary-fixture tests cover create/rename/delete, exclusion changes, stale
  activation, unusual and non-UTF-8 paths, non-ASCII names, and backend restart.
- [ ] Native acceptance verifies real file/folder opening and clipboard behavior;
  backend installation, scope settings, and service ownership are documented.

## Dependencies

[01](01-launcher-session.md), [02](02-launcher-panel.md), backend assessment above.

## Implementation review — 2026-11-03

Implemented within the existing Panel Menu source path; see
[ADR 0023](../../../docs/shell/adr/0023-unified-launcher-providers.md), the
[launcher guide](../../../docs/shell/launcher.md), and
[validation evidence](../VALIDATION.md). Original proposal criteria remain as
historical scope; persistent extension sessions, dedicated CLI/action entry points,
and new placement were replaced by the documented implementation decisions.
Further native acceptance matrices are not implied by the focused checks.
