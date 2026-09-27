# Consolidate the Canopa repository layout

Status: done

User-approved scope, 2026-09-27: finish folding Niribar and Notify into the existing
Canopa workspace as application packages under `crates/`.

- Move applications to `crates/niribar/` and `crates/canopa-notify/`, keeping source,
  tests, embedded assets, examples, migrations and original licenses with them.
- Consolidate repository validation, installation and release tooling in root
  `scripts/` and distribution files in root `packaging/`.
- Consolidate documentation under root `docs/`, preserving separate shell and
  notification domain histories. Preserve historical ticket identifiers.
- Update agent instructions, CI, fixtures and path references to use one repository
  root, one Cargo workspace, one lockfile and root build outputs.
- Preserve package/executable names, application ownership, feature profiles,
  runtime paths and independent process restart behavior. Keep the rotation
  prototype excluded and its experimental dependencies intact.
- Validate core and combined workspace checks, desktop tool paths and release
  assembly. Review changes against this scope and repository standards.

Implementation decision: `docs/adr/0009-consolidated-repository-layout.md`.
Validation and review: `docs/repository-layout-validation.md`.
