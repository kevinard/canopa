# ADR0031: Keep the Notification portal adapter inside Notify

Status: implemented with scoped wire/native acceptance; shared validation pending, 2026-10-29. Display hints, purposes, replies and the banner-hide timer added 2026-11-11 (Notify ADR 0055); backend-level checks only.

## Context

The Notification portal uses application-scoped string IDs and typed action
parameters. Notify already owns Delivery Policy, Notification Lifecycle, History,
Banners, Center, sound and input-bound activation. Those owners should serve
portal notifications through the existing Iced presentation.

A separate freedesktop bridge would receive action signals after commitment and
look up target parameters in its own mutable registration table. A replacement
could change that table before an older committed default-action signal arrived.
Notify's reserved default action key alone does not identify a revision's target.
A separate backend process would also need to reconcile its registrations across
its own restart while Notify's active registrations remained live.

Notify already provides a stronger seam. Receipt preparation and synchronous
registration run under its owned mutation coordinator before effects. Its
SenderEventPublisher freezes transport identity synchronously before accepting
ordered publication. Shared ADRs preserve ownership, and Notify ADR0030 documents
this commitment/cancellation boundary.

## Decision

Add a Notification portal adapter to the Notify service process. Register its
implementation interface at the standard portal object path and publish it through
a separate `canopa-notify.portal` descriptor/service alias. The existing dialog
backend remains responsible for Settings, FileChooser, AppChooser and Access.
Notification routing selects the Notify-owned descriptor explicitly.

Keep one authoritative sender publisher. Extend its transport-local dispatch to
freeze either a freedesktop sender registration or a portal binding, including
its action target data. Descriptor content becomes bounded owned pixels/sound bytes before receipt, so asynchronous consumers do not reopen transport-owned paths. Freeze the portal binding under the
same coordinator as receipt/action commitment. The queued publication must retain
the binding it observed even if a replacement commits before bus emission.
Activation data belongs to the matching queued action; it must not bleed into a
later invocation or another registration.

An adapter-owned bounded operation queue retains admitted add/remove operations
through caller cancellation. Resolve replacement identity and register accepted
content through the existing receipt hooks. A dropped replacement preserves the
old binding and its actions. Serialize removal with receipt operations. Sending
application exit does not withdraw a notification. The adapter shares Notify's
service lifetime and its existing stale-history/restart contract; it does not
introduce a second notification runtime or store.

Prepare bounded immutable descriptor-backed icon/sound resources before mutation
admission, using existing asset/notification mechanisms. Rasterized pixels and encoded sound bytes are owned by their asynchronous consumers; no consumer reopens a transport-owned temporary path.
Preserve typed targets, plain/markup body meaning and canonical App Identity.
The backend calls the standard org.freedesktop.Application activation interface for exported app.* actions and default application activation. The frontend retains permission policy and forwarding of non-exported action signals.
Advertise only the optional categories/button purposes actually supported.

## Verification

The implementation must cover v2 descriptors and the full parameter-array shape,
not only plain-text forwarding. Wire/model checks need app/ID isolation,
replacement/removal, dropped replacements, caller cancellation, retired IDs,
resource bounds and malformed descriptors. Hold publication while replacing a
notification to prove that the older committed action keeps its original target.
Test missing/failed activation separately from a later action's fresh token.

A private public-frontend fixture should use disposable application registrations
and recording actions. Native banner and Center input must prove default/named
callbacks and activation in both appearances, including application-exit survival.
No Notification portal is advertised until those integration checks pass.

See [Notification preparation](../../.scratch/xdg-portals/NOTIFICATION.md),
[Notify mutation commitment](../notify/adr/0030-mutation-commit-and-effects.md),
and [Notify's domain context](../notify/CONTEXT.md).


## Current boundary

Ordinary portal notifications use Notify's server-default display timeout as a
banner-hide timer (Notify ADR 0055): low, normal and high priority banners hide
after display, but the sender lifecycle stays active, so the registration,
callbacks and Center actions remain live until the user acts or dismisses, or the
application withdraws the notification. `urgent` banners stay on screen. Sending
application exit still does not withdraw a notification. (Until 2026-11-11 the
timeout expired the notification and retired its registration; before that the
first version requested no expiry and every banner stayed on screen.) Timed
`org.canopa.request-timeout-ms` requests keep their own withdrawal deadline and
no display timeout.
Priority low/normal/high/urgent maps to existing Notify low/normal/normal/critical
urgency. `display-hint` is honoured (`transient`, `tray`, `persistent`, both
lock-screen hints and `show-as-new`). SupportedOptions advertises the categories
and button purposes Notify presents: `im.received`, `alarm.ringing`,
`call.incoming`, and `im.reply-with-text` plus the call-control purposes, which
get default labels. Inline replies use a Notify-owned banner composer and the v2
response position. Full (sv)
resource shape is validated before conversion; descriptor content must be
WRITE/GROW/SHRINK sealed. Typed action targets exclude descriptors and retain
their original variant type.

The optional Canopa request expiry described in [ADR0034](0034-background-observation-and-decisions.md)
is a separate sender-withdrawal contract for temporary questions. It is explicitly
negotiated, bounded and transient, and does not change ordinary portal expiry or
Notify's display-based attention timers.

Earlier focused verification passed seven adapter tests, all 38 binary tests,
26 intake checks, six raster/SVG checks and 276 Notify integration cases. After
the generic handoff, the focused callback-replacement and captured-action tests
also pass. The private public-frontend/native fixture now has six checks per
appearance (dark/100% and light/125%) covering sender-exit survival, exported
typed activation, native token consumption, non-exported action parameters,
withdrawal and Center input through ADR0032. The descriptor and activation
metadata are verified on private buses in both appearances. Final full consumer,
shared and core validation remains in progress; this does not establish a green
workspace.
