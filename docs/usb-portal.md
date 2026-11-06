# Experimental USB portal

Canopa has a Rust `org.freedesktop.impl.portal.Usb` v1 backend and an Iced device
consent dialog. It is selectable with `--extra-portals` in the
[optional companion package](desktop-sharing.md), using the matching frontend.
The default portal descriptor and existing USB routing remain unchanged while
wider acceptance continues.

The dialog identifies the requesting application and shows device names, vendor,
VID:PID, serial and node location when supplied. All devices start unselected.
Select individual devices or use Select all, then explicitly Allow selected.
Requested read-write access may be changed to read-only. Deny, Escape or closing
the window returns no grant. Keyboard actions are Alt+S for selecting/clearing all,
Alt+R for making selected devices read-only and Alt+A for explicitly allowing a
nonempty selection. These shortcuts do not grant an empty selection.

The backend handles consent only. The standard frontend discovers and filters
devices, stores permissions, delivers device events and opens/transfers/releases
FDs. Canopa never treats a supplied device-file path as authority to open it.
A request cannot grant an unoffered device or stronger access. See
[ADR0064](adr/0064-usb-consent-and-frontend-ownership.md) for request ownership,
finite input limits and the frontend fixes required for exact FD modes,
cancellation and release after reacquisition.

Private native tests use synthetic UMockdev devices, the real Rust/Iced backend
and nested Niri. They do not access real USB hardware or establish Flatpak or
release distribution acceptance. The [ledger](../.scratch/xdg-portals/usb/README.md)
keeps current checks, failing controls and broader validation visible.

With a separately verified UMockdev prefix and prepared frontend source, run the
frontend trust-boundary regression without installing either service:

```sh
python3 scripts/test-usb-frontend.py --frontend FRONTEND_BINARY \
  --source PREPARED_FRONTEND_SOURCE --umockdev PRIVATE_UMOCKDEV_PREFIX \
  --output target/usb-frontend-check
```

The native fixture requires Python GI, UMockdev/GUdev typelibs, the existing
Wayland development tools, Weston, Niri, wtype and grim. It keeps the mock preload
out of the Rust backend and compositor environments. Provide verified executable
hashes and use a fresh evidence directory, while reusing the same build caches:

```sh
python3 scripts/test-usb-portal.py \
  --binary target/debug/xdg-desktop-portal-canopa --binary-sha256 BACKEND_SHA256 \
  --niri NIRI_BINARY --niri-sha256 NIRI_SHA256 \
  --frontend FRONTEND_BINARY --frontend-sha256 FRONTEND_SHA256 \
  --umockdev PRIVATE_UMOCKDEV_PREFIX --scheme dark --scale 1 \
  --output target/usb-native-check
```

The fixture adds USB only to its private descriptor by default.
`--shipped-descriptor` requires an actual shipped USB entry and must not be used
to claim normal registration while that entry is absent.
Use `--portal-descriptor BUNDLE/share/xdg-desktop-portal/portals/canopa.portal`
to verify the exact optional package descriptor.
