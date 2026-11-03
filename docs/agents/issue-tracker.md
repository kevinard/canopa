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
v1/v4 compatibility. The Settings portal is implemented; interactive portal
request presentation remains separate work for the first interface needing it.

Keep human-authored plans, tickets and investigation notes as Markdown under
`.scratch/`. Other scratch files are ignored by default. Put new generated run
output under `.scratch/artifacts/<feature>/`; that directory is entirely ignored,
including generated Markdown documentation. Maintained test tools belong in
`scripts/`, and selected reproducible benchmark evidence belongs in
`docs/performance/data/`.

Before removing older run directories, preserve useful results, diagnostic logs
and screenshots in a local archive under `.scratch/artifacts/`, with a manifest
and a Markdown note recording its location. Preserve tracked files and handwritten
notes; remove regenerable caches, helper binaries and generated protocol bindings.
