# Domain Docs

How the engineering skills should consume this repo's domain documentation when exploring the codebase.

## Layout

This is a single-context repo.

- Read `CONTEXT.md` at the repo root for domain language.
- Read `docs/adr/` for architectural decisions that touch the area being changed.

## Use The Glossary

When output names a domain concept, use the term as defined in `CONTEXT.md`. Do not drift to synonyms the glossary explicitly avoids.

## Flag ADR Conflicts

If output contradicts an existing ADR, surface it explicitly rather than silently overriding.

