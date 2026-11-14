# PIN and passphrase dialogs

`canopa-pinentry` provides native Wayland PIN/passphrase, confirmation and message
windows for GnuPG. It reuses Canopa's appearance, layout, controls and secret input.
GnuPG launches it on demand over Assuan stdin/stdout; no user service is needed.
GnuPG remains responsible for keys, credential checks and its agent cache.

After installing the executable, set its **absolute installed path** in
`~/.gnupg/gpg-agent.conf`, for example:

```text
pinentry-program /home/you/.local/bin/canopa-pinentry
```

Reload your agent after changing this setting:

```sh
gpgconf --reload gpg-agent
```

The agent must inherit your graphical session's `WAYLAND_DISPLAY` and
`XDG_RUNTIME_DIR`. When starting the agent before the graphical session, import the
current session environment before the first graphical prompt. For example, from a
terminal inside Canopa:

```sh
gpg-connect-agent updatestartuptty /bye
```

GnuPG must allow updating `WAYLAND_DISPLAY` (normally in its standard environment
list). Consult your distribution's GnuPG setup if it overrides that list. Canopa
never changes your GnuPG configuration, starts/reloads your host agent, or replaces
another installed pinentry automatically. A missing Wayland display returns an
Assuan error; there is no terminal/X11 fallback.

Passphrases stay masked. Enter activates the marked button, Tab moves between
buttons, Escape/window close cancels, Backspace deletes and Ctrl+U clears input.
Holding a key does not repeat it, so the toolkit does not retain cleartext repeat
events. Page Down opens/paginates full request details, Page Up goes to the preceding page, and
Enter returns from details. Opening details clears both passphrase entries.
Requests for a new passphrase show a second entry; mismatches restart the pair.
Caller descriptions, labels, errors and window titles are displayed as plain text;
underscore accelerator markers are removed from labels. Letter accelerators are
not bound. Confirmation/message windows have no secret field.
gpg-agent's `OPTION owner=PID/UID HOST` names the requesting program under the
heading ("Requested by git"): the client's command name, or for GnuPG's own tools
the program that ran them. A forwarded agent's PID from another host is ignored.
The window's height fits the current request and grows when a later step, such
as a mismatch message, needs more room.

Shared configuration is optional:

```kdl
schema-version 1
pinentry {
    font "Inter"
    auth-timeout 120
}
```

`auth-timeout` is the initial dialog timeout in seconds (5–600); the agent can
override it with `SETTIMEOUT`, including zero to disable it. The `appearance` block
applies as for other Canopa components. No scripts, wallpaper, account discovery or
passive widget providers are started by pinentry. Configuration is loaded when
GnuPG starts the process. Validate or preview without connecting to Wayland/GnuPG:

```sh
canopa pinentry --defaults check-config
canopa pinentry --defaults preview /tmp/pinentry.png
canopa config check
```

The Assuan server supports GETPIN, CONFIRM (including `--one-button` and
`--focus=ok/cancel`), MESSAGE, text/label/error/repeat setters, SETTIMEOUT,
SETKEYINFO, RESET, NOP, BYE, GETINFO pid/version/flavor, terminal/locale metadata
options and default prompt/OK/cancel labels. It percent-decodes UTF-8 arguments,
percent-escapes data, bounds lines to 1000 bytes and splits long responses into
valid data frames. Repeat requests emit `PIN_REPEATED` only after matching entries.
Passphrases support up to 2048 Unicode characters; excess input is rejected with a
visible message. Timeout, cancellation, negative confirmation and unavailable
Wayland have distinct libgpg-error codes. EOF retires an active dialog.

External caches and passphrase formatting remain unsupported. Quality, constraints
and generation are described below. Canopa-owned response storage uses dedicated
best-effort locked, non-dumpable mappings and is erased on edits/cancellation/end.
Shared presentation supports keyboard selection, compositor layouts, AT-SPI and
complex-script shaping; see [shared input](session-tools.md#shared-authentication-input).
Core dumps and ptrace dumping are disabled, and credentials are never logged.
Other processes with equivalent
session privileges/compositor access remain outside pinentry's protection.

`scripts/verify-pinentry.py` exercises actual keyboard input in a private Wayland
session and performs a symmetric encryption/decryption round trip using a real
GnuPG agent in a temporary GNUPGHOME. It does not access the host keyring or agent.
See [the design decision](adr/0123-pinentry-protocol-and-shared-dialogs.md).

Protocol references: [GnuPG's pinentry manual](https://github.com/gpg/pinentry/blob/master/doc/pinentry.texi)
and [Libassuan protocol](https://www.gnupg.org/documentation/manuals/assuan/).

Agent-supplied quality and generation controls are supported through `SETQUALITYBAR`,
`SETQUALITYBAR_TT`, `SETGENPIN` and `SETGENPIN_TT`. Quality inquiries are debounced
while editing, run off the rendering loop, and show the agent's signed score.
`OPTION constraints-enforce` requires a successful `CHECKPIN` inquiry before a
passphrase can be returned. The agent's short/long constraint hints appear in
Details. Missing, failed or cancelled checks cannot approve a passphrase. Required
checks reject values exceeding the upstream inquiry's 300-byte limit rather than
checking only a prefix. Ordinary passphrases retain the existing 2048-character
limit when this agent option is absent.

Generation uses `GENPIN` from the calling agent. The result starts masked and must
be explicitly revealed before acceptance; record it before continuing. It can be
hidden again before submission. Cursor movement and partial edits preserve the
reveal requirement; clearing or replacing the entire value starts an ordinary
entry. There is no clipboard write or local generator.
Repeat confirmation and agent constraints still apply. Inquiry response bytes and
requests use erased transport buffers; owned response storage uses best-effort
locked mappings. An absolute two-second inquiry deadline bounds partial replies.
An unresponsive inquiry does not prevent cancelling the window.

These exchanges follow the primary
[GnuPG pinentry implementation](https://github.com/gpg/pinentry/blob/master/pinentry/pinentry.c);
the calling agent owns password requirements and generated values.
