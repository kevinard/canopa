# 0021: User-input-bound focus for ordinary Wayland windows

Status: implementation in progress, 2026-10-18. Full shared-stack and file-manager
acceptance remain required.

Files transfer-result inspection exposed a real focus-return failure with three
ordinary Iced windows. Closing the inspector returned focus to another Browser,
not its Operations parent. The minimized native case reproduces this after
explicitly observing each focus transition. A private wire trace proves the
transient parent is correct and xdg_activation_v1 is advertised, but no token or
activation request is sent. Locked Iced GainFocus delegates to Winit's empty
Wayland focus_window implementation. See the diagnosis record under
.scratch/file-manager/ICED-PROGRESS-FOCUS-DIAGNOSIS.md.

Implement best-effort compositor-authorized focus in the single root Winit fork,
through its existing focus_window API. Capture recent keyboard focus/key,
pointer-press or touch input on that Wayland connection. Use the actual source
surface, seat and event serial, distinct from the target surface. Accept a
capture only while that input seat still focuses the live source and the event
is no older than two seconds. The input slot holds weak window references and
never keeps ordinary windows alive. The compositor retains final focus policy.

Queue one captured request per target before retiring windows. An admitted token
request owns the source role through its asynchronous reply or one-second expiry,
so a closing inspector can legitimately request focus for its parent. Reject
closed/dead targets, cancel on target retirement, and ignore obsolete replies.
Keep at most one pending token per target and64 per connection, evicting oldest
requests if needed. Expiry, replacement, reply and target closure release token,
timer and source ownership. Unsupported activation or ineligible input has no
effect. Background operation updates must not request focus.

The pending timestamp and timer share one deadline. The reply handler checks
elapsed time directly before activation: an event-loop stall can otherwise
deliver a reply before an overdue timer callback. The native expiry fixture
delays the real requesting surface's reply by1.2seconds inside a private
test-only diagnostic shim, then checks source cleanup and unchanged focus.

The application requests parent focus before retiring the input source. Keep
this ordering explicit; changing ordering alone could not fix the old empty
backend call. Do not add Niri IPC commands or another GUI toolkit. Native Source
roles belong to this backend and are not filesystem or Clipboard leases.

Preserve upstream versions, attribution and archive hashes. Update the Winit
reviewable patch and provenance with the intended source changes only. Validate
against the exact published archive, the minimized native focus case, the original
multi-window transfer scenario, both themes and applicable shared regressions.
This decision does not close broader activation/FileManager1, physical input,
accessibility, performance, workspace/release or full PRD gates.

Focused native acceptance passes normal return and overdue-reply rejection in
dark/100% and light/125% (four cases). The original partial-transfer/Undo/multiple-
Browser workflow also passes in both themes with the result inspector. The
deadline correction has a genuine native red and both independent review axes
are clear. See .scratch/file-manager/iced-focus-verification-01.json. Full shared
regressions and the broader gates above remain required.
