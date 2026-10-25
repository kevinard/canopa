# 0019: Open Iced transient toplevels before initial configuration

Status: accepted under delegated implementation authority, 2026-10-18.
Implementation compiles. Private Files native acceptance passes in dark at 100%
and light at 125%; full shared-stack regressions remain required.

Files needs floating Quick Look attached to its originating Browser Window.
Iced 0.14 exposes ordinary windows but no parent relationship. Winit 0.30.13's
cross-platform parent-window attribute is unsupported on Wayland.

Add one focused operation to the shared Iced runtime: `open_transient(parent,
settings)`. It allocates an ordinary window ID and reports an optional opened ID.
A missing parent or unsupported backend returns None without showing an unrelated
ordinary window. The layer-shell adapter explicitly declines this operation.

The Iced Winit adapter captures an owned native parent reference and passes it
through a Wayland-specific Winit attribute. Winit validates the parent's backend
and connection and sets the SCTK window's parent before its first commit and
initial configure. Parent lifetime is retained through creation; Files owns the
subsequent source/preview lifecycle. A newly created child cannot form a parent
cycle. This uses typed window/protocol objects and adds no unsafe pointer bridge.

Setting the parent only after Winit window construction is too late for sizing.
A real Niri diagnostic run floated the child but retained the initial tiled
1280 × 800 dimensions instead of the requested 760 × 600. The earlier relationship
must precede initial configuration, not merely the first rendered buffer. Native
acceptance checks actual independent dimensions as well as floating status.

The [Wayland parent contract](https://wayland.app/protocols/xdg-shell#xdg_toplevel:request:set_parent)
provides auxiliary-window stacking. [Niri's default policy](https://github.com/niri-wm/niri/wiki/Floating-Windows)
floats new windows with a parent. Users retain compositor policy; Canopa needs
neither an application-specific floating rule nor Niri IPC for this relationship.
Actual dimensions, source ownership, focus and lifecycle require private native
acceptance.

Maintain one copy of each shared fork in root vendor: iced_runtime and iced_winit
0.14.0 from upstream revision `3997291f318a8bc06fa522f5579836fb3feb94df`, and Winit
0.30.13 from e9809ef54b18499bb4f2cac945719ecc2a61061b. Verify the exact locked
published archive checksums, preserve original MIT/Apache notices, and record
file hashes and reviewable patches. This extends the shared stack required by
ADR 0018. Shell's layer surfaces retain their ownership and lifetime. Full
workspace, fork and native regressions remain required before claiming the
shared change verified.

The final untraced Files runs 20/21 each pass 15 checks with actual 760 × 635
outer dimensions beside a 360-pixel browser, source navigation/focus, text copy,
rendered image zoom/Fit, distinct PDF pages, search/tab ownership and source close.
All 22 captures were inspected. Exact published archives, local inventories and
patches verify. See the [implementation checkpoint](../../.scratch/file-manager/ICED-PREVIEW-WIP.md)
for remaining shared regressions and the earlier diagnostic sizing failure.
Workspace all-target compilation and four fork library tests also pass. This
does not claim the full workspace or Shell/Notify native acceptance gates pass.
The complete core entry point passes with serial test execution and private
fixture sockets outside the sandbox; its earlier parallel audio-startup failures
remain recorded in the scoped verification evidence.
