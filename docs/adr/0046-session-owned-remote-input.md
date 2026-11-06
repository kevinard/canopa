# Session-owned RemoteDesktop input

Status: Rust keyboard EIS and legacy Notify, shared sessions and Iced
consent/controls are implemented and pass scoped native checks. The pinned
frontend companion also verifies concurrent keyboard forwarding. Pointer/touch,
physical-input, full-matrix and distribution acceptance remain, 2026-10-31.
RemoteDesktop is not advertised.

A separate, unadopted compositor pointer foundation passes nine model tests and
38 private native checks on one frozen binary. It tracks actual device ownership
before bindings, preserves overlapping holds, cancels source-owned actions and
grabs, and separates host focus loss from global input resets. Reviewable delta
patches and the failed controls are recorded in
[the pointer ledger](../../.scratch/xdg-portals/POINTER-OWNERSHIP.md). This does not
add a compositor pointer lease, approved coordinate mapping, EI/legacy pointer
transport or Iced pointer approval. Those and broader acceptance remain before
canonical adoption/advertisement; the existing recipe and runtime remain unchanged.

The follow-up prepared pointer lease adds finite relative/button/scroll commands,
shared keyboard/pointer admission and source cleanup before socket shutdown.
Six wire tests and 94 native checks pass, including direct lease binding/DnD,
the earlier cleanup groups and both existing keyboard suites. See
[the lease draft](../../.scratch/xdg-portals/pointer-lease-draft/README.md).
Its private capability mask excludes absolute/touch. Approved live source mapping,
EI/legacy pointer transport, Iced approval and broader gates remain; this slice
does not change the canonical recipe, portal worker grants or advertised interfaces.

RemoteDesktop combines input with optional ScreenCast sources. Consent, ongoing
controls, capture and input share one active session lifetime. ScreenCast source
selection must recognize RemoteDesktop-created paths, whose Start/Close belong
to RemoteDesktop. Completed requests do not own active resources. Authenticate
the frontend caller and application before returning any EIS sender connection.

Use Rust reis0.7.1 for EI/EIS. Preserve upstream read behavior and add read_chunk:
at most2048bytes per call, at most1MiB pending bytes, no incoming client FDs, and
distinct EOF/WouldBlock. Drain pending requests between chunks; unread bytes may
remain on the socket. An error retires the connection. The canonical root fork
retains its MIT notice, exact archive/commit, per-file hashes and two-file patch.
Portal's dependency is desktop-only. Four socket regressions and staged real
libei interoperability prove this primitive, not a complete input worker.

The worker separately bounds requests buffered until a device frame, sender role
and granted classes. A frame from one device cannot clear another device's pending
work. Retiring a device removes its interface identities. The adopted per-device
guard passes six typed-protocol tests; a real libei flood is rejected before any
native input. A shared parent connector limits eight active or retiring children
across its sessions and retains admission until actual process exit.

The private child receives a bounded configuration and parent lease on stdin,
and transfers exactly one EI FD over its anonymous stdout socket. It subscribes
to backend/caller/compositor loss before resolving the original unique owner,
checks the compositor's bus PID against the IPC peer, and never activates a
replacement. Only approved keyboard sender contexts are currently accepted.
Frames and their serials/transitions are validated before forwarding. Keymaps
are sealed. Output backpressure retires the connection rather than queuing work.
Normal cleanup waits for the compositor's source-release EOF acknowledgement.

StopEmulating releases and renews the compositor lease while retaining the EI
connection. Device closure permits a subsequent device request. Retired IDs have
a finite tombstone set; queued old messages cannot route to a replacement.
Twelve private libei/native checks pass, including repeated periods, re-creation,
caller/parent/client/worker loss, role/grant/peer rejection and keymap retirement.
This is a keyboard worker, not the public RemoteDesktop backend or consent flow.

