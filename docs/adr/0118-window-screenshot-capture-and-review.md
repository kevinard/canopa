# Window screenshot capture and review

Status: implemented; scoped public Iced/native acceptance verified.

## Decision

Extend Screenshot v3 with Window (2) and ActiveWindow (8). Preserve Screen (1),
Area (4), PickColor, frontend permissions/document export and the existing PNG
retention owner. A window image must come from the actual window producer;
cropping the desktop cannot provide an unobscured window.

Use the existing creator-owned Niri ScreenCast contract and linked PipeWire Rust
library. This adds no compositor fork, dependency or GNOME portal process. Never
invoke Niri's screenshot IPC, which changes the clipboard. Offer window targets
only when the capture owner supports creator cleanup and its process matches
both the configured Niri IPC endpoint and the exact Wayland display used by the
Iced helpers. Publish capability changes when that owner disappears or changes.
Without the compatible provider, AvailableTargets remains5.

Window uses the shared Iced source picker with single-window validation and
screenshot-specific wording. Revalidate the selected identity against the same
owner before recording. ActiveWindow resolves the focused identity before any
helper opens; no active window is an error, not a screen fallback. Explicit
targets cannot be changed. An implicit interactive screenshot may switch from
Screen/Area to Window; discard the screen snapshot before selecting/capturing
the window. Both window paths freeze their pixels before final Iced review.
Closing or changing the source after that point cannot change the accepted PNG.
For window targets, the live picker chooses an identity before capture. This
extends the Screen/Area ordering in ADR0039/0040; all subsequent pixel review
still uses the sealed immutable snapshot, and never a live output crop.

Capture exactly one raw frame through a temporary Recorder lease with hidden
cursor. Select the exact PipeWire object serial, and check its client's kernel
process identity in the consumer context against the pinned compositor PID.
Reject foreign producers and unrecognized formats. Accept one shared-memory
plane in BGRx/BGRA, checking dimensions, aggregate pixels, offsets and stride
before allocation or copying. Copy rows with pread rather than dereferencing a
producer-owned pixel mapping; truncation then produces an error rather than a
SIGBUS. Empty buffers and temporary format clearing wait for a valid frame.

The window path shares the two-snapshot admission owner. A retained task owns
admission until its frame reader exits and its producer is stopped, including
when the public request future is cancelled. Stop the producer before opening
final review. The helper receives only the existing sealed pixel descriptor and
bounded metadata; PNG encoding/publication and result paths remain backend-owned.

## Evidence

The phase95 old-backend control reports5 in both backend and frontend on the
compatible companion. A separate real producer probe confirms its secure client
PID. The exact-source frame harness rejects a foreign PID and reads full blue
pixels from a window on another workspace. It links existing locked workspace
artifacts without a dependency build; its source hash is recorded.

Seven actual public frontend/Iced groups pass in dark/100% and light/125%:
off-workspace full window pixels, implicit Window selection, picker cancellation,
frozen ActiveWindow review after source closure, missing-active failure, fixed
explicit Screen and capability withdrawal from both backend and frontend. The
helpers load no GTK and the clipboard marker remains unchanged. Full Portal
library468 and integration58 checks pass. The screenshots were visually inspected.

The initial native fixture assumed Tab/Enter could activate standard Iced buttons;
the current widget has no such keyboard behavior. Its pointer correction uses the
existing desktop click caller in one stable cache. A second fixture correction
fits dialogs and pointer coordinates to the nested output's actual logical size
at125%, rather than the requested parent size. No application change was needed
for either correction. Keyboard-only final button confirmation remains a UI limit.

The interrupted workspace no-run build was deliberately stopped after all Portal
artifacts and tests completed, avoiding restoration of unrelated outputs. This is
focused Portal acceptance, not a new whole-workspace or release gate; the phase94
Files timing failure remains recorded. Development bundle refresh is tracked in
the phase95 checkpoint; Print remains deferred/opt-in.

Contracts: [public Screenshot](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Screenshot.html)
and [backend Screenshot](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.impl.portal.Screenshot.html).
