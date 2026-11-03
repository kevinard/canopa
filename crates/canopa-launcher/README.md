# Canopa launcher contracts

Launcher-owned preference and discovery types shared by Shell and Settings.
Shell owns searching, actions, extension processes and panel lifetime. Settings
edits preference fields using the same bounded, private, locked atomic store.
Pins and aliases retain original target identity and Unix path bytes. Read-only
extension discovery contains capabilities and safe status messages, never queries.

See [the launcher guide](../../docs/shell/launcher.md) and
[ADR 0038](../../docs/adr/0038-launcher-preferences-and-bundled-extensions.md).
