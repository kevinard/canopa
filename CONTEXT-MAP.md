# Canopa contexts

Canopa is a desktop environment based on Niri. Each context below owns its
domain language, state and lifetime; shared crates share code, not ownership.

| Context | Owns | Glossary | Architecture | Decisions |
| --- | --- | --- | --- | --- |
| Shell (`canopa-shell`, providers, `canopa-launcher`) | Bar, launcher, taskbar, Quick settings, panels, on-screen display; hosts the Notification Center through generic panel content | [shell/CONTEXT.md](docs/shell/CONTEXT.md) | [shell](docs/architecture/shell.md) | [shell/adr](docs/shell/adr/) |
| Notify (`canopa-notify`, `canopa-notify-client`) | Notification receipt, policy, persistence, sound, banners, Do Not Disturb, the Notification portal | [notify/CONTEXT.md](docs/notify/CONTEXT.md) | [notify](docs/architecture/notify.md) | [notify/adr](docs/notify/adr/) |
| Files (`canopa-files`, `canopa-file`) | Local file browsing, operations, Undo, recovery, previews, file-manager preferences | [files/CONTEXT.md](docs/files/CONTEXT.md) | [files](docs/architecture/files.md) | [adr](docs/adr/README.md) |
| Portals (`canopa-portal`) | Portal dialogs and sessions; the forked frontend owns permissions and routing | — | [portals](docs/architecture/portals.md), [desktop sharing](docs/architecture/desktop-sharing.md), [printing](docs/architecture/printing.md) | [adr](docs/adr/README.md) |
| Session (`canopa-session`, `canopa-session-ui`, `canopa-greeter`, `canopa-polkit`, `canopa-pinentry`) | Wallpaper surfaces, lock presentation and authentication input, login presentation, authorization and PIN dialogs, idle coordination | — | [session](docs/architecture/session.md) | [adr](docs/adr/README.md) |
| Settings (`canopa-settings`) | Presenting preferences; each existing owner keeps its policy and lifetime | — | [settings](docs/architecture/settings.md) | [adr](docs/adr/README.md) |
| Shared foundations (`canopa-config`, `canopa-theme`, `canopa-ui`, `canopa-assets`, `canopa-apps`, `canopa-i18n`, `canopa-services`, `canopa-widgets`, `canopa-cli`) | Configuration discovery and reload, appearance, shared controls, icons and MIME, translations, system-service clients, passive widget data | — | [foundations](docs/architecture/foundations.md) | [adr](docs/adr/README.md) |
| Panel protocol (`canopa-panel-protocol`) | The Shell ↔ provider protocol | — | [panel protocol](docs/architecture/panel-protocol.md), [provider reference](docs/shell/external-panel-providers.md) | [adr](docs/adr/README.md) |

External owners keep their authority: Niri owns windows, outputs and lock
security; greetd owns login authentication and session lifetime; PAM owns
authentication policy; native polkit owns action policy and credentials; GnuPG
owns keys and caching; NetworkManager, BlueZ, UPower, logind and the audio server
own their state.

Use each context's terminology for its domain. Shared panel types contain no
notification policy, and source sharing does not merge application lifetimes or
stores. The built-in notification module consumes only Notify's bounded summary
through `canopa-notify-client`; the Notification Center stays a declarative
provider.

Start with the [architecture overview](docs/architecture/overview.md) and the
[documentation index](docs/README.md).
