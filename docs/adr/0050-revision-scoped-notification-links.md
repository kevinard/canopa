# 0050: Revision-scoped notification links

Status: accepted; implementation and graphical acceptance in progress.

Notification bodies retain explicit link labels, styles, and validated destinations. The initial opening policy permits absolute HTTP and HTTPS URLs with a host, at most 2048 bytes before and after URL normalization. Credentials, literal whitespace/control/bidi characters, backslashes, malformed percent escapes, and escaped ASCII controls/backslashes are rejected. Unsupported targets keep their visible styled text. Links are never fetched during receipt, layout, or history loading. Existing stored text remains text; ordinary reads do not reinterpret historical markup.

A link interaction identifies a body segment in the displayed notification revision. Center interactions also carry the daemon instance. The runtime resolves the destination itself, checks the revision again after activation-token acquisition, and admits at most eight concurrent opening requests. An admitted request retains its original destination even if the notification is subsequently replaced. No URI from an interaction event is trusted as a destination.

Link opening is independent of sender actions, default activation, expiry, and acknowledgement. Retained history can still open its links after the sender lifecycle ends; a deleted row cannot. Banners require an active revision. The activation target is the desktop's selected browser, so token requests omit the optional target application ID when it is unknown.

Use the desktop OpenURI portal, passing the URI as data. Listen for the Request response before submission, pin the portal's unique bus owner, and distinguish completion, cancellation, and failure. A timed-out or cancelled portal request is never retried through a different launcher. Request cleanup is bounded. Diagnostics contain failure categories, never raw notification URLs.

The control reply confirms admission of the selected link request. Portal completion runs independently so an application chooser cannot hold the control socket or provider input queue. Daemon status exposes availability, pending count, and a sanitized last failure. Cancelling a chooser is a normal outcome. Daemon shutdown cancels pending requests and waits for bounded cleanup before releasing its bus connection.

Generic panel links carry a labeled action with an opaque provider value plus the validated URI as accessibility metadata. The host exposes the real URI through AT-SPI Hyperlink so Orca can describe the destination; it never opens that URI independently. Runtime activation still resolves the current revision and ignores presentation metadata as a command target. Real Orca verification exposed that a Link role without a Hyperlink target causes its link-description command to fail. This requires protocol version 4 and a matching host. Advertise `body-hyperlinks` only after parser, storage, opener, banner, host, and accessibility acceptance pass.

Sources and API details: [hyperlink opening research](../hyperlink-opening-research.md).
