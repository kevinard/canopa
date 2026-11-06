Status: ready-for-human

# Define launcher sessions and the provider contract

## Parent

[Unified Shell Launcher](../PRD.md).

## What to build

Define the shell-owned Launcher Session and the common provider/result/action
model. Include the asynchronous coordinator and application provider without
changing configured Panel Menu or dmenu semantics. Record the accepted ownership
decision before production integration.

## Acceptance criteria

- [ ] Session, query generation, provider generation, result identity, and displayed
  revision are distinct; activation refers to the displayed result and action.
- [ ] New queries cancel superseded work and reject stale responses even when
  cancellation is ineffective. Closing or replacing a session retires all work.
- [ ] Same-query updates preserve an explicitly selected result by stable identity;
  removal chooses a deterministic neighbor. New queries start new selection.
- [ ] Provider ordering and result budgets follow the PRD without comparing
  arbitrary provider scores. Completion, truncation, and failure states are explicit.
- [ ] A slow or failed provider cannot block another provider or queue unbounded work.
- [ ] Existing application metadata and matching adapt to the shared contract.
- [ ] Focused tests cover out-of-order batches, selection changes, provider restart,
  stale pointer/Enter activation, replacement, and close during a query.

## Dependencies

None. UI integration follows in [02](02-launcher-panel.md).

## Implementation review — 2026-11-03

Implemented within the existing Panel Menu source path; see
[ADR 0023](../../../docs/shell/adr/0023-unified-launcher-providers.md), the
[launcher guide](../../../docs/shell/launcher.md), and
[validation evidence](../VALIDATION.md). Original proposal criteria remain as
historical scope; persistent extension sessions, dedicated CLI/action entry points,
and new placement were replaced by the documented implementation decisions.
Further native acceptance matrices are not implied by the focused checks.
