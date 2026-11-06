# Canopa polkit agent

`canopa-polkit` supplies graphical authorization dialogs for the logged-in Wayland
session. It uses the shared Canopa authentication presentation and the native
`PolkitAgentListener` / `PolkitAgentSession` APIs. Polkit owns action policy,
authentication identities, its privileged backend and authorization receipts. The
agent does not execute the requesting action or install a PAM policy.

## Build and start

Install the distribution's polkit runtime, its native agent library and its
`polkit-1` PAM/backend configuration. Depending on the polkit build, the native
library uses the distribution's helper socket/service or privileged helper.
Development builds also require the `polkit-agent-1` pkg-config headers (Ubuntu:
`libpolkit-agent-1-dev`), plus the existing Wayland/xkbcommon development libraries.

```sh
cargo build --locked -p canopa-polkit -p canopa-cli
canopa polkit --defaults check-config
canopa polkit --defaults preview /tmp/polkit.png
canopa polkit
```

Run the agent as the desktop user. It validates the graphical logind session's
UID, type and class. User-manager services can discover a unique owned Wayland
login even when their PID is outside that session. Multiple graphical logins
require `--session ID` or `XDG_SESSION_ID`; foreign-user and greeter sessions are
refused. Only one authentication agent can register for a session. Registration
fails when another agent is already registered.

The release/updater includes the executable. Install the supplied
`packaging/systemd/canopa-polkit.service` into the user's unit directory alongside
the other Canopa units, then enable it when choosing Canopa as the session agent:

```sh
systemctl --user daemon-reload
systemctl --user enable --now canopa-polkit.service
```

The unit waits for Niri to publish its Wayland environment, reports readiness only
after polkit registration, and stops with the graphical session. It restarts on
failure with bounded retry frequency, including after a polkit daemon restart.
`canopa-session.target` also wants the agent. For a session without the companion
units, start `canopa-polkit` once from its compositor/session startup. The normal
Wayland environment and system bus must be available. Repository checks do not
enable the service, replace an existing agent, edit rules or change host PAM.

The agent has no mapped surface while idle. A request creates an ordinary
`org.canopa.Polkit` xdg-shell window titled “Authentication required”; completing
or cancelling it destroys the window. The compositor owns placement, focus and
session-lock behavior. Niri recognizes the fixed-size dialog; an explicit rule
can select floating presentation if local window rules override that policy:

```kdl
window-rule {
    match app-id=r#"^org\.canopa\.Polkit$"#
    open-floating true
}
```

## Appearance and interaction

The agent reads the shared Canopa configuration and appearance/theme. It has no
legacy per-application fallback. Its namespace supports the authentication
deadline and font; it runs no script widgets or external presentation commands:

```kdl
schema-version 1
appearance { color-scheme "dark"; }
polkit {
    auth-timeout 120
    font "Inter"
}
```

The deadline accepts 5..600 seconds and defaults to 120. Configuration is loaded
at startup. `--canopa-config PATH` and `--defaults` select explicit sources; the
unified `canopa config check` delegates this namespace to the agent. Preview and
schema validation need no compositor, logind, polkit connection or credentials.
Preview uses a sample request and performs no authentication.

Each dialog shows the request description, canonical action ID and the account
being authenticated. Only the supplied `unix-user` identities can be selected;
there is no arbitrary username entry. Root is labeled `root`, the current UID as
“Your account”, and other identities by UID. Tab cycles accounts while entering a
response or after a failed attempt; switching clears the response and starts a
fresh native conversation. Enter submits, Backspace edits, Ctrl+U clears input,
and Escape or Cancel dismisses the request. Pointer controls provide the same
actions. Keyboard repeat, output scale and Caps Lock reporting are supported.

Secret prompts are masked; visible prompts such as OTPs follow the backend's echo
flag. Every new prompt starts with empty input. Information/error notices and
passwordless device factors use the native conversation without adding a password
requirement. Failed attempts offer Retry with a two-second delay; three failures
close the request. Request details provides paged full description, action and
conversation text; Page Up/Page Down changes pages and Enter returns to authentication.
Opening details erases unsent input and cannot submit a credential. Shared presentation supports AT-SPI, complex-script shaping and keyboard-layout
selection; see [shared input](session-tools.md#shared-authentication-input).

## Request lifecycle

One request is admitted at a time; overlapping requests are refused. Incoming
agent calls are pinned to the authority's unique system-bus owner in addition to
the distribution's bus policy. Cookies stay in the native adapter and are never
put in window text, argv or Canopa logs. Canopa does not call the privileged
agent-response API itself: the native authentication backend supplies the receipt
which polkit uses to decide whether to grant the action.

Authority cancellation, dialog dismissal, timeout and logout cancel the native
session. Request generations and prompt serials reject stale answers and updates.
Backend work runs on a separate GLib context, keeping the Wayland UI responsive.
If native cancellation/shutdown does not return, a bounded watchdog ends the agent
and its bus registration so the session service can restart it. Cancellation does
not revoke an authorization receipt already delivered to polkit.

Agent method bodies are limited to 64 KiB, descriptions to 512 UTF-8 bytes, action
IDs to 256, identities to 32, and native prompt/notices to 2048. Unsupported or
oversized requests are refused. Responses use fixed-capacity erased buffers (up
to 1023 UTF-8 bytes), and the native command adapter erases its response copy.
Core dumps are disabled and the process is non-dumpable. `POLKIT_DEBUG` is removed
before native initialization; authentication and notification endpoints are not
inherited by the native helper. Native library/PAM memory belongs to those
components. There is no permanent rendering loop or idle status polling.

## Validation

```sh
cargo test --locked -p canopa-polkit --lib --test integration
python3 scripts/verify-polkit.py
python3 scripts/run-desktop-acceptance.py --case polkit-agent --output /tmp/canopa-polkit-acceptance
python3 scripts/test-polkit-helper.py
python3 scripts/test-session-units.py
bash scripts/check.sh --workspace
```

The native conversation test uses a private authority and an independent session
test double to check registration, caller filtering, identity selection, password/
OTP/device flows, stale responses, cancellation and overlap. Desktop acceptance
adds actual Niri windows, keyboard input, screenshots, idle/window lifetime,
readiness, failure recovery and authority restart.

The real-helper test uses user/PID/mount/network namespaces, a private bus,
synthetic account records and a test-only `polkit-1` PAM module. It exercises the
installed `PolkitAgentSession` and helper, wrong-password retry, fresh OTPs,
device-only PAM and the helper's receipt to the private authority. It does not
validate host privilege switching, real action rules, enrollment or the installed
machine's PAM/backend policy. Those require target-machine acceptance. Evidence
lives under `.scratch/polkit-agent/`.

Protocol references: [writing authentication agents](https://www.freedesktop.org/software/polkit/docs/latest/polkit-agents.html),
[native session API](https://www.freedesktop.org/software/polkit/docs/latest/PolkitAgentSession.html),
[agent D-Bus interface](https://www.freedesktop.org/software/polkit/docs/latest/eggdbus-interface-org.freedesktop.PolicyKit1.AuthenticationAgent.html).

The dialog resolves display names and photos only for the authority's selected,
permitted numeric UID. AccountsService `FindUserById` and individual public
properties provide labels and an optional icon; NSS supplies a fallback name.
It never enumerates other accounts or reads password hints, email or home metadata.
The UID remains the authentication identity regardless of the displayed label.
Missing metadata leaves the numeric label. Slow NSS and bounded local raster
loading run in a deadline-owned helper; selection/request changes cancel stale work.
