# ADR0028: Iced application chooser and activation handoff

Status: accepted; shared validation in progress, 2026-10-29.

## Context

The FileChooser request host and ordinary Iced windows provide the first desktop
portal surface. AppChooser adds live choice updates and an outgoing activation
token. Shell already owns desktop metadata discovery; ADR0027 moves that model
into `canopa-apps` without moving Shell's launch policy into a dialog backend.

## Decision

Implement `org.freedesktop.impl.portal.AppChooser` in Rust alongside Settings and
FileChooser. Share request capacity, owner authentication, Close and caller-loss
cancellation. Keep active choices authoritative in the backend; serialize updates
with result validation. Helpers receive bounded newline-delimited JSON updates
through an owned pipe. Closing a request drops its future and kills its helper,
even if a presenter does not cooperate with cancellation.

The Iced helper presents exact offered application IDs using catalog names,
search and bounded PNG/SVG icons prepared on workers. It returns a decision and
never executes desktop entries or changes MIME defaults. The XDG frontend retains
launch, document-export and permission policy.

Use the incoming activation token during native window creation. On acceptance,
ask the shared ordinary-window adapter for a fresh outgoing token. The Wayland
operation captures recent eligible input from this exact focused source window,
sets the input seat/serial and surface, and delivers the compositor's token by
request serial. Retain the chooser while waiting, reject replies older than one
second, and finish without a token if the protocol/input is unavailable or the
reply deadline expires. Choice changes invalidate pending acceptance. Never
recycle the incoming token after user interaction. Layer surfaces decline this
ordinary-window operation; the existing generic Winit startup-token API keeps
its upstream behavior.

## Validation and limits

The evidence ledger under `.scratch/xdg-portals/IMPLEMENTATION.md` records actual
wire, model, native and shared gate results. The shipped descriptor now includes
AppChooser after seven private native checks passed in dark/100% and light/125%. Fixtures must expose only disposable desktop entries and a harmless
recording application, using private D-Bus, Weston and Niri instances. Native
acceptance must prove that the chosen application receives the URI and fresh
token, consumes it and receives focus. Optional xdg-dialog support remains
compositor dependent as documented for FileChooser.
