# Local issue tracker

Track new specs and tickets as Markdown files under root `.scratch/`.
Use one feature directory, `.scratch/<feature>/`, with `PRD.md` and
`issues/<NN>-<slug>.md` as needed. Record triage as a `Status:` line using
[the shared labels](triage-labels.md); append discussion under `## Comments`.

Historical application tickets live under `.scratch/niribar/` and
`.scratch/notify/`. Preserve their identifiers and reference their paths explicitly;
do not duplicate or renumber them. Repository consolidation is scoped in
`.scratch/repository-consolidation/PRD.md` and recorded in
[ADR 0009](../adr/0009-consolidated-repository-layout.md).

Integration decisions live in root `docs/adr/`; application ADR histories live in
`docs/shell/adr/` and `docs/notify/adr/`. Built-in providers share v5 with explicit
v1/v4 compatibility. Settings is implemented; FileChooser and interactive request
lifetimes are tracked in `.scratch/xdg-portals/` and root ADR0026.
