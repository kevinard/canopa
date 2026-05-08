Status: ready-for-agent

# Make Bar and Module Identity Strict

## What to build

Enforce the accepted identity model for **Bar Names**, **Module Names**, and **Module Kinds**. Names are exact and case-sensitive; Bar and Module names live in separate namespaces; built-in Module names are reserved; placement names resolve predictably or fail as configuration errors.

## Acceptance criteria

- [ ] Duplicate Bar Names are configuration errors.
- [ ] Duplicate Module Names are configuration errors.
- [ ] Module Name matching is exact and case-sensitive.
- [ ] Built-in Module Kind default names are reserved and cannot be redefined by user-declared Modules.
- [ ] A placement name resolves first to a declared Module Name, then to a synthesizable built-in Module Kind.
- [ ] Unknown module references are configuration errors, not silently skipped.
- [ ] Built-in Modules with meaningful safe defaults can still be referenced without explicit declaration.
- [ ] Modules requiring user-provided commands or labels must be explicitly configured.

## Blocked by

- .scratch/domain-alignment/issues/01-migrate-bar-zones-to-start-center-end.md