The parent resolves an authenticated endpoint, receives exactly one CLOEXEC Unix
stream and verifies that its peer is the spawned worker. It transfers the EI FD
to its recipient and retains only the control lease. Pending-start cancellation,
active Drop and explicit Stop close the pipe; a detached reaper owns cleanup and
admission. Five process/FD regressions and six real parent/worker/libei groups
pass, including actual parent SIGKILL. The production sharing integration below
uses this same connector and admission owner.

ScreenCast and the unadvertised RemoteDesktop adapter now share one session
registry, sixteen-session admission limit, endpoint and cleanup actor. A session's
creating interface fixes which interface may Start it. Device and source selection
remain independent before Start; both keep the original app and caller binding.
ConnectToEIS transfers a descriptor only from an approved active control resource.
Its finite command queue and pending connection observe session, caller and source
loss. Shutdown retires both session kinds through the same owner.

Five private-bus contracts cover combined selection in either order, exact EI FD
transfer, pending consent/connection cancellation, invalid grants, common admission
and caller/source/backend loss. They pass alongside the existing capture contracts.
The first execution caught zbus's automatic ConnectToEis spelling; the adapter
explicitly exports the required ConnectToEIS name.

The production presenter now shares the existing capture factory and recorder
budget. It presents one Iced choice with independently approved sources and
keyboard control, initially unchecked. Input-only approval requires an explicit
keyboard grant; a combined request can be reduced to view-only sharing. The
presenter validates the returned sources against its original catalog and pins
the authenticated compositor identity across consent. A passive owner observer
also retires approval or an active session before its first input connection.
One ongoing Iced control surface describes the granted scope and stops all
related resources. No worker is started until an approved ConnectToEIS request.

Six private input-only native groups pass in each appearance with real Iced
consent/controls, a libei sender, a harmless receiver and the pinned keyboard
compositor. They establish initially unchecked approval, request cancellation,
French-layout modifier feedback, UI Stop, session Close, client/caller loss and
backend SIGKILL cleanup. The light run uses the rebuilt wording/config harness;
the earlier dark run's exact sources and binaries remain separately recorded.

Six combined-session groups also pass in dark/100% and light/125% through the
standard XDG frontend with a private opt-in descriptor. They verify both selection
orders, view-only reduction denying input, real asymmetric monitor pixels through
the restricted PipeWire FD and serial, concurrent French-layout EI input, and
common producer/input/controls retirement on each active shutdown path. Exact
fixture sources are archived before execution. These are monitor/keyboard proofs;
legacy Notify methods, pointer/touch, physical-input and compatible compositor
distribution acceptance remain. The main service and shipped descriptor do not
register or advertise RemoteDesktop yet.

Use a compositor-owned input lease for authoritative keyboard state. Niri owns
the keyboard source and held keys; the worker owns the lease FD. FD loss releases
that source without running release-triggered bindings or releasing another
source's overlapping keys. The portal still owns authorization and consent.
A parent lease revokes the worker after backend loss; caller/session/controls/
compositor loss retires every related resource. Pin the compositor's unique bus
owner to its authenticated process before obtaining an input lease. Preserve
source identity and coordinate maps for absolute input. Freeze and bound the
keymap exposed to EI clients.

The separate Niri candidate returns a configured keymap and authoritative initial
modifier masks while the client is unfocused. Thirteen private native checks
establish French/US layouts, Caps Lock, asynchronous modifier updates, overlapping
held keys, idle keymap-change retirement, FD/worker-loss cleanup, malformed input
retirement and finite admission. A cached content identity avoids serializing the
keymap on ordinary compositor refresh. Slow modifier readers are retired instead
of accumulating an application queue. These checks establish the original evdev
transport. The typed symbol extension below adds layout-aware legacy semantics;
pointer/touch capability handling and physical-input acceptance remain open. Its
reviewable patch is retained separately from the accepted capture/drag recipe;
the explicit niri-remote-input recipe is now registered with archive-verified
added-file support. Existing capture/drag recipes are independent.

