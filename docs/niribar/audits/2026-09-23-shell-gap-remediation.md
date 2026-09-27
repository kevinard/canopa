# Desktop shell implementation gaps — remediation

This implements findings F1–F7 from the [desktop shell readiness audit](2026-09-23-desktop-shell-readiness.md), whose baseline was `988f970`. niri remains the primary compositor. The separate roadmap for a complete desktop environment is not implemented by this change.

| Finding | Implementation | Regression evidence |
| --- | --- | --- |
| F1: applications killed by command timeout | Explicit argv-only `launch` action; shipped terminal/browser actions migrated. Bounded commands keep their timeout and process-group cleanup. | Configuration accepts literal arguments with shell automation disabled; menu launch closes the panel; a compositor-owned fixture runs past three seconds after the launch client exits. |
| F2: applications inherit bar service ownership | Executable launches belong to niri; D-Bus activation belongs to the session bus. The executable fallback uses a separate systemd user scope. No change to the bar unit's KillMode. | Typed IPC preserves arguments and propagates rejection; launch-client lifetime fixture. A real user-service restart remains a graphical acceptance check. |
| F3: incomplete desktop-entry activation | Preserve selected entry path and desktop ID. GIO owns Exec field expansion and Path; D-Bus-enabled entries use an acknowledged session-bus Activate call. Localized metadata is searchable. Missing executable/entry and handoff failures are logged. | Real GIO expands `%c`, `%i`, `%k`, removes unused file placeholders and honors Path. Private-bus activation covers twenty acknowledged calls, symlink identity, dashed filenames, directory-prefixed discovery IDs, remote rejection and missing service. Locale fallback regression. |
| F4: inconsistent audio backend, stale missing sink | Both observation and control use libpulse against the same selected server. Adjustments serialize read/modify/write. Removed/missing sinks clear readings; stale overlapping queries are discarded. | Private PipeWire/Pulse protocol daemon with synthetic sinks exercises volume up/down, default switch, mute observation, last sink removal, reappearance and server loss. Production control invokes no helper executable. |
| F5: absent/full batteries shown as charging | Preserve IsPresent and all UPower states; absent battery maps to Empty Module. Full, pending and unknown states have distinct format selectors. | Private UPower service changes presence and state without percentage changes, including removal/reappearance; module mapping verifies Empty versus visible. Existing owner-restart test retained. |
| F6: misleading connectivity and link labels | Separate unknown, portal, link-connected and Internet-confirmed states. networkd lists usable online links and watches link changes; NM also refreshes profile renames. | Private services cover NM unknown/portal/profile rename and networkd Ethernet/Wi-Fi handover with unchanged manager state and disconnected/virtual link noise. |
| F7: menu-only primary click | Retain ItemIsMenu. Primary click opens the host menu on the clicked output; items without an exported menu use ContextMenu. | Private-bus menu-only fixture verifies ContextMenu rather than Activate; module interaction test verifies the source window and menu path. |

## Configuration and dependencies

Existing user configuration is not rewritten. Replace application-opening `command "kitty"` with `launch "kitty"` (and likewise for other applications). `command` remains automation with its configured deadline; `launch` arguments have no implicit shell expansion. See [ADR 0012](../adr/0012-session-owned-application-launches.md).

Ordinary Exec desktop-entry launching requires `gio`; fallback executable launch requires `systemd-run` and a user manager. niri handoff errors do not trigger a second launcher, avoiding duplicate launches after an ambiguous response. D-Bus activation awaits its reply and reports protocol error names without remote message content. The niri acknowledgement confirms handoff, not successful application initialization: later errors belong to niri/GIO diagnostics. The fallback likewise logs unsuccessful scope exit.

Network format selectors now include `unknown`, `connected` and `portal` alongside `full`, `limited` and `none`. Battery selectors additionally expose `full`, `pending-charge`, `pending-discharge` and `unknown`. Fully charged and pending batteries no longer match `charging`.

## Validation boundaries

The suite uses private D-Bus daemons, a fake compositor IPC endpoint, actual GIO, and private PipeWire/Pulse daemons with no hardware discovery. CI installs the fixture dependencies. No running desktop service, network connection or audio device is modified.

Hardware/compositor acceptance still includes an actual Niribar service restart with applications running, cold D-Bus service activation, a native PulseAudio-only server, real battery hardware, representative tray clients, and the existing accessibility/output matrix. These are not claimed by the private fixtures. Terminal desktop entries, activation tokens, desktop actions and SNI attention/overlay/theme-path expansion remain follow-on features, as scoped in the audit.

The private audio fixture supplies active default-device metadata itself because it deliberately has no desktop session manager. PipeWire's [Pulse server implementation](https://raw.githubusercontent.com/PipeWire/pipewire/master/src/modules/module-protocol-pulse/pulse-server.c) observes that metadata to update client defaults. This is fixture setup, not additional production coupling to PipeWire.

## Final verification

`cargo test --locked --all-targets -j 2` passed all 1,191 tests with no failures or ignored tests: 17 library, 1,156 application, 6 Canopa, 2 provider protocol and 10 Wi-Fi. The recursively invoked AT-SPI fixture is counted once. `cargo clippy --locked --all-targets -- -D warnings`, formatting and diff whitespace checks pass; local vendor provenance and all 21 default contrast checks also pass. Toolchain remains Rust/Cargo 1.97 nightly, not an additional Rust 1.95 acceptance run. The [verification record](data/2026-09-23-shell/remediation-verification.txt) retains the final command results.

## Standards review

Review against `988f970...f97a05e` found two documented-standard issues: diagnostics included configured executable text contrary to ADR 0006, and the niri handoff lacked a total deadline. Both were corrected. The handoff now uses one async deadline around connection, request transmission and bounded reply receipt; a trickling-reply regression prevents per-read timeout resets. Diagnostics omit raw argv and raw compositor error replies. A follow-up review confirmed both fixes and found no new standards issues.

One optional duplication concern was retained deliberately: NetworkManager and networkd have similar observation loops but separate snapshot contracts. No generic observer abstraction was added for these two adapters.

## Spec review

The independent requirements review found no missing implementation, scope creep or incorrect behavior within F1–F7. The hardware and real-service acceptance limits above remain explicit.

Final review: zero unresolved standards violations; zero Spec findings. One optional standards maintainability observation was accepted as a tradeoff.

The final parallel run exposed a GIO CLI activation that exited successfully without delivering the private-bus request. Twenty isolated attempts subsequently passed. The [GIO implementation](https://raw.githubusercontent.com/GNOME/glib/main/gio/gdesktopappinfo.c) queues activation and flushes asynchronously, supporting a possible exit race rather than establishing the exact cause of that one failure. Direct acknowledged activation removes that delivery uncertainty and surfaces remote errors; ordinary Exec entries still use GIO. The native path follows the [desktop activation protocol](https://specifications.freedesktop.org/desktop-entry/latest/dbus.html).

Both review axes rechecked the final native D-Bus activation change and found no additional issues. The activation fixture owns its bus service before launching; it does not claim cold-start acceptance.
