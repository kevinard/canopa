# Canopa contexts

Canopa is a desktop environment based on Niri. Canopa Shell is its shell; Canopa Notify
owns notification receipt, policy, persistence, sound, and banners. The shell hosts
the Notification Center through generic declarative panel content.

- [MPRIS media controls](docs/shell/adr/0018-mpris-media-controls.md)
- [Shell language and ownership](docs/shell/CONTEXT.md)
- [Notification language and ownership](docs/notify/CONTEXT.md)
- [Configuration source and reload contract](docs/adr/0002-shared-configuration.md)
- [Shared appearance and UI ownership](docs/adr/0003-shared-appearance.md)
- [Local asset mechanisms and caller policies](docs/adr/0005-shared-assets.md)
- [Shared provider contracts and host state](docs/adr/0006-shared-provider-contracts.md)
- [Unified built-in provider protocol](docs/adr/0007-unified-panel-protocol.md)
- [Rich item sliders](docs/adr/0012-rich-panel-sliders.md)
- [Rich item source attribution](docs/adr/0010-rich-item-source-attribution.md)
- [Settings portal and shared-only configuration](docs/adr/0008-settings-portal.md)
- [Configuration user guide](docs/configuration.md)
- [Workspace decision](docs/adr/0001-shared-workspace-and-upstream-forks.md)
- [Repository layout and tooling ownership](docs/adr/0009-consolidated-repository-layout.md)
- [Canopa Shell naming and compatibility](docs/adr/0011-canopa-shell-name.md)
- [Integration proposal and remaining phases](docs/canopa-integration-proposal.md)

Use each context's terminology for its domain. Shared panel types contain no
notification policy. Source sharing does not merge application lifetimes or stores.

The built-in notification module consumes only Notify’s bounded summary through
`canopa-notify-client`; Notify owns attention meaning and history. The Center stays
a declarative provider. See [ADR 0004](docs/adr/0004-integrated-notifications.md).
