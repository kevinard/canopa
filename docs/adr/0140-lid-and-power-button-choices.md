# 0140 Lid and power-button choices in the session

Status: accepted. Builds on [ADR 0136](0136-settings-pages-patterns-and-activation.md)
and [ADR 0137](0137-shared-system-service-clients.md).

The Power page offers what closing the lid and pressing the power button do.
logind owns both switches. Its system policy (`HandleLidSwitch`,
`HandlePowerKey` in `logind.conf`) is the administrator's, applies to every
user and needs root to change.

## Options considered

- **A polkit-authorized `logind.conf.d` drop-in.** Canopa has no existing
  privileged helper or polkit action for writing system files, and adding
  one would mean a new privileged mechanism, a policy file and a reload of
  logind, for a preference that belongs to one person's session. Rejected.
- **logind's handler inhibitors in the session.** logind documents
  `handle-lid-switch` and `handle-power-key` block inhibitors for exactly this:
  while a session holds one, logind leaves that switch to the session, which
  asks logind for the action it chose. When the session ends the descriptor
  closes and the system policy applies again. GNOME's power plugin works this
  way. Chosen.

## Decision

**Lid.** `~/.config/canopa/power.kdl` holds `lid-close` with `system` (the
default), `suspend`, `hibernate`, `lock`, `nothing` or `power-off`. The Power
page saves it with the usual file-backed Save. The Settings display profile
watcher (`canopa-settings watch-display-profiles`, `canopa-display-profiles.service`)
already runs for the whole Canopa session, so it also follows this file;
no new process is added. For any choice other than `system` it holds the
`handle-lid-switch` block inhibitor and, on each open-to-closed transition,
asks logind to suspend, hibernate or power off (interactive, so polkit can
ask), or asks logind to lock the session (the lock listener answers logind's
`Lock` signal). The lid state comes from UPower's `LidIsClosed` change signal,
or logind's `LidClosed` polled every second without UPower. While logind
reports `Docked`, closing the lid does nothing, like logind's default
`HandleLidSwitchDocked=ignore`. A lid already closed when the choice changes
is not acted on until it opens again. A restarted logind gets a new inhibitor.
The client is `canopa_services::power_switches`.

**Power button.** Niri already takes the `handle-power-key` inhibitor and
suspends on the power key unless `input { disable-power-key-handling }` is
set. The page offers **Suspend** (Niri's default) or **Use the system setting**
(Settings writes `disable-power-key-handling` into the managed input include,
so logind's `HandlePowerKey` applies, usually power off). It applies at once,
like the compositor motion control, and only when the Keyboard & pointer page
has no unsaved edits. Other power-key actions would need both a compositor key
binding and an inhibitor holder and are left for later.

System services keep their state: logind performs every action and still
enforces its inhibitors and polkit; Canopa stores only the person's choice.

## Consequences

- Outside a Canopa session, or if the watcher is stopped, the system policy
  applies. A failure in lid handling is logged and does not stop display
  profile watching.
- The idle timeouts on the same page save without Stasis, which stays
  optional; a running Stasis reloads them and the previous file returns if it
  refuses.
