# 0133 Display profiles

Status: accepted.

Settings owns named layouts in a separate KDL file. It resolves the entire
connected set, including disabled outputs, with one unique output per selector.
Saved rules prefer make, model and serial when that identity is unique. Connector
constraints disambiguate missing or duplicate serials. The highest priority
compatible profile is eligible for hotplug. Ties require manual selection.
The model takes inspiration from Kanshi's named profiles and output selectors.
[Kanshi configuration reference](https://man.archlinux.org/man/kanshi.5.en)

A dedicated Canopa session unit observes Wayland output management batches and
reads the actual topology from Niri IPC after a configurable quiet period. Only
changes to connected identities trigger dock and undock decisions. Profile edits
invalidate stale offers and are read before the next decision. The suggested
layout opens in Settings for review. The notification action and trial helper
both check current inputs before acting. If a topology change leaves every
connected screen disabled, the watcher enables a screen before offering the
layout and prefers the built-in panel when one is available.

Manual and automatic switches use the same leased display transaction. It
enables and checks a replacement display before turning off the old one, and
verifies the requested layout before keeping it. Manual switches use the
existing confirmation window. Automatic switches keep the verified trial only
while the selected profile and policy still agree. Profile trials apply to the
live compositor. The regular display editor remains responsible for persisting
the default Niri include.
