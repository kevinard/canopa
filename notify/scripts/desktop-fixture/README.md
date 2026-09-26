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
