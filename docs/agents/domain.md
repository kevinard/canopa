# Domain documentation

Read root `CONTEXT-MAP.md` before exploring the codebase. It maps the two application
contexts and shared infrastructure:

- Shell language and decisions: `docs/niribar/CONTEXT.md` and `docs/niribar/adr/`.
- Notification language and decisions: `docs/notify/CONTEXT.md` and `docs/notify/adr/`.
- Integration decisions: `docs/adr/`.

Use each context's glossary terms when describing its concepts. Shared crates do
not transfer notification policy or persistence into the shell. Surface conflicts
with accepted ADRs explicitly instead of silently overriding them.

Application instructions live in `crates/niribar/AGENTS.md` and
`crates/canopa-notify/AGENTS.md`. Repository commands run from the workspace root.
