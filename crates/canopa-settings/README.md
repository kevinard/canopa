# Canopa Settings

`canopa-settings` is the Canopa Settings application: displays and display profiles,
keyboard and pointer, network, Bluetooth, printers, sound, personalization,
accessibility, power and idle, date and time, region and language, users, privacy,
system information, notifications, default applications, launcher, shortcuts and
widget layouts. System services and Canopa components own and apply each setting;
privileged changes ask polkit. Run `canopa-settings [PAGE]` or `canopa settings`; a
running window switches to the page. The same binary initializes the Canopa
compositor configuration at login (`canopa-settings init-session`).

- [Settings guide](../../docs/settings.md)
- [Settings architecture](../../docs/architecture/settings.md): page registry, page
  patterns and how to add a page.

Core adapter tests are in the `integration` target:
`bash scripts/cargo.sh test --locked -p canopa-settings --test integration`.
Native Niri parser tests are ignored by default; run them with
`CANOPA_NIRI_BIN=/absolute/path/to/niri bash scripts/cargo.sh test --locked -p canopa-settings --test integration -- --ignored`.
