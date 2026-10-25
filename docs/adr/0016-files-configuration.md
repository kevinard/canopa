# Files configuration joins the shared envelope

Status: accepted under delegated implementation authority, 2026-10-16.

Canopa Files owns the `files` namespace in the existing shared configuration.
`canopa-config` admits a unique argument-free block and provides the same bounded
read, source fingerprint, safe diagnostics and independent namespace selection
as for Shell and Notify. Files supplies its own schema and headless validator.
It does not introduce a second entry-point configuration or a legacy fallback.

`canopa config check` invokes the Files validator against the same immutable
snapshot used by the other owners. Installed releases must place `canopa-files`
beside the other component executables. The graphical Files action editor may
update its namespace while retaining unrelated settings, subject to stale-source
checks. That write is a source edit; each consumer retains its own installation
and reload lifetime.

The namespace contains explicit custom actions and interface preferences. See
[Files ADR 0009](../files/adr/0009-custom-action-editor.md) for its editor,
execution and acceptance boundaries. Folder view overrides remain application
state as recorded in Files ADR 0002. Global view, sidebar, toolbar, bookmark and shortcut preferences extend this
owner schema as recorded in [Files ADR 0011](../files/adr/0011-interface-preferences.md).

Files now prepares its settings and shared appearance as one immutable candidate
and reloads saved source/theme changes automatically. Source selection, last-good
state, editor drafts and installation races follow
[Files ADR 0019](../files/adr/0019-live-configuration-reload.md). This adds no shared
service or cross-process installation transaction.
