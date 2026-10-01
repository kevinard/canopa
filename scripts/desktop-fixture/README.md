# Disposable desktop input fixture

`click.c` injects a pointer click into the private compositor used by
`scripts/test-desktop.py`, with coordinates normalized to its 1600×1000 output.
It is test-only code under the project license. The runner compiles its Wayland
bindings into the artifact directory; generated code is not checked in.

The Python fixture creates one shared Canopa configuration, checks banner and Center
native actions, sends Tab/Escape through `wtype`, and checks both consumers' palette
pixels. A Notify reload switches the visible banner palette while the shell keeps
its startup palette. Run `--color-scheme dark` and `--color-scheme light --scale 1.25`
with separate new artifact directories. Screenshots and `result.json` are retained.

`wlr-virtual-pointer-unstable-v1.xml` is copied without modification from
`wayland-protocols-wlr` 0.3.12's bundled wlr-protocols source (Wayland RS revision
`a3d7927d87799b2955bf491b51c7c2a3a82da661`). Its embedded MIT notice credits
Josef Gajdusek, 2019. The upstream protocol lives in
[wlr-protocols](https://gitlab.freedesktop.org/wlroots/wlr-protocols/-/blob/master/unstable/wlr-virtual-pointer-unstable-v1.xml).

## Acceptance matrix and shared session

`acceptance.json` is the native acceptance inventory. The desktop workflow runs it
through `scripts/run-desktop-acceptance.py`; `bash scripts/check.sh` validates its
coverage without starting a compositor. Adding a `verify-*.py` fixture requires a
manifest entry. The matrix includes both appearances, fractional placement, fork
lifetimes, drawers, transitions and close flashes, scrolling, content rotation,
focused icons, and idle inhibition on both Sway and Niri.

List cases with `python3 scripts/run-desktop-acceptance.py --list`. Run all cases
with `--output /tmp/canopa-desktop-artifacts`, or repeat `--case drawer-modes
--case panel-motion` to select cases. Use a new output directory. The runner retains
per-case logs, each fixture's screenshots/results, and a summary even on failure.
A failed case does not hide results from later cases. The dedicated self-hosted
runner remains manually dispatched; native dependencies are checked before builds.

The existing `Session` in `validate-desktop.py` owns private bus and process groups,
Weston/Niri startup, IPC/display/output readiness and observable scale application.
Its private bus has no installed service activation directories. `build_pointer`
compiles the shared input fixtures; `capture` obtains a synchronous Grim frame and
`screenshot` waits for a fresh Niri image. Scenario files retain their own compositor
configuration and assertions. Drawer, motion, placement and measurement fixtures
use this startup boundary and close the Session in `finally`.
