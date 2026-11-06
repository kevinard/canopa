# UMockdev dependency plan for the canonical Usb frontend

Research only, 2026-11-01. No artifact was downloaded, extracted, built or
installed; no testbed, device or D-Bus service was opened. This note records
read-only local metadata and proposed future commands. The frontend target stays
`/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1`; preserve its
guarded sources and existing executable.

**Recommended route:** obtain a verified official Arch `umockdev 0.19.9-1`
x86_64 binary package and extract it to one private directory. It supplies the
library, preload interposer, typelib and CLI without a system installation or
Vala build. Use the existing Python GI/Gio interpreter for a standalone fixture;
pytest/dbusmock are needed only to run the upstream pytest harness.
[Official package](https://archlinux.org/packages/extra/x86_64/umockdev/),
[runtime sonames](https://archlinux.org/packages/extra/x86_64/umockdev/sonames/).

## Verified local metadata

Read on this Arch Linux host using `command -v`, exact-package `pacman -Q`,
`pkg-config`, Python distribution/module metadata, directory listings and
`readelf -d` only:

| Component | Observed locally | Consequence |
| --- | --- | --- |
| Python / GI / D-Bus Python | Python `3.14.8`; PyGObject `3.58.1`; dbus-python `1.5.0` | Existing GI interpreter can host a standalone Gio fixture. Compatibility has not been executed. |
| Introspection | gobject-introspection `1.86.0-2`; `g-ir-scanner`, `g-ir-compiler`; GLib/GObject/Gio/GioUnix/GUdev typelibs in `/usr/lib/girepository-1.0` | UMockdev's own library/typelib are the missing GI pieces. |
| C dependencies | GLib/Gio/GObject `2.90.1`; GUdev `238`; libudev `262`; libpcap `1.11.0`; glibc `2.44` | Required runtime library files exist, including `libpcap.so.1` and `libudev.so.1`. This is not an ELF compatibility acceptance result for a future artifact. |
| Build tools | Meson `1.12.1`, Ninja `1.13.2`, gcc `16.2.1`; cc/clang/make/pkg-config/autoconf/automake/libtool available | Most source-build prerequisites already exist. |
| Missing tools/modules | No `valac`, `umockdev-run`, `umockdev-wrapper`; no `umockdev-1.0.pc` or `UMockdev-1.0.typelib`; no pytest/dbusmock/xdist in default Python metadata | Source build needs Vala; standalone Gio avoids the missing Python harness packages. |
| Canonical executable | `/tmp/canopa-xdp-session-canonical-13/frozen/xdg-desktop-portal` has ELF NEEDED `libgudev-1.0.so.0` | GUdev was linked into this executable. No new XDP build is indicated merely to supply UMockdev. |

Filename searches found no UMockdev archive/source/binary/typelib or Vala package
in inspected `/tmp`, user `.cache`, `.local`, `Downloads`, repository scratch or
`/var/cache/pacman/pkg`. The pacman cache does contain the already installed
gobject-introspection packages. This is scoped discovery, not proof that no
artifact exists anywhere on the machine.

## Official versions and the generated-C question

Use upstream **0.19.9**: its release explicitly fixes libpcap 1.11 USB-header
compatibility, matching the local libpcap version. The official release asset is
[`umockdev-0.19.9.tar.xz`](https://github.com/martinpitt/umockdev/releases/download/0.19.9/umockdev-0.19.9.tar.xz),
490 KiB, with published SHA-256
`666e050f76eb61921e6d79e309c5833cef639de3ec1f9c35c74ebce1681f7c34`.
[Release](https://github.com/martinpitt/umockdev/releases/tag/0.19.9),
[asset metadata](https://github.com/martinpitt/umockdev/releases/expanded_assets/0.19.9).

**The supported 0.19.9 release build cannot bypass valac through a maintainer
mode or generated-C option.** Its Meson project unconditionally enables C and
Vala and obtains the Vala compiler; library/run targets name `.vala` inputs.
The sole custom option is `gtk_doc`. Release CI publishes `meson dist` output;
the dist hook stamps `.version`, rather than switching targets to generated C.
Archive bytes were not fetched, so this conclusion concerns the published build
system, not an asserted tar member listing. Do not invent an Autotools
`--disable-maintainer-mode` path for this release.
[Pinned Meson](https://raw.githubusercontent.com/martinpitt/umockdev/0.19.9/meson.build),
[options](https://raw.githubusercontent.com/martinpitt/umockdev/0.19.9/meson_options.txt),
[release workflow](https://raw.githubusercontent.com/martinpitt/umockdev/0.19.9/.github/workflows/release.yml),
[dist producer](https://raw.githubusercontent.com/martinpitt/umockdev/0.19.9/tests/run-apt),
[version hook](https://raw.githubusercontent.com/martinpitt/umockdev/0.19.9/src/getversion.sh).

The official Arch package page now reports **0.19.9-1**, 147.6 KiB compressed,
669.4 KiB installed. Some indexed search/file-list pages still return 0.19.8;
require the downloaded package's metadata/signature to match 0.19.9-1 and record
its actual checksum. The older file list documents the expected paths, but is
not evidence of 0.19.9 archive contents. No binary package checksum was verified
here. [Package metadata](https://archlinux.org/packages/extra/x86_64/umockdev/),
[cached older file list](https://archlinux.org/packages/extra/x86_64/umockdev/files/).

## Concrete acquisition/build plan — proposed, not run

1. Supply one official Arch 0.19.9-1 package plus signature. Verify its signer,
   architecture, package metadata and hash, then extract privately with `bsdtar`
   or equivalent; do not invoke pacman installation hooks. A stable location
   such as `/tmp/canopa-usb-tools/umockdev-0.19.9-1` can be reused. Required
   artifacts are `usr/lib/libumockdev.so.0`,
   `usr/lib/libumockdev-preload.so.0` plus their real library files/symlinks,
   `usr/lib/girepository-1.0/UMockdev-1.0.typelib`, and `usr/bin/umockdev-run`
   (the upstream test file executes it during collection). Preserve package
   attribution and a concise signature/hash receipt.
2. Inspect the extracted ELF NEEDED entries before loading. The official runtime
   list is libc, gio/glib/gobject, libpcap.so.1 and libudev.so.1; local files
   exist. Validate GI namespace loading and an empty synthetic inventory as
   separate future checks before launching XDP.
3. If an official binary is unavailable, supply the verified upstream release
   asset and a working private Vala compiler/toolchain. Meson/Ninja, C compiler,
   GLib/GObject/Gio/GioUnix, libudev/libpcap headers, and g-ir-compiler suffice
   for the relevant library/typelib/tools; gtk-doc can stay disabled. GUdev is
   available for checks. No GTK UI dependency is introduced.

Example source-build commands, **only after those dependencies are supplied**:

```sh
task_src=/tmp/canopa-usb-tools/umockdev-0.19.9
task_build=/tmp/canopa-usb-tools/umockdev-build
meson setup "$task_build" "$task_src" --buildtype=debug -Dgtk_doc=false
meson compile -C "$task_build" -j 2
```

This builds in the separate tools tree and performs no install or Cargo build.
To minimize compilation further, inspect generated Ninja target metadata and
select only the library, preload, `UMockdev-1.0.typelib` and `umockdev-run`
outputs. Their dependencies include generated GIR/VAPI files. For build-tree
use set both loader and typelib paths to that build directory, as upstream
documents. Retain actual build commands/versions; source-build success is still
unverified. [Upstream build-tree instructions](https://raw.githubusercontent.com/martinpitt/umockdev/main/README.md#build-test-run).

## Minimal standalone GI/Gio runtime

Use environment maps scoped to the fixture controller and canonical XDP child,
not shell/session-wide exports. For the private binary package, the proposed
controller environment is:

```sh
task_tools=/tmp/canopa-usb-tools/umockdev-0.19.9-1
env LD_LIBRARY_PATH="$task_tools/usr/lib" \
    GI_TYPELIB_PATH="$task_tools/usr/lib/girepository-1.0" \
    LD_PRELOAD="$task_tools/usr/lib/libumockdev-preload.so.0" \
    PATH="$task_tools/usr/bin:$PATH" \
    python3 /absolute/path/to/the-future-standalone-fixture.py
```

This is an environment recipe, not an existing fixture path or executed command.
GI_TYPELIB_PATH adds UMockdev discovery while system GLib/Gio typelibs remain
available. Python imports `gi`, requires UMockdev 1.0, and uses
`from gi.repository import UMockdev, Gio, GLib`; GUdev 1.0 is useful for the
initial interception check but is not needed for Gio D-Bus calls. No pytest,
dbusmock, libusb, USB ioctl replay or hardware recording is required for an
equivalent standalone consent/FD fixture.
[Wrapper preload mechanism](https://raw.githubusercontent.com/martinpitt/umockdev/0.19.9/src/umockdev-wrapper),
[upstream Python GI example](https://raw.githubusercontent.com/martinpitt/umockdev/0.19.9/tests/test-umockdev.py).

Create exactly one `UMockdev.Testbed.new()` per fixture process before starting
GUdev/XDP. It creates an empty sysfs tree and sets `UMOCKDEV_DIR`; still pass
`testbed.get_root_dir()` explicitly to the XDP child's environment, following
the canonical harness. Hold the Testbed reference until all children have
exited. Its destructor removes the tree and unsets the variable. Keep the
backend/Iced children on a clean environment map: they need no interception.
[Testbed lifecycle](https://raw.githubusercontent.com/martinpitt/umockdev/0.19.9/src/umockdev.vala),
[canonical child environment](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/tests/conftest.py:484).

The interposer redirects libc-visible sysfs, device nodes and uevents. A private
D-Bus alone does not isolate hardware enumeration. **UMockdev is not a security
sandbox:** its `/dev` and `/proc` paths fall through to the real filesystem when
no mock entry exists; `/sys` is replaced. Use synthetic records only, begin by
asserting empty visible inventory, and allow only testbed-created device paths.
Do not run `umockdev-record`. Use the Testbed API, rather than `umockdev-run`
alone, when add/change/remove events must be driven after frontend startup.
[Interception semantics](https://github.com/martinpitt/umockdev#mocking-proc-and-dev).

The canonical synthetic record supplies `P: /devices/usbN`,
`N: bus/usb/001/NNN`, `DEVTYPE=usb_device`, `DEVNAME`, subsystem `usb`, vendor,
model and serial properties. These nodes are synthetic; fill their backing
regular files with sentinel bytes and test transferred FDs with read/fcntl,
without any USB ioctl. Set up a private bus, permission store and explicit Usb
backend routing before starting canonical XDP with the same testbed environment.
Subscribe to public events before causing additions/removals; use a GLib main
loop and bounded deadlines. [Canonical record](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/tests/test_usb.py:66),
[frontend node opens](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/src/usb.c:1354).

## What upstream test_usb additionally requires

The canonical `test_usb.py` uses pytest, python-dbusmock, dbus-python, PyGObject,
GLib and UMockdev. Its removal case requires UMockdev **>=0.18.4** and executes
`umockdev-run --version` at collection. Host/Flatpak identity variants are test
fixtures, not real Flatpak installations. A fake Flatpak with no USB query sees
no devices in these tests; the Host variant sees all fake inventory.
[Imports/fixtures/version gate](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/tests/test_usb.py:5),
[query/removal expectations](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/tests/test_usb.py:278).

Its shared conftest imports UMockdev even before tests run, ensures preload by
re-execing Python, makes isolated XDG directories, starts private session/system
buses, and points XDP at mock backend templates. Required executable variables
are `XDG_DESKTOP_PORTAL_PATH`, `XDG_PERMISSION_STORE_PATH`,
`XDG_DOCUMENT_PORTAL_PATH`, `XDP_VALIDATE_ICON`, `XDP_VALIDATE_SOUND` (or the
documented validator-auto mode). `tests/run-test.sh` automatically runs Meson
compile unless `UNDER_MESON` is set; do not use it casually during guarded work.
[Harness setup](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/tests/conftest.py:48),
[private buses](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/tests/conftest.py:253),
[compile side effect](/tmp/canopa-xdp-session-canonical-13/xdg-desktop-portal-1.22.1/tests/run-test.sh:29).

A standalone Gio fixture can reuse the same inventory and public API sequence
with existing Python/GI and avoid the pytest/dbusmock acquisition entirely.
Running it successfully would establish synthetic frontend/backend behavior,
not real hardware transactions, kernel FD revocation, real Flatpak identity,
or distribution acceptance. No such runtime acceptance is claimed here.
