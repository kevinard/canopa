# 0069: Iced native file clipboard

Status: implementation in progress, 2026-10-18. Continues the full PRD and
ADR0055/0056/0067/0068; it does not accept complete transfer or restart behavior.

Keep the established Copy/Cut/Paste commands and current Browser folder as the
Paste destination. Use a separate non-widget Wayland connection and worker for
file clipboard exchange, leaving Iced's text-editing clipboard intact. The
supported Canopa compositor exposes ext-data-control-v1: the private Niri 26.04
probe records both that protocol and its deprecated wlr predecessor. Prefer the
current ext protocol through the workspace's existing wayland-client and
wayland-protocols dependencies. Report unavailable service on unsupported
compositors; do not pretend text clipboard contents are files. Target Niri's
single seat and refuse an ambiguous multi-seat connection rather than selecting
another seat's clipboard. Losing that original seat requires a fresh connection.

Advertise GNOME copied-files, URI list and KDE cut-selection MIME types together.
Decode GNOME actions first, otherwise URI lists with the optional KDE marker.
Accept local file locations only, preserve original Unix bytes through percent
encoding, reject malformed locations and bound each payload to 16 MiB. Reject
parent-directory URI spellings and unescaped whitespace/backslashes that URL
normalization could silently turn into another filename. A read
has a five-second deadline and must still refer to the same offer when complete.
Peer data must not block the UI or the clipboard dispatch loop. Bound concurrent
reads/writes and wake the idle worker through a descriptor instead of polling.
Initialization uses the same interruptible descriptor loop and a five-second
deadline, including registry and device synchronization. Open a nonblocking
connection to the named Wayland display; never consume or mutate an inherited
`WAYLAND_SOCKET` descriptor owned by Iced. Report an unavailable service when no
named display is provided. Publish shared ownership state before acknowledging
Copy/Cut, so the Iced caller can retain the matching source placement immediately.

Capture source placement at Copy/Cut, separately from Paste's destination
binding. Clipboard ownership has a process-local opaque token. A completed or
partial Move may update only that still-current Cut; a new Copy/Cut or external
owner supersedes it. Automatic Cut updates change only the payload served by
the original source, or destroy that specific source when empty; they do not
issue set-selection again. This also protects a newer owner whose cancellation
notification has not reached the worker. Restarted operation metadata cannot control a new
clipboard. Keep owned file selections alive after Browser closure until the
selection is replaced, with primary-instance activation remaining a required
integration gate. Source retention, consuming-operation ownership and journal
reconciliation remain required for complete clipboard and restored execution.

The Iced adapter captures selection paths and the resolved Browser folder with
the original Storage snapshot before publishing Copy/Cut. It retains that
observation only for the matching ownership token; asynchronous replies from a
superseded publication cannot replace the new owner's metadata. Preparing the
selection and publishing it run outside the UI thread. Start the platform service
when the application starts, keeping rendererless controller construction free
of clipboard connections. Pending requests and actual file ownership keep the
application alive after Browser closure. Text fields keep Iced's text commands.
Selection preparation carries an invocation generation. Only the latest
prepared Copy/Cut may publish, so a slower older preparation cannot supersede
the user's newer command. Earlier failed publications cannot report failure for
the newer invocation.

Paste captures its destination Browser origin and source observation before the
asynchronous clipboard read. An owned selection requires its matching captured
source placement; missing metadata is an explicit refusal, never a substitution
of current storage. External selections use storage observed at Paste. Submit
the captured destination and source through the shared transfer service even if
the Browser later navigates or closes. Refresh affected views while preserving
their selection; reveal an actual reported destination only when the originating
Browser state still matches. Failed admission releases its pending application
hold; accepted work remains visible in Operations after Browser closure.

Verification extends the established public file-exchange seam with Selection
encoding/decoding and the worker's actual publish/read/ownership operations.
Use fixed URI fixtures and real private paths, then the existing private Niri
harness with independent wl-copy/wl-paste peers. Never attach a clipboard test
to the user's running display. Iced acceptance must exercise the real keyboard,
menus, conflicts, cancellation and source/destination bindings.
