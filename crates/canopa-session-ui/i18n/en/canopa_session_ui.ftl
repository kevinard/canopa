# Canopa Shared lock and login presentation. English is the source language.

## Lock screen, login and authorization dialogs (shared presentation)

# Sample prompt in previews of the lock screen.
session-password-prompt = Password:
# Battery widget while charging. $level is a formatted percentage ("45%").
session-battery-charging = { $level } · Charging
# Button that switches keyboard layout; F6 is the key that does the same.
# $layout is the system's layout name.
session-keyboard-control = Keyboard: { $layout }   · F6
# Accessible name of the keyboard layout switch. $layout is the layout name.
session-keyboard-layout = Keyboard: { $layout }
# Keyboard layout shown beside the field; F6 switches it.
session-keyboard-layout-shortcut = { $layout } · F6
# Placeholders inside the response field.
session-placeholder-type-response = Type response
session-placeholder-enter-response = Enter response
session-placeholder-authenticate = Enter to authenticate
session-waiting-for-authentication = Waiting for authentication…
session-caps-lock-on = Caps Lock is on
# Hints below the response field.
session-hint-follow-instructions = Follow the authentication instructions
session-hint-press-enter = Press Enter to continue
session-hint-escape-cancels = Escape cancels the current attempt
# Last line of the lock-screen notification list. $count is the number of
# notifications not shown.
session-notifications-more = { $count ->
    [one] +{ $count } more
   *[other] +{ $count } more
}
# Screen-reader description of the response field. $layout is the keyboard layout.
session-input-description = { $layout }. Enter submits. Escape cancels.
session-input-description-caps = { $layout }. Caps Lock is on. Enter submits. Escape cancels.
# Screen-reader status after a failure. $message is the current status message.
session-status-error = { $message }. Authentication error
# Screen-reader names of the submit and cancel actions.
session-action-submit = Submit response
session-action-cancel = Cancel authentication
# Screen-reader description of the current choice in a list. $detail is its subtitle.
session-current-selection = Current selection
session-current-selection-detail = Current selection. { $detail }
# The account that authenticates. $name is a person's name or username.
session-account = Account: { $name }
# Request card in authorization dialogs.
session-policy-source = Policy source
session-requested-for = Requested for
session-policy-source-value = Policy source: { $value }
session-requested-for-value = Requested for: { $value }
session-authenticating-as = Authenticating as
# Card labels shown in capitals.
session-policy-request = POLICY REQUEST
session-key-identity-caps = KEY IDENTITY
session-request-context = REQUEST CONTEXT
session-fingerprint = Fingerprint
session-key-identity = Key identity
# Lock screen heading above the person's name.
session-locked-heading = Session locked
# A back button. $arrow is ← (→ in right-to-left languages); $label is the action.
session-back-label = { $arrow } { $label }
# Chip on the current choice in a list (short).
session-current = Current
# Login screen row that opens the desktop chooser. $session is the desktop's name.
session-desktop-choice = Desktop · { $session }
# Keyboard help in choice lists.
session-chooser-shortcuts = ↑ ↓ to move · Enter to select
