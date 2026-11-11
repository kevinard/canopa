# Canopa Polkit authentication agent. English is the source language.

## Authorization dialog. The request's own message comes from polkit, already
## translated.

# Screen-reader name of the dialog.
polkit-title = Canopa Authorization
# Window title.
polkit-window-title = Authentication required
polkit-heading = Authorize system change
# Shown in capitals above the heading.
polkit-eyebrow = SYSTEM AUTHORIZATION
polkit-request-details = Request details
polkit-details-hint = Details · Page Up/Down: page · Enter: back
polkit-retry = Retry
polkit-authenticate = Authenticate
polkit-waiting = Waiting…
polkit-cancel = Cancel
polkit-policy-details = Policy details
polkit-back-to-authentication = Back to authentication
# $page of $count pages of request details.
polkit-next-page = Next page ({ $page }/{ $count })
polkit-waiting-for-authentication = Waiting for authentication…
polkit-failed = Authentication failed. Try again.
polkit-authenticating = Authenticating…
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
# $publisher is the vendor of the polkit policy.
polkit-policy-publisher = Policy publisher: { $publisher }
# Sample request in rendered previews, standing in for polkit's own text.
polkit-preview-message = Change DNS servers
polkit-preview-prompt = Password:
