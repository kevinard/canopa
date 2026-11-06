# Background portal preparation

Status: implemented with scoped acceptance; advertised in source packaging,
2026-10-29. The Rust observer, request service, Notify questions and user-opened
Iced review are implemented. Seven native presentation checks pass per appearance
and four real frontend acceptance groups pass with synthetic sandbox instances.
Common dialog regressions and shipped-descriptor acceptance pass. All shared
stages have been attempted/continued, with the same ten Files integration failures
recorded separately; the accessibility fixture compatibility failures are repaired.
The investigation below records the original requirements and source evidence.

## Backend contract

The [official backend reference](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.impl.portal.Background.html)
and installed interface XML define three methods and one change signal.
`GetAppState` returns an app-ID dictionary with uint32 values: 0 background,
1 has windows, 2 active. `RunningApplicationsChanged` asks the frontend to query
again. `NotifyBackground(handle, app_id, name)` returns a results dictionary whose
uint32 `result` means forbid (0), allow (1) or allow this instance (2); its separate
numeric response is unused. `EnableAutostart` is deprecated and current frontends
do not call it. Check the installed frontend's compatibility requirements before
choosing the implementation's legacy-method behavior.

The [current GNOME backend](https://github.com/GNOME/xdg-desktop-portal-gnome/blob/main/src/background.c)
uses GNOME Shell introspection, groups windows by resolved sandboxed app ID and
aggregates active/running state. Its current notification method returns allow
without presenting a question. That is reference behavior, not Canopa's consent
design or a reason to add GTK.

## Frontend ownership

The [current frontend](https://github.com/flatpak/xdg-desktop-portal/blob/main/desktop-portal/background.c)
combines compositor state with Flatpak instance discovery and permission storage.
Missing app entries are interpreted as background. Failed state queries stop
that observation pass. A forbid decision can cause the frontend to terminate the
tracked instance; allow may update its stored permission, while instance-only
allow leaves permission unchanged. Its RequestBackground path uses the Access
backend when a question is needed. Autostart file validation/publication is now
in the frontend. Keep these policy/process/file responsibilities there rather
than duplicating them in a backend UI.
The inspected frontend defaults a missing notification result to instance-only
allow. Cancellation must preserve that non-persistent, non-destructive behavior.

## Niri observation seam

[Niri's IPC types](https://github.com/YaLTeR/niri/blob/main/niri-ipc/src/lib.rs)
provide an initial state snapshot followed by events on an EventStream connection.
Windows include declared app ID, optional Wayland-connection PID and focus state.
Separate requests can observe different moments. The locked `niri-ipc =26.4.0`
source has these fields too. Canopa Shell already has a reconnecting subscription
and window cache in `src/niri/`; its UI fallback to the last active window is a
presentation convenience, not evidence that an application currently has focus.

Design questions to resolve before advertising:

- Observe a consistent initial window snapshot and its subsequent changes.
  Define bounded input lines/window counts, disconnection, reconnect, unknown
  events and revision retirement. An unavailable or incomplete snapshot must
  fail explicitly; a fabricated empty success could change frontend policy.
- Decide how a window is associated with a sandbox's authoritative application
  identity. A client-declared app ID is insufficient evidence by itself. Inspect
  what Niri's PID represents, available process/sandbox metadata, process exit
  and PID reuse. Do not label an unresolvable live window as a known background
  app. Determine a safe failure strategy for incomplete identity observations.
- Preserve compositor/session ownership while avoiding a dependency on the
  Shell UI or sharing its application lifetime. A small renderer-independent
  observer may be appropriate if both consumers need the same contract; do not
  extract the existing Iced subscription merely to reuse a loop.
- Reuse Requests for the notification question's admission/owner/Close lifetime.
  Use an explicit Iced decision with distinct persistent allow, instance-only
  allow and forbid wording. Cancellation and caller loss must not accidentally
  become a persistent allow or a destructive forbid. Consider Notify-owned Iced
  notification actions for this automatically triggered question, so observing a
  background application does not steal focus with a new toplevel.

## Required evidence

First build deterministic observation tests: initial snapshot, focused and
unfocused windows, multiple windows for one app, last-window close, app-ID/PID
changes, unknown/malformed input, reconnect and absent compositor. Add identity
fixtures for valid sandbox metadata, unsupported identity and PID reuse.

Wire tests must verify uint32 dictionary values and decision results, change
signals, request ownership/caller loss, concurrent independent decisions and
shared admission. Native tests should create harmless fixture windows, compare
real Niri transitions and display all Iced decisions in both appearances.
Public acceptance must use a controlled sandbox instance and fixture permission
store, with no real application termination. The absence of an actual Flatpak
environment remains an explicit acceptance gap. Advertise only the functionality
whose compositor and frontend paths have been exercised.

## Native observation capability verified

`.scratch/xdg-portals/probe-background-niri.py` passes five private native checks
in `/tmp/canopa-background-niri-02` (initial successful run retained as `-01`).
The result records `/usr/bin/niri`, version `26.04 (8ed0da4)` and SHA-256
`f09e0b1cb2144794fb615c90e9cdaff6adc0cb6b05347503123bc392aeea66d3`.
Two fixture Wayland clients expose their exact launched PIDs and distinct window
IDs. Focus events follow explicit changes, closing one window retains its
sibling, closing the final window removes it, and reconnect starts with a fresh
complete snapshot. The journal contains fixture window events only.

The retained pinned source corroborates the PID seam:
[IPC window construction](https://github.com/YaLTeR/niri/blob/8ed0da44d974c32c6877d2f4630c314da0717ecb/src/ipc/server.rs)
reads credentials from the mapped surface; the
[credential helper](https://github.com/YaLTeR/niri/blob/8ed0da44d974c32c6877d2f4630c314da0717ecb/src/utils/mod.rs)
uses client credentials and can deliberately return None for unknown credentials.
It does not establish a resolved sandbox application ID. The initial probe proves
this compositor capability only. The subsequent Rust observer fixture verifies
seven checks against actual Niri and bubblewrap roots whose metadata identity
deliberately differs from their Wayland label. The implementation and its
remaining limits are in
[ADR0034](../../docs/adr/0034-background-observation-and-decisions.md), with exact
validation outcomes in [the ledger](IMPLEMENTATION.md).
