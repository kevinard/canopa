# Decision records

A decision record captures one significant decision: a new ownership boundary,
protocol, persistence format, security rule, or an alternative that was rejected
and will be proposed again. The current design as a whole is described in
[`../architecture/`](../architecture/overview.md); records explain why.

## Where records live

- `docs/adr/` — new decisions for any area, including Files and the portals.
- `docs/shell/adr/` and `docs/notify/adr/` — the existing Shell and Notify
  histories. New decisions specific to one of those applications may continue
  there.

Earlier workspace and Files decision records (root `0001`–`0148`, Files
`0001`–`0180`) were consolidated into the architecture pages. The originals are in
the git history at commit `cb300936` (`docs/adr/`, `docs/files/adr/`). New root
records continue the numbering at `0149` so old references stay unambiguous.

## Template

```markdown
# NNNN. Title in the imperative or as a noun phrase

Status: accepted | superseded by NNNN

## Context

What forces the decision: the problem, the constraints, the owners involved.

## Decision

What we do, stated plainly.

## Consequences

What becomes easier or harder, what we gave up, and what code or docs follow
from it.
```

Keep a record under about 80 lines. Do not add progress status ("in progress",
"scoped verified"), test counts, run numbers, measurements or links to run
output. When a decision changes, write a new record and mark the old one
`superseded by NNNN`; then update the architecture page.
