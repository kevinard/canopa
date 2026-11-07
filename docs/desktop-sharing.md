# Canopa desktop sharing

Canopa provides Rust/Iced ScreenCast, RemoteDesktop and Clipboard portals. Screen
and window selection, separate keyboard/pointer/touch/clipboard approval, and the ongoing
Stop sharing controls use Iced. The standard XDG frontend still owns public
requests, sandbox identity and restricted PipeWire access.

Desktop sharing needs both maintained companions: Niri supplies capture lifetime
and input leases; the frontend supplies ordered input and original-owner session
cleanup. The [Arch packages](installation.md#desktop-portals) include both
companions and select the Canopa sharing backends in the Canopa session. The
optional portable release component contains their executables, exact source
recipes, patches and original notices. It extends `canopa.portal` with only these
three interfaces. An additional explicit option includes InputCapture,
GlobalShortcuts and Usb. These interfaces remain experimental. Print retains its
separate opt-in. The base portable archive ships without the companion component.

## Build and package

Use existing verified build receipts when available. To prepare the current
combined Niri recipe and frontend, follow the source builders in the
[ScreenCast development guide](screencast-portal.md), selecting
`--overlay niri-remote-touch` for the combined Niri companion. That recipe also retains the Files
drag-source fixes and the later input facilities. Reuse the same source/build
directories with `--reuse`; building does not install either program.

Add both receipts when creating an application release:

```sh
bash scripts/package-release.sh BUILD_DIR OUTPUT_DIR \
  --desktop-niri NIRI_BUILD/canopa-build.json \
  --desktop-frontend FRONTEND_BUILD/canopa-build.json
bash scripts/smoke-release.sh OUTPUT_DIR/canopa-x86_64-unknown-linux-gnu.tar.gz \
  --require-desktop-sharing
```

Append `--extra-portals` to the package command to also include
[InputCapture](input-capture-portal.md), [GlobalShortcuts](global-shortcuts-portal.md)
and [Usb](usb-portal.md). This requires a combined capture/shortcut recipe:
`niri-remote-touch`, `niri-input-capture-keyboard`, or the preceding pointer-only
`niri-global-shortcuts`. Use `--require-extra-portals` on the
smoke command to require this six-interface selection. InputCapture advertises the keyboard/pointer classes supported by the selected
companion; both choices start unchecked. Touch capture remains unavailable.

With `niri-remote-touch`, RemoteDesktop also supports touch control of an approved
screen or window through the public Notify methods and EI touchscreen devices.
Touch has its own unchecked Iced choice (Alt+T) and requires a shared source;
keyboard or pointer approval does not grant touch. Connecting to EI releases
legacy contacts and ends Notify input for that session. Source or client loss
releases its held contacts. RemoteDesktop touch sends input; it does not add
touch reception to InputCapture. Older companions continue to expose only their
supported input classes.

The packager verifies current source inventories, canonical recipes and patches,
the built executable checksums, ELF dependencies and original notices. Both
receipts are required together. It rejects older capture recipes without creator
cleanup, stale recipes, changed sources, mismatched executables and incomplete
components. `share/canopa/desktop-sharing.json` records the resulting identities
and required shared-library names. Libraries remain supplied by the distribution;
this is an x86-64 Linux component, not a self-contained runtime.

## Select the optional setup

Install the chosen application bundle using the existing
[Canopa backend installation](settings-portal.md#installation-and-backend-selection).
Keep its `libexec/canopa/desktop-sharing/` directory together. With a `~/.local`
installation, the companion executables are:

```text
~/.local/libexec/canopa/desktop-sharing/niri
~/.local/libexec/canopa/desktop-sharing/xdg-desktop-portal
```

Select that Niri executable in the compositor launcher for the next session,
retaining the existing configuration and `--session` option. Select the companion
frontend with a user service override. For the `~/.local` layout, copy the supplied
`share/canopa/desktop-sharing/xdg-desktop-portal.service.conf` to
`~/.config/systemd/user/xdg-desktop-portal.service.d/canopa.conf`. For another
prefix, change its `ExecStart` to the installed companion's absolute path.

Install the extended `share/xdg-desktop-portal/portals/canopa.portal` in
`~/.local/share/xdg-desktop-portal/portals/` (or the distribution's portal directory).
The companion frontend searches user and system data directories, so existing
backend descriptors remain available. Merge the selected entries from
`share/canopa/desktop-sharing/portals.conf.fragment` into the existing `[preferred]`
group of the session's selected portal configuration, normally
`~/.config/xdg-desktop-portal/niri-portals.conf`. Preserve its existing `default`
and every other interface selection. The fragment is not a replacement
configuration file.

For WirePlumber, install the two supplied access-policy files before starting the
new session. From the extracted bundle directory, for a per-user installation:

```sh
install -Dm644 share/canopa/desktop-sharing/wireplumber/canopa-screencast.lua \
  ~/.local/share/wireplumber/scripts/canopa-screencast.lua
install -Dm644 share/canopa/desktop-sharing/wireplumber/90-canopa-screencast.conf \
  ~/.config/wireplumber/wireplumber.conf.d/90-canopa-screencast.conf
```

The policy preserves the frontend's selected-stream grants. On the tested
WirePlumber 0.5.18, the stock portal permission manager otherwise widens the
connection's fallback permissions and exposes unrelated nodes. The Canopa policy
keeps fallback access denied and permits the core handshake; camera and ordinary
audio policy retain their existing handling. Older versions without the
permission-manager API keep their existing policy. See
[the access-policy decision](adr/0112-screencast-pipewire-access.md).

Reload user unit definitions with `systemctl --user daemon-reload`, then start a
new session with the selected compositor. Complete existing capture sessions
before replacing the frontend. Packaging and validation never make these live
selection changes automatically.

The bundled `bin/canopa-portal-config` can prepare the complete file instead of
manually merging a fragment. Starting from the configuration currently selected
for the session, name each interface to select, for example:

```sh
bin/canopa-portal-config --base CURRENT_SESSION_CONFIG \
  --interface Settings --interface FileChooser --interface ScreenCast \
  --interface RemoteDesktop --interface Clipboard \
  --output /tmp/niri-portals.conf.canopa
```

It reads the extended descriptors from this bundle, preserves other routes and
configured fallbacks, and creates a new file for review. Use `--list` to see its
advertised interfaces. Review and install it using the
[configuration instructions](settings-portal.md#generate-a-complete-configuration-for-review).
This does not select either companion or install the WirePlumber policy.

## Runtime and verification

PipeWire and its session manager must run in the session. The packaged executables
use the host's graphics, input and GLib libraries listed in the manifest. Metadata
cursor support needs PipeWire 1.4.8 or newer; the backend omits that mode on older
or unknown libraries. Missing or unsupported compositor contracts fail before
sharing or input approval. Approval is fresh for each session and each requested
input permission starts unchecked.

`scripts/test-settings-portal.py` verifies activation and normal interface
registration. `scripts/test-remote-desktop-availability.py --normal-backend`
checks device-cache updates through the real frontend on a private bus. Actual
Iced/video/stop-control verification uses the maintained
[ScreenCast media fixture](screencast-portal.md), passing the packaged Niri and
frontend paths and their manifest checksums. These are separate from broader
physical-device and distribution acceptance; optional packaging does not claim
those checks have passed.

Pass `--portal-descriptor BUNDLE/share/xdg-desktop-portal/portals/canopa.portal`
to that fixture to use the exact optional descriptor. The package regression is:

```sh
python3 scripts/test-desktop-sharing-package.py \
  --niri-build NIRI_BUILD/canopa-build.json \
  --frontend-build FRONTEND_BUILD/canopa-build.json \
  --output target/desktop-sharing-package-check.json
```

It loads the real companions after relocation, checks incomplete and altered
components and rejects stale/source-mismatched builds before staging. Its
temporary executable copies are removed when the check finishes.

The RemoteDesktop and Clipboard fixtures also accept `--normal-backend` and
`--portal-descriptor`: `scripts/test-remote-desktop-pointer.py` exercises real
pointer/keyboard Notify delivery and EI handoff, and
`scripts/test-clipboard-portal.py` checks independent consent, byte transfers and
pipe revocation. Supply `--portal`/`--portal-sha256`, `--niri`/`--niri-sha256` and
`--frontend`/`--frontend-sha256` for the chosen executables. The unit-fixture mode
remains available with an explicit harness instead of `--normal-backend`.

The real Flatpak ScreenCast fixture additionally checks authenticated sandbox
identity, denial of direct host PipeWire/backend access, exact window pixels,
hidden unrelated nodes, cancellation, Iced Stop and application-exit retirement.
It uses the same optional WirePlumber policy in a private namespace. Its existing
minimal runtime is reused without an SDK/runtime download. See
[the sandbox fixture instructions](screencast-portal.md#real-flatpak-client).

The InputCapture, GlobalShortcuts and Usb native fixtures accept the same
`--portal-descriptor` option and already start the normal backend. They use
`--binary`/`--binary-sha256` for its path and checksum. Usb verification uses only
synthetic UMockdev devices in a private session.
