Status: ready-for-human

# Add calculator results and copy activation

## Parent

[Unified Shell Launcher](../PRD.md).

## What to build

Add arithmetic as a provider using the same result and activation model. Use Qalculate's CLI and document the dependency, supported numeric grammar,
resource bounds, locale handling, and isolation from personal calculator state.

## Acceptance criteria

- [ ] Arithmetic works automatically for recognizable expressions and through the
  explicit `=` scope, with precedence, parentheses, decimals, and unary signs.
- [ ] Tests include `12 * (8 + 3) = 132`, `-2 * 3 = -6`, decimal calculations,
  incomplete expressions, division by zero, and excessive expression complexity.
- [ ] Explicit calculator scope shows concise errors; ordinary text searches do
  not show calculator errors or lose application/file results.
- [ ] Decimal input and display/copy formatting have documented locale behavior;
  results do not acquire accidental ANSI formatting or diagnostic text.
- [ ] Queries cannot invoke calculator commands, persist definitions/history, or
  fetch data. The evaluator has bounded time and output and rejects stale results.
- [ ] Activating the displayed result copies its plain value. Clipboard failure
  remains visible and keeps the launcher open; successful-copy close behavior is
  verified with actual Wayland clipboard ownership after dismissal.
- [ ] Missing dependencies disable this provider clearly while other sources work;
  installation and the supported expression grammar are documented.

## Dependencies

[01](01-launcher-session.md), [02](02-launcher-panel.md).

## Implementation review — 2026-11-03

Implemented within the existing Panel Menu source path; see
[ADR 0023](../../../docs/shell/adr/0023-unified-launcher-providers.md), the
[launcher guide](../../../docs/shell/launcher.md), and
[validation evidence](../VALIDATION.md). Original proposal criteria remain as
historical scope; persistent extension sessions, dedicated CLI/action entry points,
and new placement were replaced by the documented implementation decisions.
Further native acceptance matrices are not implied by the focused checks.

## Unit conversions — 2026-11-03

At the user's request, the same provider now supports numerical amounts and
compatible source/target units, using `to`, `in`, or arrow separators. Unit symbols
and names resolve through the host catalog; Qalculate owns conversion factors and
absolute temperature offsets. Results validate the requested target unit and copy
both value and unit. Built-in units are loaded only for conversion expressions;
currencies, functions, variables, datasets, and personal state remain disabled.
See [unit conversion validation](../UNIT-CONVERSIONS.md).
