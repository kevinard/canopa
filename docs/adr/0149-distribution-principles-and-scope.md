# 0149. Distribution principles and scope

Status: accepted

## Context

Canopa is packaged as a complete desktop session (Arch packages with a pinned
compositor and portal frontend). Each new capability raises the same questions:
who owns the state, whether to write a new program, which upstream to build on,
and what the packages install by default.

## Decision

- **System services own state; Canopa owns presentation.** NetworkManager,
  BlueZ, UPower, logind, PAM, polkit, GnuPG, greetd and the audio server keep
  their policy and data. Canopa provides the interface and talks to them through
  their public APIs (`canopa-services`).
- **Order of preference for a new capability:** an existing service with a
  Canopa frontend; then an existing Rust application; then an existing non-Rust
  application; and only then a new Canopa tool.
- **Coherence comes from portals and theme bridges**, not from replacing
  applications: third-party applications follow Canopa's appearance through the
  Settings portal and use Canopa's dialogs through the other portals.
- **Stock upstreams by default, forks where needed.** Patched upstreams are
  tracked GitHub forks with one PR per change, aimed at upstreaming
  (`docs/vendor-maintenance.md`).
- **Test what ships.** Acceptance runs against the compositor and frontend the
  packages install.
- **No default application set.** Canopa ships its own components but does not
  pick a browser, terminal, editor or media player; users choose.
- **Experimental interfaces stay opt-in until accepted.** Print, InputCapture,
  GlobalShortcuts and Usb are not advertised by the packaged session.

## Consequences

Features that would need Canopa to own system state (for example its own network
daemon) are out of scope; Settings pages present the owner's state instead.
Moving an interface from opt-in to default needs its acceptance on real hardware
first (see the `verification` issues).
