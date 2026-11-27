# Canopa Polkit authentication agent. English is the source language.

## Authorization dialog. The request's own message comes from polkit, already
## translated.

# Screen-reader name of the dialog.
polkit-title = Canopa Authorization
# Window title.
polkit-window-title = Authentication required
polkit-heading = Authorize system change
# Shown below the heading.
polkit-subtitle = This change needs administrator permission
polkit-request-details = Request details
polkit-details-hint = Details · Page Up/Down: page · Enter: back
polkit-retry = Retry
polkit-authenticate = Authorize
polkit-cancel = Cancel
polkit-policy-details = Request details
polkit-back-to-authentication = Back to authorization
# $page of $count pages of request details.
polkit-next-page = Next page ({ $page }/{ $count })
polkit-waiting-for-authentication = Waiting for authentication…
polkit-failed = Incorrect password. Try again.
polkit-cancelling = Cancelling authentication…
# The authenticating account when it is the person's own.
polkit-your-account = Your account
# An account without metadata. $uid is a numeric user ID.
polkit-user-id = User ID { $uid }
polkit-uid = UID { $uid }
# An account in the request details. $name is a person's name.
polkit-identity = { $username } (UID { $uid })
polkit-identity-named = { $name } · { $username } (UID { $uid })
# The process that asked. $pid is a process ID.
polkit-process = Process { $pid }
polkit-request-subject = Request subject: Process { $pid }
polkit-request-subject-named = Request subject: { $name } (process { $pid })
# $publisher is the vendor of the polkit policy.
polkit-policy-publisher = Policy publisher: { $publisher }
# Sample request in rendered previews, standing in for polkit's own text.
polkit-preview-message = Change DNS servers
polkit-preview-prompt = Password:

# Captions in the request details card.
polkit-fact-description = Request
polkit-fact-action = Policy action ID
polkit-fact-account = Account
polkit-fact-requester = Requested by
polkit-fact-publisher = Policy source
# A requesting process with its application name.
polkit-checking = Checking…
polkit-requested-by = Requested by { $name }
polkit-admin-required = Administrator account
# Label of the password field when PAM asks for a plain password.
polkit-admin-password = Administrator password
polkit-password-hint = Your password is used only to approve this change
