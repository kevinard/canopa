# PIN and passphrase dialogs

`canopa-pinentry` is a GnuPG pinentry: gpg-agent starts it when it needs a PIN or
passphrase, a confirmation or a message, and talks to it over the Assuan protocol on
stdin and stdout. It shows a Wayland dialog in Canopa's style. GnuPG keeps the keys,
checks the passphrase and caches it. No service runs between prompts.

The same program also draws the keyring password dialog for the optional oo7 setup
(see [Keyring prompts](#keyring-prompts)).

## Setup

Set the absolute path in `~/.gnupg/gpg-agent.conf`:

```text
pinentry-program /usr/bin/canopa-pinentry
```

Use `~/.local/bin/canopa-pinentry` for the local layout. Then reload the agent:

```sh
gpgconf --reload gpg-agent
```

gpg-agent must have the session's `WAYLAND_DISPLAY` and `XDG_RUNTIME_DIR`. If the
agent started before the graphical session, update it from a terminal in Canopa:

```sh
gpg-connect-agent updatestartuptty /bye
```

Without a Wayland display the pinentry returns an error; there is no terminal or X11
fallback. Canopa never changes your GnuPG configuration.

## The dialog

The window (`org.canopa.Pinentry`, floating in the packaged Niri configuration) shows
the agent's title, description and prompt as plain text. "Requested by" names the
program behind gpg-agent's `OPTION owner` (for GnuPG's own tools, the program that ran
them); an owner from another host is ignored.

| Key | Action |
| --- | --- |
| Enter | Activate the highlighted button |
| Tab | Next button |
| Escape, closing the window | Cancel |
| Page Down / Page Up | Open and page through the full request details |
| F6 | Next keyboard layout |

Editing keys are the shared ones, see
[shared authentication input](session-tools.md#shared-authentication-input).
Opening details clears both passphrase fields. Holding a key does not repeat it.

- **New passphrases** ask twice; a mismatch clears both.
- **Quality**: when the agent sends `SETQUALITYBAR`, the dialog asks the agent for a
  score while you type and shows it.
- **Constraints**: with `OPTION constraints-enforce`, the passphrase is accepted only
  after the agent's `CHECKPIN` check succeeds. Checked passphrases are limited to
  300 bytes; otherwise passphrases can be up to 2048 characters.
- **Generate**: with `SETGENPIN`, the agent generates a passphrase. It starts masked
  and must be revealed (the eye button) before you can accept it, so you can write
  it down. Nothing is copied to the clipboard.

Inquiries to the agent time out after two seconds.

The dialog uses the shared Iced session controls, with hover feedback,
release-to-activate buttons, icon tooltips and pointer text selection. See
[shared authentication input](session-tools.md#shared-authentication-input).

## Configuration

Optional, in `~/.config/canopa/config.kdl`, read each time gpg-agent starts the
program. The shared `appearance` block applies too.

```kdl
schema-version 1
pinentry {
    font "Inter"
    auth-timeout 120
}
```

| Key | Value | Default |
| --- | --- | --- |
| `auth-timeout` | 5–600 seconds | 120 |
| `font` | family name | the first `appearance` typography family |

`auth-timeout` is the dialog timeout until the agent sends `SETTIMEOUT`, which
replaces it (0 disables it).

```sh
canopa pinentry --defaults check-config
canopa pinentry --defaults preview /tmp/pinentry.png
```

`--display` is accepted for gpg-agent compatibility and ignored.

## Protocol support

Commands: `SETDESC`, `SETTITLE`, `SETPROMPT`, `SETOK`, `SETCANCEL`, `SETNOTOK`,
`SETERROR`, `SETREPEAT`, `SETREPEATERROR`, `SETREPEATOK`, `SETKEYINFO`,
`SETQUALITYBAR`, `SETQUALITYBAR_TT`, `SETGENPIN`, `SETGENPIN_TT`, `SETTIMEOUT`,
`OPTION`, `GETINFO`, `GETPIN`, `CONFIRM` (including `--one-button`), `MESSAGE`,
`RESET`, `NOP` and `BYE`. Recognized options include `owner`, the `default-*` labels
and `constraints-enforce`. Timeout, cancel, "not confirmed" and "no display" return
distinct GnuPG error codes. The protocol follows the
[pinentry manual](https://github.com/gpg/pinentry/blob/master/doc/pinentry.texi) and
[Assuan](https://www.gnupg.org/documentation/manuals/assuan/).

## Keyring prompts

The Arch `canopa-oo7` package installs `/usr/lib/canopa/canopa-keyring-prompter`, a
GCR `org.gnome.keyring.SystemPrompter` service. Once you link its D-Bus activation
file into your account (see [installation](installation.md)), keyring password and
confirmation prompts in the active Canopa session use `canopa-pinentry keyring-dialog`
(window `org.canopa.Keyring`). Other desktops keep GCR's own dialog.

## Known limitations

- External password caches and passphrase formatting options are not supported.
- There is no fallback outside Wayland.
