# 0011: Name the desktop shell Canopa Shell

Status: Accepted, 2026-09-30.

Niribar now supplies Canopa's bars, panels, drawers, menus and Notification Center
host. Its product name is **Canopa Shell**, making its role within the desktop
explicit. This supersedes the retained shell names and documentation path in
[ADR 0009](0009-consolidated-repository-layout.md), without changing ownership,
application lifetimes or licensing.

The package and main executable are `canopa-shell`, with Rust imports under
`canopa_shell`. Source lives in `crates/canopa-shell/`. The shipped Wi-Fi helper is
`canopa-wifi-provider`. The companion target starts `canopa-shell.service` and
`canopa-notify.service`; both remain independently restartable. Tools, validators,
release bundles and current documentation use these names together.

The shell's layer namespace and control socket become `canopa-shell` and
`$XDG_RUNTIME_DIR/canopa-shell.sock`. The dismissal surface uses
`canopa-shell-dismissal`. Tracing targets use `canopa_shell`; performance reporting
uses `CANOPA_SHELL_PERF`. The provider backend override uses
`CANOPA_WIFI_PROVIDER_BACKEND`. Both environment variables accept their old
`NIRIBAR_*` counterparts when the new variable is absent.

The shared `canopa/config.kdl` and its `shell` namespace remain authoritative.
Discovery still accepts `niribar/config.kdl` when the shared file is absent;
there is no new component-specific configuration directory. Existing configuration
is never moved or rewritten automatically. Explicit executable references in user
configuration, compositor startup and key bindings need the new names.

Renaming a product does not revise its protocols. `niribar-v1`, its framed magic,
and the existing IPC message encoding retain their literal values. Provider
implementations keep working; shell clients must use the new executable/socket.
The built-in provider resolver locates the renamed helper beside the shell before
consulting PATH. No second executable or service alias is installed.

Shell documentation and its ADR/archive history move to `docs/shell/`, retaining
decision identifiers and historical terminology. Links are adjusted to the moved
files. `.scratch/niribar/`, original Git branches, recorded evidence, the excluded
rotation prototype, vendor provenance, and original copyright notices retain
their historical identities.

Installation remains explicit. The updater requires the renamed service; the
[migration guide](../shell-rename.md) covers the one-time switch before normal
updates resume. A build or test never renames or restarts installed services.
