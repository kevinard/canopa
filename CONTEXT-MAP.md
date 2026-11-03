# Canopa contexts

Canopa is a desktop environment based on Niri. Canopa Shell is its shell; Canopa Notify
owns notification receipt, policy, persistence, sound, and banners. The shell hosts
the Notification Center through generic declarative panel content. Canopa Files
owns local file browsing, operations, Undo, previews and file-manager preferences.
Canopa Wallpaper owns background surfaces and output image assignments; Canopa Lock
owns secure session locking, authentication and lock presentation. Canopa Greeter
owns login presentation while greetd owns login authentication and session lifetime.
Canopa Pinentry presents GnuPG PIN/passphrase and confirmation dialogs.
Canopa Polkit presents action authorization while native polkit owns policy and credentials. `canopa-widgets`
shares passive data without transferring shell actions or application lifetimes.
Canopa Settings presents desktop preferences while their existing owners retain
policy and lifetime. The supported system installation has a dedicated session.

- [Supported session and Settings ownership](docs/adr/0036-supported-session-and-settings.md)
- [Arch system installation](docs/installation.md)
- [Settings application](docs/settings.md)

- [Wallpaper, lockscreen and shared widget data](docs/adr/0026-session-tools-and-widget-data.md)
- [Logind, interactive PAM and lock composition](docs/adr/0027-logind-authentication-and-lock-composition.md)
- [Greetd greeter and shared session presentation](docs/adr/0028-greetd-greeter-and-shared-session-presentation.md)
- [Polkit agent and native authorization](docs/adr/0029-polkit-agent-and-native-authorization.md)
- [Pinentry protocol and shared dialogs](docs/adr/0030-pinentry-protocol-and-shared-dialogs.md)
- [PIN/passphrase dialog setup](docs/pinentry.md)
- [Polkit agent setup](docs/polkit.md)
- [Login screen setup](docs/greeter.md)
- [Session tools setup and configuration](docs/session-tools.md)
- [Stasis idle policy and sleep coordination](docs/idle.md)
- [Shared authentication input and accessibility](docs/adr/0032-shared-authentication-input-and-accessibility.md)
- [Authentication recovery and agent inquiries](docs/adr/0035-authentication-recovery-and-agent-inquiries.md)
- [Wallpaper state and picker](docs/adr/0034-wallpaper-state-picker-and-source-events.md)
- [Greeter preferences and live catalogs](docs/adr/0033-greeter-preferences-and-live-catalogs.md)
- [MPRIS media controls](docs/shell/adr/0018-mpris-media-controls.md)
- [Shell language and ownership](docs/shell/CONTEXT.md)
- [Notification language and ownership](docs/notify/CONTEXT.md)
- [File manager language and ownership](docs/files/CONTEXT.md)
- [File manager Iced and shared controls](docs/adr/0018-files-iced-and-shared-controls.md)
- [Iced transient toplevel windows](docs/adr/0019-iced-transient-toplevels.md)
- [Iced input transition boundaries](docs/adr/0020-iced-input-transition-boundaries.md)
- [Wayland ordinary-window focus, in progress](docs/adr/0021-wayland-window-focus.md)
- [File manager configuration ownership](docs/adr/0016-files-configuration.md)
- [File manager distribution and runtime](docs/adr/0017-files-distribution-and-runtime.md)
- [Ordinary Iced Wayland drag transport, in progress](docs/adr/0023-wayland-drag-transport.md)
- [Niri drag-source lifetime compatibility and pinned Arch package selection](docs/adr/0024-niri-drag-source-lifetime.md)
- [File manager resident host and activation](docs/adr/0022-files-resident-host-and-activation.md)
- [File manager standard desktop presentation, scoped core/native verification](docs/adr/0025-files-file-manager-interface.md)
- [File manager live configuration reload](docs/files/adr/0019-live-configuration-reload.md)
- [Configuration source and reload contract](docs/adr/0002-shared-configuration.md)
- [Shell live configuration reload](docs/shell/adr/0021-live-configuration-reload.md)
- [Launcher clipboard history and Settings](docs/adr/0039-launcher-clipboard-settings.md)
- [Launcher preferences, bundled extensions and action search](docs/adr/0038-launcher-preferences-and-bundled-extensions.md)
- [Launcher file actions and Settings pages](docs/adr/0037-launcher-file-actions-and-settings-pages.md)
- [Unified launcher and search providers](docs/shell/adr/0023-unified-launcher-providers.md)
- [Shared appearance and UI ownership](docs/adr/0003-shared-appearance.md)
- [Configurable themes and surface geometry](docs/adr/0013-configurable-theme-and-surface-geometry.md)
- [Theme configuration reference](docs/theme-customization.md)
- [Canopa visual direction and first shell slice](docs/adr/0014-canopa-visual-direction.md)
- [Local asset mechanisms and caller policies](docs/adr/0005-shared-assets.md)
- [Shared provider contracts and host state](docs/adr/0006-shared-provider-contracts.md)
- [Unified built-in provider protocol](docs/adr/0007-unified-panel-protocol.md)
- [Rich item sliders](docs/adr/0012-rich-panel-sliders.md)
- [Rich item source attribution](docs/adr/0010-rich-item-source-attribution.md)
- [Settings portal and shared-only configuration](docs/adr/0008-settings-portal.md)
- [Configuration user guide](docs/configuration.md)
- [Workspace decision](docs/adr/0001-shared-workspace-and-upstream-forks.md)
- [Runtime CPU and memory measurements](docs/performance.md)
- [Repository layout and tooling ownership](docs/adr/0009-consolidated-repository-layout.md)
- [Canopa Shell naming and compatibility](docs/adr/0011-canopa-shell-name.md)
- [Integration proposal and remaining phases](docs/canopa-integration-proposal.md)

Use each context's terminology for its domain. Shared panel types contain no
notification policy. Source sharing does not merge application lifetimes or stores.

The built-in notification module consumes only Notify’s bounded summary through
`canopa-notify-client`; Notify owns attention meaning and history. The Center stays
a declarative provider. See [ADR 0004](docs/adr/0004-integrated-notifications.md).
