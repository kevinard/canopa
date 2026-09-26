# Canopa contexts

Canopa is a desktop environment based on Niri. Niribar is its shell; Canopa Notify
owns notification receipt, policy, persistence, sound, and banners. The shell hosts
the Notification Center through generic declarative panel content.

- [Shell language and ownership](niribar/CONTEXT.md)
- [Notification language and ownership](notify/CONTEXT.md)
- [Configuration source and reload contract](docs/adr/0002-shared-configuration.md)
- [Configuration user guide](docs/configuration.md)
- [Workspace decision](docs/adr/0001-shared-workspace-and-upstream-forks.md)
- [Integration proposal and remaining phases](docs/canopa-integration-proposal.md)

Use each context's terminology for its domain. Shared panel types contain no
notification policy. Source sharing does not merge application lifetimes or stores.
