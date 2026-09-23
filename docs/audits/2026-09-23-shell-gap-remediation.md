# Desktop shell implementation gaps — remediation

This implements findings F1–F7 from the [desktop shell readiness audit](2026-09-23-desktop-shell-readiness.md), whose baseline was `988f970`. niri remains the primary compositor. The separate roadmap for a complete desktop environment is not implemented by this change.

| Finding | Implementation | Regression evidence |
| --- | --- | --- |
| F1: applications killed by command timeout | Explicit argv-only `launch` action; shipped terminal/browser actions migrated. Bounded commands keep their timeout and process-group cleanup. | Configuration accepts literal arguments with shell automation disabled; menu launch closes the panel; a compositor-owned fixture runs past three seconds after the launch client exits. |
| F2: applications inherit bar service ownership | niri Spawn IPC owns applications; fallback uses a separate systemd user scope. No change to the bar unit's KillMode. | Typed IPC preserves arguments and propagates rejection; launch-client lifetime fixture. A real user-service restart remains a graphical acceptance check. |
| F3: incomplete desktop-entry activation | Preserve selected entry path and desktop ID. GIO owns field expansion, Path and D-Bus activation. Localized metadata is searchable. Missing executable/entry and handoff failures are logged. | Real GIO expands `%c`, `%i`, `%k`, removes unused file placeholders, honors Path, and activates a D-Bus-only entry on a private bus, including a symlink with a different backing filename. Locale fallback regression. |
| F4: inconsistent audio backend, stale missing sink | Both observation and control use libpulse against the same selected server. Adjustments serialize read/modify/write. Removed/missing sinks clear readings; stale overlapping queries are discarded. | Private PipeWire/Pulse protocol daemon with synthetic sinks exercises volume up/down, default switch, mute observation, last sink removal, reappearance and server loss. Production control invokes no helper executable. |
| F5: absent/full batteries shown as charging | Preserve IsPresent and all UPower states; absent battery maps to Empty Module. Full, pending and unknown states have distinct format selectors. | Private UPower service changes presence and state without percentage changes, including removal/reappearance; module mapping verifies Empty versus visible. Existing owner-restart test retained. |
| F6: misleading connectivity and link labels | Separate unknown, portal, link-connected and Internet-confirmed states. networkd lists usable online links and watches link changes; NM also refreshes profile renames. | Private services cover NM unknown/portal/profile rename and networkd Ethernet/Wi-Fi handover with unchanged manager state and disconnected/virtual link noise. |
| F7: menu-only primary click | Retain ItemIsMenu. Primary click opens the host menu on the clicked output; items without an exported menu use ContextMenu. | Private-bus menu-only fixture verifies ContextMenu rather than Activate; module interaction test verifies the source window and menu path. |

## Configuration and dependencies

Existing user configuration is not rewritten. Replace application-opening `command "kitty"` with `launch "kitty"` (and likewise for other applications). `command` remains automation with its configured deadline; `launch` arguments have no implicit shell expansion. See [ADR 0012](../adr/0012-session-owned-application-launches.md).

Installed application launching requires `gio`; fallback launch requires `systemd-run` and a user manager. niri handoff errors do not trigger a second launcher, avoiding duplicate launches after an ambiguous response. The niri acknowledgement confirms handoff, not successful application initialization: later errors belong to niri/GIO diagnostics. The fallback likewise logs unsuccessful scope exit.

Network format selectors now include `unknown`, `connected` and `portal` alongside `full`, `limited` and `none`. Battery selectors additionally expose `full`, `pending-charge`, `pending-discharge` and `unknown`. Fully charged and pending batteries no longer match `charging`.

## Validation boundaries

The suite uses private D-Bus daemons, a fake compositor IPC endpoint, actual GIO, and private PipeWire/Pulse daemons with no hardware discovery. CI installs the fixture dependencies. No running desktop service, network connection or audio device is modified.

Hardware/compositor acceptance still includes an actual Niribar service restart with applications running, a native PulseAudio-only server, real battery hardware, representative tray clients, and the existing accessibility/output matrix. These are not claimed by the private fixtures. Terminal desktop entries, activation tokens, desktop actions and SNI attention/overlay/theme-path expansion remain follow-on features, as scoped in the audit.