The same explicit recipe now adds OpenKeyboardSymbols/SymbolVersion1 without
changing the original evdev byte format. Typed commands carry a sequence, input
class, value and state; replies acknowledge the command and authoritative
modifiers. Sequence0 is unsolicited feedback. Logical refusal preserves existing
held keys, while malformed frames or blocked feedback retire the source. The
compositor resolves symbols against its current keymap, Caps and group while
holding the input-thread XKB guard. The planner retains numeric chords, sharing
modifier references until the last owning symbol releases them. No map reference
escapes the guard or moves across threads. Both protocols share admission, held-key
and dispatch limits.

Sixteen planner/wire tests, twelve typed native groups, thirteen existing evdev
groups and six combined frontend/media/EIS groups pass on the typed compositor.
The portal's optional legacy worker mode forwards one fixed-size pending command
and its acknowledgement, retaining the existing authenticated endpoint and reaped
worker pool. Default EIS bootstrap remains compatible with older workers. Five
real legacy parent/worker groups and six existing EIS groups pass on one frozen
new Portal build, including held-key cleanup after parent and worker SIGKILL.
These establish the internal transport. Public keyboard Notify dispatch, the
session's input-mode guard and production control are implemented; eleven Cargo
private-bus contracts pass. Notify uses a lazy authenticated legacy worker, and
ConnectToEIS stops that worker before transferring an EI FD. Notify after EIS is
denied. Overflow revokes the common grant so a discarded release cannot leave a
held key active. Each of the sixteen sessions permits at most 64 pending commands.
The earlier four-command bound retired an ordinary eight-event shortcut because
the standard frontend forwards Notify before backend acknowledgements complete;
both a native replay and a delayed-acknowledgement private-bus regression reproduce
that failure. A separate four-thread regression reproduced reordering before
commands reached the actor. The RemoteDesktop dispatcher now admits keyboard
commands synchronously in D-Bus receive order, then waits for acknowledgements in
detached, bounded reply tasks. Session setup retains its asynchronous typed
handlers. Eleven private-bus contracts pass, including 1,024 ordered transitions
and input while another session awaits consent.

The installed standard frontend 1.22.1-2 also reorders concurrently issued public
Notify calls before forwarding them. A private-bus trace records ordered public
messages, shuffled backend messages and matching legal receiver transitions;
the backend cannot reconstruct ordering information absent from the forwards.
The pinned frontend companion in [ADR0047](0047-ordered-portal-input-forwarding.md)
fixes that scheduling boundary with bounded receive-ordered admission and one
worker per authorized session. Fourteen native groups pass per appearance and
under address/undefined sanitizers, including concurrent and NoReplyExpected
input. The installed frontend remains unchanged; distribution acceptance remains
a release gate. Earlier reply-barrier tests retain their separate scope.
Nine native public reply-barrier groups pass in dark/100% and light/125%, including
French symbols/shared modifiers, view-only denial, real monitor frames, worker and
backend loss, and legacy-to-EIS handoff on the original approved stream. All
eighteen owned input workers are terminal. These establish their stated keyboard
scope; pointer/touch, physical-input and distribution acceptance remain. No main
registration or advertisement is added by this slice.

Pinned Smithay releases tracked keys/buttons when virtual devices disappear.
Niri's shared-keymap modifier masks have separate ownership limits; zeroing them
does not preserve local input. End-to-end worker acceptance must cover actual key
transitions, non-US layouts, modifiers, normal Stop, client loss and worker/backend
crash cleanup. The standalone keyboard lease checks above do not establish that
whole session lifetime. Touch support is not inferred from keyboard/pointer support.

Consent and ongoing controls use Iced/shared appearance and state the scope of
desktop input control. Restore data grants nothing without fresh consent.
Clipboard is a separate session grant. Keep current routing until Rust input,
media/FD lifetime and compatible compositor distribution are accepted. Development
and acceptance use the private desktop and harmless receivers.

See [RemoteDesktop evidence](../../.scratch/xdg-portals/REMOTE-DESKTOP.md),
[fork maintenance](../vendor-maintenance.md), and
[ScreenCast ownership](0042-screencast-sessions-and-consent.md).
