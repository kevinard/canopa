Status: ready-for-human

# Add executable launcher extensions

## Parent

[Unified Shell Launcher](../PRD.md).

## What to build

Expose the common provider model through a versioned JSONL stdin/stdout protocol
for explicitly configured executables. Ship a small bookmark or project-shortcut
example and documentation sufficient to implement an extension in another language.
Keep this distinct from the existing whole-panel provider protocols.

## Acceptance criteria

- [ ] Configuration validates names, literal argv, unique keywords, and participation
  in unscoped queries; only routed queries are delivered to a provider.
- [ ] Each request/response declares protocol version 1. One process receives
  one query and returns one result batch; the host executes typed selected actions.
- [ ] Session/query generations, displayed revision, and provider/result identity
  prevent stale responses and stale activation after process replacement.
- [ ] Host rendering supports metadata and named actions without arbitrary widgets.
  Validated copy/open actions use host boundaries; domain actions route to the owner.
- [ ] Limits cover frame bytes, result/action counts, field sizes, pending requests,
  startup/query/activation deadlines, stderr capture, and process-tree teardown.
- [ ] A crash, oversized/invalid frame, incompatible version, ignored cancellation,
  or hung activation affects only that provider and leaves the launcher responsive.
- [ ] No actions run during queries. Long-lived application launches survive panel
  close through the documented session-owned handoff.
- [ ] The example proves default/additional actions, cancellation, and error handling
  through the public contract without host special cases.
- [ ] Protocol fixtures, host regressions, and private native checks exercise rapid
  typing, process failure, configuration reload, and launcher dismissal.

## Dependencies

[01](01-launcher-session.md), [02](02-launcher-panel.md). Define the wire contract
early so files and calculator results exercise the same concepts before release.

## Implementation review — 2026-11-03

Implemented within the existing Panel Menu source path; see
[ADR 0023](../../../docs/shell/adr/0023-unified-launcher-providers.md), the
[launcher guide](../../../docs/shell/launcher.md), and
[validation evidence](../VALIDATION.md). Original proposal criteria remain as
historical scope; persistent extension sessions, dedicated CLI/action entry points,
and new placement were replaced by the documented implementation decisions.
Further native acceptance matrices are not implied by the focused checks.
