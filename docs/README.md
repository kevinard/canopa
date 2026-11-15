# Canopa documentation

## Using Canopa

| Guide | Covers |
| --- | --- |
| [Installation](installation.md) | Arch packages and the Canopa session, the release archive, updating, upgrading from Niribar, recovery |
| [Configuration](configuration.md) | `config.kdl` reference for every component, `canopa config`, reloading |
| [Themes](theme-customization.md) | Appearance settings, built-in themes and custom theme files |
| [Shell](shell.md) | The bar and its modules, Quick settings, taskbar, panels, sound and media, keys |
| [Launcher](launcher.md) | Applications, search providers, actions and extensions |
| [Notifications](notifications.md) | Banners, Notification Center, Do Not Disturb, history, `canopa-notify ctl` |
| [Files](files.md) | The file manager |
| [Settings](settings.md) | The Settings application and its pages |
| [Wallpaper, lock and idle](session-tools.md) | Wallpaper picker, lockscreen, idle and sleep |
| [Login screen](greeter.md) | Canopa Greeter with greetd |
| [Polkit](polkit.md) and [pinentry](pinentry.md) | Authorization and passphrase dialogs |
| [Desktop portals](portals.md) | File chooser, screenshots, app chooser and the other dialogs apps request |
| [Desktop sharing](desktop-sharing.md) | Screen casting, remote desktop, clipboard, input capture, global shortcuts, USB |
| [Printing](printing.md) | The experimental Print portal |

## Developing Canopa

| Guide | Covers |
| --- | --- |
| [Contributing](contributing.md) | Setup, workflow, issues, documentation and decision rules |
| [Building](building.md) | Build profiles and the free-space guard |
| [Testing](testing.md) | Check gates, test layout, native desktop harnesses |
| [Continuous integration](ci.md) | Workflows and the self-hosted desktop runner |
| [Releasing and packaging](releasing.md) | Release archive, Arch packages, tagging |
| [Translations](translations.md) | Fluent strings, adding languages |
| [Fork maintenance](vendor-maintenance.md) and [fork inventory](fork-inventory.md) | Patched upstream dependencies and their upstream status |
| [Dependency attributions](dependency-attributions.md) | Generated license inventory (do not edit) |

### Architecture

- [Overview](architecture/overview.md): components, processes and how they talk
- [Foundations](architecture/foundations.md): configuration, appearance, shared
  controls, assets, i18n, accessibility, service clients
- [Panel protocol](architecture/panel-protocol.md) and the
  [provider reference](shell/external-panel-providers.md)
- [Shell](architecture/shell.md), [Notify](architecture/notify.md),
  [Files](architecture/files.md), [Settings](architecture/settings.md),
  [session components](architecture/session.md)
- [Portals](architecture/portals.md), [desktop sharing](architecture/desktop-sharing.md),
  [printing](architecture/printing.md)

### Domain language and decisions

- Glossaries: [Shell](shell/CONTEXT.md), [Notify](notify/CONTEXT.md),
  [Files](files/CONTEXT.md); the [context map](../CONTEXT-MAP.md)
- [Decision records](adr/README.md), plus the [Shell](shell/adr/) and
  [Notify](notify/adr/) histories
- Notify reference: [maintenance](notify/maintaining.md),
  [preferences control API](notify/settings-control-api.md),
  [freedesktop compliance](notify/freedesktop-notification-compliance.md)
