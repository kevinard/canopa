# Compositor-owned shortcut leases

Status: canonical private foundation, 2026-11-01. The build, 53 actual-source
core tests and 50 scoped native groups pass. Required workspace validation has
completed with the same ten Files failures; all other Rust and post-Rust stages
pass. The Rust endpoint passes fifteen unit tests and three private native groups.
Public GlobalShortcuts v2 sessions and Iced grant/reconfiguration are now
experimental under ADR0067. Normal advertisement remains separate work.

The accessibility monitor can register dynamic grabs, but its signals omit
source event time and stable press ownership. Resolving a release against new
modifiers can lose that release. A broad keyboard watcher in the portal would
not recover trustworthy ownership or source time. The native baseline and the
source-cleanup correction are recorded separately in ADR0065.

The private org.canopa.Niri.GlobalShortcuts1 module issues one leased Unix socket
to a trusted desktop backend. The compositor owns admission, current-keymap
resolution, ordinary/a11y reservations and press recipients. The Rust
portal owns authenticated application sessions, saved choices and Iced consent.
An application receives directed XDG signals, never this privileged descriptor.

The module combines logical definitions, registration generations, captured
source/key cycles and socket lifetime. Replace requires the observed generation
and commits the full proposed set or preserves the previous set. List reports
the actual admitted set. Framed JSON, sequence validation, finite admission and
eight commands per dispatch bound control processing. A blocked writer retires
its own lease; it cannot stall input or retain an abandoned binding.

Input integration observes raw source edges before Smithay can absorb duplicate
presses or nonfinal releases. New capture follows normal/hardcoded and a11y
precedence, lock/inhibitor and compositor UI policy. Captured releases follow the
original recipient rather than current symbols or modifiers. The actual combined
seat state reconciles every input result, including filtered non-transitions.
RemoteInput's direct source teardown explicitly notifies the module after seat
cleanup. Each physical keyboard gets a fresh source identity, as virtual and
remote keyboards already do; device removal cannot end another holder.

Rebind, suspension and closure deactivate authority once while keeping held-key
suppression until actual seat-up. Losing backend focus does not prove a held key
is up: its real pressed state is retained during reset. Source event time is
converted from supplied u64 microseconds to u64 milliseconds, preserving the
backend's actual precision. Synthetic retirement uses the monotonic clock.
No activation token is fabricated.

Resolution uses layout-qualified base-key slots and the compositor's ASCII
fallback. Named XKB modifiers and a real borrowed probe state project conflicts
through the ordinary matcher. Modifier triggers include their own post-press
modifier. Keymap/config/a11y changes retire old generations before readmitting
the approved logical definitions and publishing the accepted subset. Identical
accessibility reservations leave active actions intact. Temporary conflicts may
withdraw live authority without erasing previously approved choices.

The canonical niri-global-shortcuts recipe retains the official Niri and Smithay
archives, licences and existing keyboard-source correction. Complete patches
remain relative to those archives. Exact old/new materializations verify the
same stable source paths and only two original Smithay dependency bindings.
Compilation reuses the pinned nightly, incremental dev graph, one job and existing
compositor cache. It does not install a compositor or change portal routing.

The [compositor ledger](../../.scratch/xdg-portals/global-shortcuts/COMPOSITOR.md)
records exact build, source, native and broader-check evidence. The initial
staged core passed 51 tests; the canonical core adds actual-source checks for
modifier triggers and bulk retirement. Physical-device, complete layout/lifetime,
public portal/Iced and distribution acceptance remain open.
