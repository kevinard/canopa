# Polkit authentication agent

`canopa-polkit` shows the password dialog when a system service asks polkit for
authorization, for example when Settings adds a printer or changes the time zone.
Polkit decides which actions need authentication and which accounts may answer; the
native polkit agent library checks the credentials and hands polkit the result.
Canopa only presents the conversation. It does not run the action and installs no PAM
policy.

## Setup

**Arch package.** `canopa-polkit` is in `/usr/bin` and `canopa-polkit.service` starts
with the Canopa session (`canopa-session.target`). Nothing needs enabling. The
`canopa` package depends on `polkit`.

**Local layout.** Install `packaging/systemd/canopa-polkit.service` with the other
user units and enable `canopa-session.target` as described in
[session tools](session-tools.md#starting-the-tools). Building needs the
`polkit-agent-1` development files (Debian/Ubuntu: `libpolkit-agent-1-dev`). In a
session without these units, start `canopa-polkit` once from the session's startup.

Only one authentication agent can register per session; stop any other agent first.
The unit reports ready after registration, restarts on failure (at most three times in
30 seconds) and stops with the graphical session.

```sh
canopa polkit                          # run the agent
canopa polkit --session 3              # choose the logind session
canopa polkit --defaults check-config
canopa polkit --defaults preview /tmp/polkit.png
```

The agent runs as the desktop user and registers for the user's graphical logind
session: `--session ID`, else `XDG_SESSION_ID`, else the only graphical Wayland
session of the user. With several graphical sessions, pass `--session`. Other users'
sessions and greeter sessions are refused.

## The dialog

Nothing is shown while idle. A request opens an `org.canopa.Polkit` window titled
"Authentication required"; the packaged Niri configuration opens it floating. The
dialog shows the request's description, the account being authenticated and
"Requested by": the name of the requesting program, taken from its command line and
matching desktop entry. A program chooses its own name, so treat it as context;
**Request details** lists the request, action ID, account, requesting program with
its process ID, and policy source; long values scroll.

| Key | Action |
| --- | --- |
| Enter | Submit, or go back from details |
| Escape | Cancel the request |
| Tab | Next permitted account (clears the field and starts over) |
| Page Down | Open request details |
| F6 | Next keyboard layout |

Only the accounts polkit offers can be chosen; there is no free username field. They
are labeled "Your account", `root`, or by name and picture from AccountsService (NSS
as a fallback). The UID is what polkit authenticates.

Secret prompts are masked; visible prompts (one-time codes) follow polkit's echo flag.
Messages from PAM modules are shown, and device factors such as fingerprints can
complete without typing. While polkit checks a response the field stays in place
with a spinner. After a failed attempt the field stays open: type again and press
Enter, and the dialog restarts authentication and sends the new response when polkit
asks, no sooner than two seconds after the failure. How many attempts are allowed is
up to polkit and PAM. Opening request details clears unsent input.

One request is handled at a time; overlapping requests are refused. Cancelling,
timing out or logging out cancels the request. If the native library hangs while
cancelling, a watchdog ends the agent and systemd restarts it.

The dialog uses the shared Iced session controls, with hover feedback,
release-to-activate buttons, icon tooltips and pointer text selection. See
[shared authentication input](session-tools.md#shared-authentication-input).

## Configuration

The agent reads the shared `appearance` and a `polkit` block from
`~/.config/canopa/config.kdl` at startup. Restart the service to apply changes.

```kdl
schema-version 1
polkit {
    auth-timeout 120
    font "Inter"
}
```

| Key | Value | Default |
| --- | --- | --- |
| `auth-timeout` | 5–600 seconds per request | 120 |
| `font` | family name | the first `appearance` typography family |

No other keys exist; the layout and widgets are fixed. `canopa config check` validates
this block with the other components. See [Configuration](configuration.md).

## Known limitations

- Configuration changes apply only after restarting `canopa-polkit.service`.
