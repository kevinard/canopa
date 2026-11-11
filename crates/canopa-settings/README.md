# Canopa Settings

An ordinary Iced window for display/input configuration, shared appearance and
lockscreen preferences, the existing wallpaper picker and Stasis idle controls.
Run `canopa-settings` or `canopa settings`. See [the settings guide](../../docs/settings.md).

Display trials run in an independent process and require confirmation before
persistence. A disconnected frontend, timeout or failed partial apply restores
previous live output settings. Niri, Wallpaper, Lock and Stasis retain their own
policy and lifetimes. No settings daemon runs after the window closes.

The supported Arch installation uses a separate Canopa session and compatible
Niri build; see [installation](../../docs/installation.md). Existing ordinary Niri
configuration is never rewritten. Core adapter tests are in the `integration`
target; select them with
`bash scripts/cargo.sh test --locked -p canopa-settings --test integration`.
The IPC peer tests use a separate validator stub. Three native parser tests are
explicitly ignored in portable checks and run by desktop CI; select them with
`CANOPA_NIRI_BIN=/absolute/path/to/niri bash scripts/cargo.sh test --locked -p canopa-settings --test integration -- --ignored`.
