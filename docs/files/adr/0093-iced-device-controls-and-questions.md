# 0093: Iced device controls and question windows

Status: scoped Mount/Unmount, Power off/Eject and Iced workflow verified,
2026-10-19. Extends
ADR0074/0076/0091. The full device and product acceptance gates remain open.

## Ownership and presentation

Extend the existing sidebar and narrow Locations menu with the prepared local
device catalog. Keep the same Open behavior at either width and preserve normal
parent navigation. DeviceRequests.open owns the current-root versus mount
decision. It returns a prepared location or an admitted request ID; Iced never
assembles platform calls. Removal uses the current token and platform capability
label: Power off, Eject or Unmount. No force option is introduced.

DeviceId is a parent-owned row identity while a device stays continuously
observed. Disappearance and reappearance mint a new identity even when native
objects are reused. It is separate from DeviceToken, which binds request intent
to the current catalog revision. Labels, paths, row identities and UUIDs never
become request targets. Menu controls keep their captured tokens; later changes
refuse stale activation instead of retargeting it.

The existing frontend volume receiver updates device presentation alongside
filesystem operations. The backend attention host retains its own receiver.
There is no additional observer, GUI-driven initialization or filesystem work in
the renderer. Shared Operations shows bounded descriptive requests, cancellation,
questions, results and notification errors. Finished rows omit unavailable
controls. Explicit Open folder revalidates the completed mount through the
backend; a saved root path never grants current navigation.

## Browser intent

An Open invocation captures its existing Window, active Tab, scan/navigation
revision, search serial, location draft and latest device intent. A completed
mount navigates only when those still match. It never recreates a closed origin,
redirects another tab or steals focus from another window. New navigation, an
edited location or a newer device invocation makes the old result descriptive.

This deliberately follows the existing conservative source-context checks: a
refresh revision can also invalidate automatic opening. The result remains
explicitly openable while its mount proof is current. Broader browser-session
workflow extraction remains part of ADR0074; this decision does not turn an
observed root into atomic filesystem or restart execution authority.

## Questions and credentials

Device questions open in a floating transient window above the focused origin
or Operations. Otherwise backend attention requests review without opening a
window. The dialog owns presentation drafts only; the accepted request and its
question token stay in the backend. Password fields use the backend's semantic
requirements for username, domain, password, anonymous access and TCRYPT options.
Passwords are masked and credentials are never offered for saving.

Edited text has a redacted Debug representation. DeviceAnswer does not enter
GUI messages, persisted preferences, history or public snapshots. Submission
moves the password draft into the private answer. Anonymous normalization lives
in the backend and clears username, domain and password before replying to GIO,
including when another frontend supplied those drafts.

Every edit, toggle, Submit and provider choice carries the token from the form
that rendered it. A replacement question drops old drafts and resets focus to
Cancel; queued old controls cannot answer the new question. The shared Cancel
represents an identically labelled provider Cancel option, avoiding duplicate
buttons. All other provider actions retain their original indices and labels.
No choice or credential is supplied implicitly.

The question window's explicit Cancel, close request or Escape cancels the
device request, retaining backend ownership through actual native teardown.
Disposal notifications and closure of its originating Browser or Operations
window only dismiss presentation. They never answer or cancel accepted work.
Opening Operations can review that same pending question after every window
has closed. Native focus ordering follows root ADR0021.

## Verification and limits

Default validation passes 77 library tests and the public device-runtime
program's nine check groups, all-target Clippy with warnings denied, the build
and workspace formatting. The runtime program exercises backend Open for both a
prepared root and an unmounted device, stale refusal, and fresh row identity
after actual private-provider unplug/replug. A real MountOperation test checks
anonymous credential removal before its reply. Controller tests reject mount
completion for a changed, different or closed tab/window and an edited location.

Eighteen native checks cover the Iced device workflow in dark and light at 125%:
sidebar and narrow Locations, Left navigation, real private GIO Unmount, floating
Cancel defaults, keyboard credentials, newer navigation and closed-tab results,
windowless questions and replacement-question focus. Forty-four selected native
regressions cover existing actions, helpers, catalog observation, headless
attention and resident reopening. The attention case builds a separate
local-runtime test executable without Iced; the other cases use the same app.
Representative captures were inspected, including masked input, the single
Cancel action and a question reopened from Operations.

The first light attempt clicked the wrong row because the fixture assumed the
dark output's logical dimensions. The fixture now reads Niri's actual dimensions.
One later outer runner exited143 after completed cases, without an aggregate
receipt. Remaining lifecycle cases and an isolated action case completed in
separate runs; all eight selected scenarios have passing final evidence. This
does not establish a full native-matrix run or explain that outer termination.

This stage received primary review only. Physical devices, expanded
busy-question/helper-loss/credential variants, accessibility,
performance and the full product/release gates remain open. See
[the verification](../../../.scratch/file-manager/iced-device-ui-verification.json)
and [the work log](../../../.scratch/file-manager/ICED-DEVICES-WIP.md).

## Power off and Eject extension

The private GIO provider now implements GDrive in addition to its existing
GVolume/GMount. Marker-selected drive capabilities exercise the actual helper's
Power off (`stop`) and Eject dispatch, including the preference for Stop when
Eject and Unmount are also available. The default provider remains unchanged.
No production execution interface or application code changed in this extension.

Twelve headless checks cover capability refusal before dispatch, actual busy
errors, cancellation through a delayed callback, unplug/replug and stale
observation refusal, disappearance before success, and owner EOF awaiting the
exact callback. Both operations use no-force flags and leave the private
directory and original file bytes intact.

Sixteen Iced checks cover both drive capabilities in dark and light at 125%:
wide Open, narrow Locations/Left, delayed removal after every window closes,
notification delivery, same-owner reopening and inspectable Operations results,
followed by orderly Quit. The fixture makes the existing owner resident before
closing its windows; provider initialization occurs exactly once. Captures of
the capability labels and completed result rows were inspected.

The existing Mount/Unmount/question workflows pass another eighteen checks in
both appearances; the existing helper/catalog cases pass fifteen. These total
61 selected checks across nine scenarios. One outer runner was terminated with status 143
after its four new UI cases passed, without an aggregate receipt. The unfinished
regressions completed in separate terminal-zero runs. The termination's cause
remains unknown; this is not a full native-matrix result or a termination fix.
Application Rust/Cargo inputs and the preserved executable are unchanged from
the previous validated checkpoint. The maintained inventory now has 160 cases.
This extension received primary review only. See
[the extension evidence](../../../.scratch/file-manager/device-drives-verification.json).
