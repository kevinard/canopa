# Canopa XDG portals

Status: ready-for-agent

Started: 2026-10-29. Implementation in progress.

Provide Canopa-owned portal backends in Rust, with Iced and shared appearance
for interactive surfaces. Avoid GTK. Begin with FileChooser, reusing Files'
renderer-independent browsing. Extend the existing Settings service.

Deliver OpenFile, SaveFile, SaveFiles, byte-safe paths, filters, application
choices, parent/modal hints, cancellation and concurrent request lifetimes.
Keep sandbox/document responsibilities in the public XDG frontend. Ship
activation/selection metadata, verification and explicit Niri setup instructions.
Require focused protocol/model and private native public-portal checks. Shared
stack changes retain one vendor copy and reviewable provenance. Leave the live
desktop and current portal selection untouched during development.

The broader goal continues beyond FileChooser through the
[implementation map](ROADMAP.md). AppChooser is the default next interface
pending the user's priority preference, followed by shared permission surfaces
and capability-backed desktop integration. ScreenCast/RemoteDesktop need distinct
compositor/PipeWire design. Never route unimplemented interfaces to Canopa,
advertise stubs as replacements, or remove working fallback backends prematurely.
