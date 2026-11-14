# 0148 Session surface presentation

Status: accepted. Builds on [ADR 0121](0121-greetd-greeter-and-shared-session-presentation.md),
[ADR 0123](0123-pinentry-protocol-and-shared-dialogs.md) and
[ADR 0125](0125-shared-authentication-input-and-accessibility.md).

## Decision

`canopa-session-ui` draws the greeter, lock screen, polkit agent and pinentry in
three arrangements of one visual language:

- **Form** (sign-in and unlock): a centred identity (picture, or initials on a
  stable per-person colour), a pill field whose submit button sits inside it,
  status lines and centred text actions. Secret fields have an eye control
  (`input::REVEAL_CONTROL`, also an accessible action) that shows the typed
  response; the view keys it to the prompt, so any new prompt is masked again.
- **Sheet** (choosers, power and password-change steps): a card with the dialog
  heading.
- **Dialog** (polkit, pinentry): heading tile with title and subtitle, a request
  card of caption/value sections separated by dividers, a labelled pill field and
  a footer whose actions keep one primary style.

System controls (desktop, keyboard layout, battery, power) are chips at the
bottom screen corners. A layout region at those corners moves them into the
column instead, so configured widgets keep their places.

The greeter lists known accounts as rows (`ControlKind::Account`). Up/Down,
Enter and typing keep their existing meanings; a row click chooses that account
and begins authentication without discarding a desktop chosen on the screen.
With `remember-selection`, the greeter begins one attempt for the remembered
last user at startup (after its greetd reset), so the screen opens on their
password as the lock screen does. An untouched attempt that times out restarts
quietly; any input, cancellation or power request ends that, so a person's
cancel always returns to the account list. A configured `default-user` is only
preselected. Otherwise authentication begins on a person's action.

Dialog windows fit their content. `SessionView::dialog_height` reports the
natural height; clients size the window from it at creation and grow it, by
committing the larger buffer themselves, when a later step needs room. Niri does
not configure a mapped floating window again after its size limits change, and
a configure never shrinks the window below the requested height.

Requesters are named for context only: polkit maps the subject's command line to
its desktop entry's display name; pinentry accepts gpg-agent's `OPTION owner`
from the local host and names the client, or for GnuPG's tools the program that
ran them. A process chooses its own name, so the UI labels it "Requested by" and
the details keep the process ID.

## Validation

`examples/*-gallery.rs` in each consumer render representative states (dark and
light) without a compositor or authentication. The native greeter, polkit,
pinentry and session-tools suites cover the arrangements with real compositors;
`verify-greeter.py` drives the account list with the pointer.
