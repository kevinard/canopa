# Canopa PIN and passphrase entry. English is the source language.

## Passphrase dialog. GnuPG's own title, description, prompt and button
## labels are shown as sent; these are Canopa's defaults and messages.

# Window title and screen-reader name.
pinentry-title = Canopa Pinentry
pinentry-keyring-title = Canopa Keyring
# Window title with the request's title. $name is pinentry-title or
# pinentry-keyring-title.
pinentry-window-title = { $name } · { $title }
pinentry-passphrase-required = Passphrase required
pinentry-request-details = Request details
pinentry-key-details = Key details
pinentry-confirm-passphrase = Confirm passphrase
pinentry-unlock-openpgp-key = Unlock OpenPGP key
# Instructions below the heading.
pinentry-subtitle-unlock = GnuPG needs this key to continue
pinentry-subtitle-repeat = Enter the same passphrase again
pinentry-subtitle-confirm = Review this request before continuing
# Under the heading. $name is the program that asked, such as git.
pinentry-requested-by = Requested by { $name }
pinentry-confirmation-required = Confirmation required
pinentry-message = Message
# Defaults when the agent sends none.
pinentry-default-prompt = Passphrase:
pinentry-default-ok = Continue
pinentry-default-cancel = Cancel
pinentry-default-repeat-error = Passphrases do not match. Try again.
pinentry-repeat-passphrase = Repeat passphrase:
pinentry-repeat-password = Repeat password:
# Buttons in the request details, with their keys.
pinentry-details-back = Back to the request
# $page of $count pages.
pinentry-details-next = Next page (Page Down) · { $page } / { $count }
pinentry-details-previous = Previous page (Page Up)
pinentry-hide-generated = Hide generated passphrase
pinentry-reveal-generated = Reveal generated passphrase
# Name of the agent's passphrase quality measure when it gives none.
pinentry-quality-name = Quality
# $name is the measure's name (from the agent or pinentry-quality-name);
# $value a formatted percentage.
pinentry-quality = { $name }: { $value }
pinentry-quality-rejected = { $name }: { $value } · agent rejects this passphrase
pinentry-quality-unavailable = Agent quality feedback unavailable.
pinentry-requesting-generated = Requesting a passphrase from the agent…
pinentry-record-generated-first = Reveal and record the generated passphrase before continuing.
pinentry-record-generated = Reveal and record this generated passphrase before continuing.
pinentry-generate-failed = The agent could not generate a passphrase.
# $limit is a number of bytes.
pinentry-constraint-limit = Passphrase exceeds the agent constraint check's { $limit }-byte limit.
pinentry-checking-requirements = Checking the agent's passphrase requirements…
pinentry-check-failed = Could not verify the agent's passphrase requirements. Try again or cancel.
# $limit is a number of characters.
pinentry-length-limit = Passphrase limit: { $limit } characters. Input was rejected.

# Headings and lines for new passphrases and destructive confirmations.
pinentry-create-passphrase = Create a passphrase
pinentry-subtitle-new = Needed to use the key — it can’t be recovered if forgotten
pinentry-confirm-deletion = Confirm deletion
pinentry-unlock = Unlock
# Captions in the request details card.
pinentry-fact-key = Key
pinentry-fact-request = Request
pinentry-fact-requested-by = Requested by
pinentry-fact-requirements = Requirements
# Passphrase strength reported by the agent.
pinentry-strength-rejected = Too weak — the agent won’t accept it
pinentry-strength-weak = Weak — add another unrelated word
pinentry-strength-fair = Fair — add more unrelated words
pinentry-strength-good = Good — another unrelated word makes it stronger
pinentry-strength-strong = Strong passphrase
pinentry-strength-hint = Several unrelated words make a strong passphrase
pinentry-incorrect-passphrase = Incorrect passphrase. Try again.
pinentry-subtitle-permanent = This can’t be undone
pinentry-delete-key = Delete secret key?
# $count is the number of attempts left.
pinentry-incorrect-passphrase-left = { $count ->
    [one] Incorrect passphrase. 1 attempt left.
   *[other] Incorrect passphrase. { $count } attempts left.
}
pinentry-field-hint = Your passphrase stays on this computer
