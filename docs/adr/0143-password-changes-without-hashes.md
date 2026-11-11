# 0143 Password changes without handling hashes

Status: accepted. Builds on [ADR 0136](0136-settings-pages-patterns-and-activation.md)
and [ADR 0137](0137-shared-system-service-clients.md).

The Users page changes passwords: one's own, and, for an administrator,
another person's. AccountsService's `SetPassword` takes a crypt(3) hash that
the client computes. Doing that in Settings would mean choosing a hash method
and parameters, handling the clear password and the hash in a desktop
process, and bypassing PAM: password quality rules (`pam_pwquality`),
history (`pam_pwhistory`), and the check of the current password.

## Options considered

- **`SetPassword` with a hash computed in Settings** (GNOME's approach).
  Rejected for the reasons above.
- **A PAM `chauthtok` transaction in Settings**, like the lock screen's
  isolated authentication worker. It would verify the current password and
  apply the rules, but needs a second PAM service file, FFI and an isolated
  worker for one dialog that `passwd` already provides.
- **`passwd` for one's own password.** `passwd` runs the system's `passwd`
  PAM stack: it verifies the current password, applies the quality and
  history rules and writes the shadow entry through its own setuid path.
  Settings sends the current password and the new one twice on its standard
  input and reports its outcome. Chosen.
- **For another account, a new password at the next login.** AccountsService's
  `SetPasswordMode(1)` makes the person choose a password when they next log
  in, after polkit authorizes the administrator. Chosen; Settings never sets
  another person's password itself.

## Decision

- Your account: **Change password** runs `passwd` with `LC_ALL=C`, writes the
  three lines and closes its input; success is its exit status. Its messages
  are mapped to "current password not correct", "too weak: <reason>", "do
  not match" and "changed too recently"; nothing else of its output is shown.
  The typed passwords are not part of the live page's retained actions; they
  are wiped from the form when sent, when the page is left or reloaded.
- Other accounts: **Ask for a new password at next login**. New accounts get
  the same mode, so their owner chooses the first password.
- Adding, removing (keeping or deleting the home folder), the account type
  (AccountsService's administrator type, the `wheel` group on Arch), the full
  name and the picture go through AccountsService, which asks polkit.
- Pictures follow the login screen's limits (PNG or JPEG, at most 2 MB and
  2048 pixels a side), checked from the file header before AccountsService
  copies the file.
- Automatic login is not offered: the Canopa greeter and greetd do not read
  AccountsService's `AutomaticLogin`.

## Consequences

- No hash, salt or crypt method in Canopa.
- Changing one's own password depends on `passwd` reading from a pipe, as
  `passwd` with `pam_unix` does through libpam's conversation.
- Between "ask at next login" and that login, the account has no password;
  the page says so.
