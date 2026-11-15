# Native desktop fixtures

This directory holds the fixtures that native tests use: the acceptance manifest,
the Files scenario drivers, small C/C++ Wayland and libei clients, and Python
D-Bus peers that stand in for system services. They are test-only code under
the project license, except the two Wayland protocol files noted below. Nothing
here is installed or run against the live session.

See [testing](../../docs/testing.md) for the check gates and
[CI](../../docs/ci.md) for the self-hosted desktop runner.

## Private sessions

Every native test owns its whole environment:

- a private `dbus-daemon` session bus with no service activation directories
  (a fixture may add directories it owns), and no system bus
  (`DBUS_SYSTEM_BUS_ADDRESS` points at a missing socket);
- private `XDG_RUNTIME_DIR`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME`,
  `XDG_STATE_HOME` and `XDG_CACHE_HOME`, usually a private `HOME` too;
- a nested compositor: Niri inside a headless pixman Weston (most cases), Niri
  inside headless Sway (`test-desktop.py`), headless Sway alone (Sway comparison
  cases, `test-fork-desktop.py`, `test-screenshot-composition.py`), or Cage/Weston
  kiosk for the greeter;
- software rendering (`LIBGL_ALWAYS_SOFTWARE=1`, `WGPU_BACKEND=gl`) and
  `CANOPA_LANGUAGE=en`, because checks assert English text.

The inherited `DISPLAY`, `WAYLAND_DISPLAY`, `NIRI_SOCKET`,
`DBUS_SESSION_BUS_ADDRESS`, `AT_SPI_BUS_ADDRESS` and `NOTIFY_SOCKET` are removed
before anything starts.

Shared Python modules in `scripts/`:

| Module | Role |
| --- | --- |
| `validate-desktop.py` | `Session`: private bus, Weston + Niri startup, output scale, process groups and cleanup; `build_pointer()` compiles `click`/`scroll`/`drag`; `capture()` (grim) and `screenshot()` (Niri). Run alone, it measures Shell CPU/memory. |
| `screencast_media.py` | `MediaSession`: a `Session` with private PipeWire/WirePlumber and a checksum-pinned Niri, used by the sharing harnesses |
| `native_compositor.py` | Niri selection, checksum verification and the shared snapshot store |
| `native_accessibility.py` | Private AT-SPI bus launcher and an independent peer (`session-a11y.py`) |
| `file_chooser_toolkit.py` | Builds and caches the GTK/Qt file-dialog clients under `target/portal-filechooser-*-client/` |
| `window_screenshot_fixture.py` | Window-screenshot receiver, pointer and frame helpers, cached under `target/native-test-tools/window-screenshot/` |

Each test writes logs, screenshots and a `result.json` or `summary.json` into
an output directory that must not exist yet.

### Compositor selection

Native runs share one read-only copy of each Niri executable, stored by SHA-256
under `target/native-test-snapshots/compositor-by-sha256/<sha256>/niri`. The
acceptance runner and the sharing harnesses link to that copy from their output
directory (`OUTPUT/compositor/bin/niri`) instead of copying the binary, so
rebuilding the original during a run cannot change later cases.

Use the compositor Canopa ships, not a distribution Niri. Stock Niri delivers a
Drop to the receiver after the drag source is destroyed, so the
`files-dnd-cancel-*` cases fail there. The pinned kevinard/niri fork carries the
fix (`canopa/dnd-source-lifetime`), as well as the RemoteDesktop, InputCapture
and GlobalShortcuts topics that those harnesses need. Build it with
`scripts/build-niri.py` (see [fork maintenance](../../docs/vendor-maintenance.md),
"Niri compositor"); the executable is `target/niri-fork/debug/niri` and
`binary_sha256` in the work directory's `canopa-build.json` is its checksum.

## Acceptance matrix

`acceptance.json` lists every maintained native scenario as
`{"id", "script", "args"}`. `scripts/run-desktop-acceptance.py` runs it:

```sh
python3 scripts/run-desktop-acceptance.py --list
python3 scripts/run-desktop-acceptance.py --niri target/niri-fork/debug/niri \
  --niri-sha256 HEX --stasis /usr/bin/stasis --output /tmp/canopa-acceptance
python3 scripts/run-desktop-acceptance.py --niri PATH --niri-sha256 HEX \
  --case drawer-modes --case panel-motion --output NEW_DIR
```

| Option | Default | Meaning |
| --- | --- | --- |
| `--list` | | Validate the manifest and print `id: script`; starts nothing |
| `--case ID` | all cases | Select a scenario; repeat for several |
| `--output DIR` | required to run | New directory; refused if it exists |
| `--niri PATH` | `niri` from `PATH` | Niri for every case in the batch |
| `--niri-sha256 HEX` | | Refuse to start unless the selected Niri has this checksum |
| `--shell`, `--notify`, `--files`, `--greeter`, `--polkit`, `--pinentry` | `target/debug/canopa-*` | Binaries under test |
| `--session-bin-dir` | `target/debug` | Directory for the session-tool binaries |
| `--stasis` | `target/debug/stasis` | External Stasis 1.7 executable for idle cases |

The runner adds only the frozen Niri's directory to `PATH`, so every Niri server
and `niri msg` call in the batch uses the same bytes. Each case runs with the
repository root as working directory and writes:

- `OUTPUT/<id>.log`: the case's combined stdout and stderr;
- `OUTPUT/<id>/`: its screenshots, logs and result files;
- `OUTPUT/summary.json`: selected cases, each command, return code, elapsed time
  and status, and the Niri source path, version, SHA-256 and a final check that
  the snapshot did not change.

A failing case does not stop later cases. The exit status is non-zero if any
case failed.

Manifest rules, checked on every invocation (including `--list`, which runs in
`bash scripts/check.sh --workspace`):

- `id` is unique and matches `[a-z0-9-]+`;
- `script` is a file directly in `scripts/`;
- `args` is a list of strings containing `{artifacts}`; the other placeholders
  are `{shell}`, `{notify}`, `{files}`, `{session}`, `{greeter}`, `{polkit}`,
  `{pinentry}` and `{stasis}`;
- every `scripts/verify-*.py`, plus `test-desktop.py` and `test-fork-desktop.py`,
  is used by at least one case. Add a manifest entry when you add a verifier.

Appearance variants: `-dark` cases run at scale 1 and `-light` cases at a
fractional scale (1.25; 1.5 for `shell-appearance-light` and `quick-settings-light`).

### Runner preflight

```sh
dbus-run-session -- python3 scripts/check-desktop-runner.py --niri PATH --niri-sha256 HEX
python3 scripts/test-desktop-runner.py [--niri PATH]
```

`check-desktop-runner.py` is read-only. It resolves Niri as the runner does,
looks for every required command, checks that Weston supports kiosk
program exit (15+), that the PAM fixtures compile, that bubblewrap works, that
Python GI exposes Gio, Atspi and Gtk/Gdk 4.0, that Pillow imports, the
`pkg-config` modules and the AT-SPI bus launcher and registry.

`test-desktop-runner.py` checks Niri selection and snapshot handoff without a
desktop: refusals for a missing file, a directory, a malformed or wrong checksum
and a non-Niri executable, `--list` without an executable, corrupt or symlinked
snapshots, and a source changing during copy. Two cases need a real Niri and are
skipped without one. It runs in the workspace gate.

### Other verifiers in the matrix

| Script | Covers |
| --- | --- |
| `test-desktop.py` | Shell and Notify appearance on Sway with nested Niri (`--color-scheme`, `--scale` 1, 1.25, 1.5 or 2) |
| `test-fork-desktop.py` | Shared fork behavior on private Sway outputs, including grabbed tray popups |
| `verify-shell-appearance.py`, `verify-theme-customization.py`, `verify-configuration-reload.py` | Bar appearance, theme reload, short-bar geometry, click-through, reload |
| `verify-drawer-modes.py`, `verify-panel-motion.py`, `verify-menu-scrolling.py`, `verify-overlay-placement.py` | Drawers, transitions and idle CPU, menu scrolling, two-bar placement |
| `verify-content-rotation.py`, `verify-focused-window-icons.py`, `verify-shell-launcher.py` | Rotated text, focused-window icons, launcher |
| `verify-idle-inhibitor.py` | Manual idle inhibition (`--compositor sway` or `niri`) |
| `verify-session-tools.py`, `verify-wallpaper-followups.py`, `verify-lockscreen-integration.py`, `verify-stasis.py` | Wallpaper, lock screen with logind and PAM fixtures, real Stasis |
| `verify-greeter.py`, `verify-greeter-followups.py` | Greeter against `greetd.py` (`--compositor direct`, `cage` or `weston`) |
| `verify-polkit.py`, `verify-pinentry.py`, `verify-settings.py` | Polkit agent, pinentry with real GnuPG in a temporary `GNUPGHOME`, Settings |

## Files cases

Most Files scenarios go through `scripts/verify-files.py --binary PATH --output
DIR [--scheme dark|light] [--scale N]`. Without a scenario flag it runs the
default browsing walkthrough (`files-dark`, `files-light`). Scenario flags are
mutually exclusive:

| Flag | Driver in this directory |
| --- | --- |
| `--iced-browsing-only`, `--iced-search-only`, `--iced-preview-only`, `--iced-interface-only`, `--iced-actions-only` | `files-iced-browsing.py`, `-search.py`, `-previews.py`, `-interface.py`, `-actions.py` |
| `--iced-views-only [--iced-views-case full\|reflow\|reflow-prefix\|reflow-stress\|gutter]` | `files-iced-views.py` |
| `--iced-preferences-only [--iced-preferences-case full\|toolbar-fast\|locations]` | `files-iced-preferences.py` |
| `--iced-media-only`, `--iced-media-formats-only [--desktop-entry FILE]` | `files-iced-media.py` with `files-media.py` |
| `--iced-operations-only [--iced-operations-case CASE]` | `files-iced-operations.py` and the per-case drivers below |
| `--downloads-only`, `--hidden-navigation-only`, `--reload-only` | `files-downloads.py`, `files-hidden-navigation.py`, `files-reload.py` |

`--iced-operations-case` selects one operations scenario (default `full`):

| Cases | Driver |
| --- | --- |
| `startup`, `startup-interrupted`, `window-navigation`, `location-editing`, `trash` | `files-iced-operations.py`, `files-iced-trash.py` |
| `recovery`, `recovery-directories` | `files-iced-recovery.py`, `files-iced-recovery-directories.py` |
| `interrupted` | `files-iced-interrupted.py` with `files-interrupted.py` (stops only the private Files process) |
| `clipboard`, `clipboard-lifetime`, `clipboard-capacity`, `clipboard-source-copy`, `clipboard-source-move`, `source-leases` | `files-iced-clipboard*.py`, `files-iced-source-leases.py` |
| `drops`, `drop-exchange`, `drop-choices`, `drop-stale`, `drop-devices` | `files-iced-drop*.py` |
| `transfers`, `parents`, `compact`, `focus`, `focus-expired` | `files-iced-transfers.py` |
| `conflicts`, `move-conflicts`, `question-focus`, `mixed-copy`, `mixed-move`, `unavailable` | `files-iced-conflicts.py`, `files-iced-mixed-conflicts.py`, `files-iced-unavailable.py` |
| `continuation`, `dismissal`, `transfer-completion`, `claim-review`, `prepared-rename` | `files-iced-continuation.py`, `-dismissal.py`, `-transfer-completion.py`, `-claim-review.py`, `-prepared-rename.py` |

Other Files verifiers:

| Script | Covers |
| --- | --- |
| `verify-files-dnd-transport.py` | Iced drag transport against the independent GTK peer `files-dnd-transport-peer.py`. Needs the `drag_transport` example next to `--binary` (`bash scripts/cargo.sh build -p canopa-files --example drag_transport`). `--cancel-control` repeats cancellation with a GTK source; `--gtk-receiver` uses GTK on both sides; `--compositor sway` is allowed only with both, at scale 1. |
| `verify-files-clipboard.py` | File clipboard service against independent `wl-copy`/`wl-paste` peers |
| `verify-file-device-catalog.py` | Headless device catalog against the private GIO provider (`--drives` adds Stop/Eject) |
| `verify-files-device-ui.py` | Device controls in the Iced sidebar and Places menu (`--removal stop\|eject`) |
| `verify-files-action-host.py` | Background actions and attention notifications on a private bus, no GUI |
| `verify-files-resident.py` | Headless residency and activation of the resident owner |
| `verify-files-startup.py` | Repeated first-window startup (`--fixture operations\|interrupted`) |
| `verify-files-manager.py` | `org.freedesktop.FileManager1` cold activation from a private service directory |

Files fixtures in this directory:

- `files-volume-monitor.c`: a GIO volume-monitor module loaded only through the
  fixture's `GIO_EXTRA_MODULES`. It simulates volumes, drives, mounts and removal;
  no real device is touched.
- `files-notifications.py`: private `org.freedesktop.Notifications` receiver.
- `files-media.py`: generates the media files with FFmpeg and runs private
  PipeWire/PipeWire-Pulse for recorded playback checks.
- `delay-activation.c`: `LD_PRELOAD` shim for the `focus-expired` case. It pauses
  delivery of one named surface's activation token after the compositor reply.

Media cases need GStreamer with the decoder plugins described in
[Files](../../docs/files.md); `--iced-media-formats-only` also needs FFmpeg with
the `libmp3lame`, `flac`, `libopus`, `aac` and `libx264` encoders. The
`drop-choices` case needs a second filesystem for its private destination.

## Portal and desktop-sharing harnesses

These are run by hand when working on a portal; they are not in the acceptance
matrix. Common conventions:

- `--binary` or `--portal` is the backend, `target/debug/xdg-desktop-portal-canopa`.
- `--frontend` is the pinned portal frontend from
  `scripts/build-xdg-portal-compat.py` (`src/xdg-desktop-portal` in its build
  directory). The dialog-portal harnesses without `--frontend` use the
  distribution frontend at `/usr/libexec/xdg-desktop-portal` or
  `/usr/lib/xdg-desktop-portal`.
- `--harness` is the `canopa-portal` library test executable from the normal
  workspace build (`target/debug/deps/canopa_portal-*`). The harness runs one
  ignored, environment-guarded test, such as
  `screencast::remote::production::tests::native_backend_fixture`, as the
  backend. `--normal-backend` runs the real backend executable instead, where offered.
- Every `--NAME PATH` executable option has a required `--NAME-sha256`.
- By default a harness writes a test portal descriptor that enables the
  interface under test. `--shipped-descriptor` uses `packaging/portal/canopa.portal`
  unchanged; `--portal-descriptor FILE` uses an exact optional-package descriptor.
- `--scheme`/`--color-scheme` `dark|light` and `--scale` select the appearance;
  run dark at 1 and light at 1.25 separately.
- Parent windows come from `portal-parent.c` (xdg-foreign-v2 exporter), compiled
  into the output directory.

| Script | Checks | Executables |
| --- | --- | --- |
| `test-settings-portal.py` | D-Bus activation and the public Settings portal (workspace gate) | `--binary`, optional `--frontend` |
| `test-access-portal.py`, `test-account-portal.py`, `test-app-chooser-portal.py`, `test-dynamic-launcher-portal.py`, `test-lockdown-portal.py`, `test-screenshot-portal.py` | Iced dialogs through the public frontend | `--binary` |
| `test-file-chooser-portal.py` | FileChooser dialogs (`--folder-history`) | `--binary` |
| `test-file-chooser-gtk.py`, `test-file-chooser-qt.py` | Ordinary GTK4 `GtkFileDialog` and `QFileDialog` routing to Iced (`file-chooser-gtk.c`, `file-chooser-qt.cpp`) | `--binary`, `--frontend` |
| `test-file-chooser-flatpak.py` | Flatpak caller, Iced chooser, Documents grants (`--modes open save cancel`, `--new-folder`, `--check-folder-history`) | `--binary`, `--niri`, `--frontend`, `--flatpak-store`, `--client-build` |
| `test-background-portal.py`, `test-background-frontend.py` | Background questions and review with Notify; the frontend variant uses bubblewrap instances | `--binary`, `--notify` |
| `test-notification-portal.py` | Notification backend and frontend (`--shell-binary` adds Center input) | `--binary` (Notify) |
| `test-wallpaper-portal.py` | Wallpaper review applied through the Shell owner | `--binary`, `--shell-binary` |
| `test-wallpaper-owner.py` | Shell's wallpaper owner on one Niri output (`--bar-layer`) | `--binary` (Shell) |
| `test-window-screenshot-portal.py` | Window screenshots through Niri and PipeWire | `--binary`, `--niri`, `--frontend` |
| `test-screenshot-composition.py` | PNG composition on a two-output Sway | `--binary` |
| `test-screencast-portal.py` | ScreenCast consent; `--media` adds real frames (needs `--niri`, `--frontend`) | `--binary` |
| `test-screencast-flatpak.py` | Flatpak ScreenCast with a restricted PipeWire FD (`--modes cancel stop exit`) | `--binary`, `--niri`, `--frontend`, `--flatpak-store`, `--client-build` |
| `test-screencast-capabilities.py`, `test-screencast-identities.py` | Cursor capabilities per PipeWire version; PipeWire serial lookup | `--binary`; `--harness` |
| `test-capture-owner-admission.py` | ScreenCast is refused before consent on a Niri without the capture-owner cleanup contract | `--portal`, `--niri` |
| `test-remote-desktop-portal.py` | Keyboard EIS sessions with Iced consent and controls; held-key release on every loss | `--niri`, `--portal`, `--harness` |
| `test-remote-desktop-media.py` | Combined ScreenCast and RemoteDesktop: monitor pixels through the restricted FD, view-only, source and device order | `--niri`, `--portal`, `--harness` |
| `test-remote-desktop-pointer.py` | Pointer consent, Notify delivery and EI handoff (`--matrix`, `--ending close\|ui-stop\|client-kill\|backend-kill`) | `--portal`, `--niri`, `--frontend`, `--harness` or `--normal-backend` |
| `test-remote-desktop-touch.py` | Touch through the public portal, Iced, Notify and libei | `--portal`, `--niri`, `--frontend` |
| `test-remote-desktop-availability.py` | Frontend device-cache invalidation; no desktop | `--portal`, `--frontend`, `--harness` or `--normal-backend` |
| `test-remote-desktop-dispatch.py` | Advertised Touch methods reach the backend dispatcher (workspace gate) | `--binary` |
| `test-remote-touch-lease.py` | Compositor touch leases and Wayland mapping (`--idle-check`) | `--niri` |
| `test-remote-touch-core.py`, `test-input-capture-keyboard-core.py` | Pure modules of the pinned Niri archive, compiled with `rustc --test` (`--archives`, default `target/fork-archives`) | none |
| `test-clipboard-portal.py` | Clipboard v1 consent and descriptor lifetimes | `--portal`, `--niri`, `--frontend`, `--harness` or `--normal-backend` |
| `test-input-capture-portal.py` | InputCapture with EI receiver (`--keyboard`, `--clipboard`, `--phase-case all\|repeated-enable\|stale-release`) | `--binary`, `--niri`, `--frontend` |
| `test-global-shortcuts-portal.py` | GlobalShortcuts (`--layout us\|fr`, `--xkb-options`, `--check-editor-inhibition`, `--check-keyboard-recording`) | `--binary`, `--niri`, `--frontend` |
| `test-global-shortcuts-lease.py` | Compositor shortcut leases with real Wayland input | `--niri` |
| `test-usb-portal.py` | USB consent with synthetic UMockdev devices (`--umockdev PREFIX`) | `--binary`, `--niri`, `--frontend` |
| `test-usb-frontend.py` | Frontend USB ownership with a controllable consent service | `--frontend`, `--source`, `--umockdev` |
| `test-desktop-sharing-package.py` | Optional sharing package from real build receipts | `--niri-build`, `--frontend-build` |

The Flatpak harnesses use an existing fixture-owned Flatpak store; prepare the
probe applications with `scripts/prepare-file-chooser-flatpak.py` and
`scripts/prepare-screencast-flatpak.py` (`--store`, `--runtime`, `--output`).
Nothing is downloaded.

The RemoteDesktop keyboard harnesses configure a French keymap in the private
Niri and use a harmless receiver (`remote-input-receiver.c`). They send real libei
input only after fresh consent.

Sharing fixtures in this directory:

| File | Role |
| --- | --- |
| `keyboard.c` | Persistent virtual keyboard with one full US keymap; reads evdev key codes (optionally `ctrl `-prefixed) from stdin and acknowledges each release |
| `remote-input-ei-client.c`, `remote-input-ei-pointer.c`, `remote-input-ei-touch.c` | libei senders on the portal's EIS socket |
| `remote-input-receiver.c`, `remote-input-pointer-receiver.c`, `remote-input-touch-receiver.c` | Wayland windows that record received keyboard, pointer and touch input |
| `input-capture-ei-client.c`, `input-capture-keyboard.c`, `input-capture-pointer.c` | InputCapture libei receiver and persistent private keyboard/pointer sources |
| `screencast-consumer.c` | Restricted-FD PipeWire consumer recording frames and cursor metadata |
| `screencast-flatpak.c`, `file-chooser-flatpak.c` | Sandboxed callers deployed by the prepare scripts |
| `portal-parent.c` | xdg-foreign-v2 parent window; `-DPORTAL_CHOSEN` and `-DPORTAL_CAPTURE_PATTERN` variants |

## Print harnesses

Print is opt-in (see [printing](../../docs/printing.md)). These harnesses never
use an installed scheduler or a physical printer. Those that take
`--scheduler`/`--cups-server` need a disposable CUPS scheduler and queue that you
own and set up; they create jobs only in that queue and cancel only their own.

The private CUPS client, IPP backend and test IPP receiver come from the pinned
CUPS fork (see [fork maintenance](../../docs/vendor-maintenance.md), "CUPS client,
IPP backend and receiver"):

```sh
python3 scripts/build-cups-authentication.py --archives ARCHIVES --download --work-dir target/cups-authentication-compat
python3 scripts/build-print-ipp-backend.py --client-build target/cups-authentication-compat/canopa-build.json --work-dir target/print-ipp-backend-compat/maintained
python3 scripts/build-print-ipp-receiver.py --client-build target/cups-authentication-compat/canopa-build.json --work-dir target/print-ipp-receiver
```

| Script | Checks | Key options |
| --- | --- | --- |
| `test-print-dialog.py` | Print catalog and Iced draft against a disposable scheduler; `--file-only` expects an empty scheduler and checks the Save picker | `--binary`, `--niri`, `--parent`, `--harness` (each with `-sha256`), `--cups-server`, `--queue` (default `CanopaFixture`) |
| `test-print-custom-media.py` | Exact custom paper lookup through the worker (metadata requests only) | `--binary`, `--scheduler`, `--queue`, repeated `--size`/`--reject WIDTHxHEIGHT` in 1/100 mm, `--sides`, `--instance`, `--config-home`, `--library` with `--expected-library-sha256`, `--receipt` |
| `test-print-cups-replies.py` | Worker acknowledgements; creates three jobs | `--binary`, `--scheduler`, `--queue`, `--document`, `--receipt` |
| `test-print-cups-reconnect.py` | Recovery across reconnects; the relay `print-cups-proxy.py` closes the connection at libcups cancellation | `--binary`, `--scheduler`, `--replacement`, `--queue` (same on both), `--receipt` |
| `test-print-early-replies.py` | Public CUPS write results against loopback TLS peers; no scheduler | `--library`, `--operation write\|start` |
| `test-cups-authentication.py` | Worker-only CUPS image against the strict Digest oracle `print-digest-oracle.py`; `--self-test` runs in the workspace gate | `--binary`, `--library`, `--algorithm`, `--case`, `--tls` |
| `test-print-handoff.py` | Print setup completes while a CLOEXEC fork copy (`print-handoff-inherit.py`) holds the setup socket; `--copy closed` is the control | `--binary`, `--harness`, `--artifacts` |
| `test-print-render-annotations.py` | Printable/screen-only annotation appearances through the isolated worker; no CUPS | `--binary`, `--output`, `--width`/`--height` in PDF points |
| `test-print-ipp-receiver.py` | Validation-only custom media requests to the receiver (`print-custom-ipp.conf`) | `--binary`, `--library`, `--out` |
| `test-print-ipp-backend.py` | Backend option conversion with `print-pdf-advanced.ppd` and `print-advanced-ipp.conf`, including no invented resolution | `--backend`, `--receiver`, `--library`, `--out`, `--case 300dpi\|600dpi\|600x1200dpi\|118x236dpcm\|default` |
| `test-print-runtime-package.py`, `test-print-backend-package.py` | Optional runtime and backend staging from build receipts | `--build-receipt`, `--client-build`, `--receiver` |

Print fixtures:

- `print-paper-document.py --output FILE.pdf [--annotation none|print|screen|print-hidden]`
  writes four A4 pages with centered calibration marks that stay visible when a
  smaller sheet crops the page at 100%.
- `print-pdf-advanced.ppd` declares source, type, quality and resolution choices,
  a Manual/Glossy conflict and a smaller manual-tray custom limit.
- `print-custom-ipp.conf` and `print-advanced-ipp.conf` are `ippeveprinter`
  attribute files for the PDF receiver; dimensions and margins are in 1/100 mm.

`test-print-dialog.py --parent` takes a compiled `portal-parent.c`; build it as
the other portal harnesses do (generate `xdg-shell` and `xdg-foreign-unstable-v2`
code with `wayland-scanner`, then `cc ... -lwayland-client`).

## Authentication and session fixtures

| File | Role | Used by |
| --- | --- | --- |
| `greetd.py` | greetd IPC peer; never authenticates host accounts or runs sessions | `verify-greeter*.py` |
| `accounts.py` | Read-only AccountsService on the private bus | greeter, polkit |
| `lock-logind.py` | logind protocol fixture on the private bus | lock, Stasis, polkit, greeter follow-ups |
| `lock-pam.c`, `lock-pam-interactive.c` | Isolated PAM modules; never install in host PAM | lock, session tools, `test-greetd-transport.py`, `test-polkit-helper.py` |
| `polkit.py`, `polkit-session.c` | Private polkit authority and native-session test double | `verify-polkit.py` |
| `nss.c` | Synthetic NSS enumeration, active only in the isolated helper | `verify-greeter.py`, `test-nss-discovery.py` |
| `session-keyboard.c` | Named keys, typed text and alternate keymaps; works through kiosk compositors | greeter, polkit, pinentry, wallpaper |
| `session-a11y.py` | Independent libatspi client on fixture-owned accessibility buses | `native_accessibility.py` |
| `u2f-enroll.c` | Test-only non-resident U2F enrollment | `test-lockscreen-device.py` |

Namespace-isolated harnesses outside the matrix:

- `test-greetd-transport.py --greetd PATH`: Canopa's transport against real greetd
  with private PAM.
- `test-nss-discovery.py`: real libc NSS in a private Bubblewrap mount namespace
  with synthetic passwd, nsswitch and login.defs.
- `test-polkit-helper.py`: real `PolkitAgentSession` and helper with test-only
  account and PAM files in user/PID/mount/network namespaces.
- `test-lockscreen-device.py --module PAM_MODULE --device HIDRAW`: opt-in
  physical FIDO touch; enroll with `--enroll FILE` or `--u2f-enrollment`.

## Pointer and protocol fixtures

`click.c`, `scroll.c` and `drag.c` use `zwlr_virtual_pointer_v1` with coordinates
normalized to a 1600×1000 output. `Session.build_pointer()` generates the protocol
code with `wayland-scanner` into the output directory and compiles them; generated
code is not checked in.

- `click X Y [right|move]`: left or right click, or move only;
- `scroll X Y DELTA COUNT [INTERVAL_US]`: vertical axis events (default interval 8333 µs);
- `drag X1 Y1 X2 Y2 READY_FILE RELEASE_FILE`: holds the button at the target,
  writes `READY_FILE`, and releases when `RELEASE_FILE` appears.

Protocol files, copied unchanged with their MIT notices:

- `wlr-virtual-pointer-unstable-v1.xml` from `wayland-protocols-wlr` 0.3.12
  (wayland-rs revision `a3d7927d87799b2955bf491b51c7c2a3a82da661`); upstream in
  [wlr-protocols](https://gitlab.freedesktop.org/wlroots/wlr-protocols/-/blob/master/unstable/wlr-virtual-pointer-unstable-v1.xml).
- `virtual-keyboard-unstable-v1.xml` from `wayland-protocols-misc` 0.3.12, same
  revision; upstream
  [virtual-keyboard-unstable-v1.xml](https://github.com/Smithay/wayland-rs/blob/a3d7927d87799b2955bf491b51c7c2a3a82da661/wayland-protocols-misc/protocols/virtual-keyboard-unstable-v1.xml).

## Requirements

A Linux machine with software Mesa and the tools `check-desktop-runner.py`
checks (the list is in [CI](../../docs/ci.md), "Provisioning the runner"), the
pinned Niri fork and, for idle cases, an external Stasis 1.7. Sharing harnesses
also need the pinned portal frontend; the Qt, Flatpak, UMockdev and CUPS
harnesses need those toolkits or prepared stores. Builds go through
`bash scripts/cargo.sh` (see [building](../../docs/building.md) for the disk guard).

## Known limitations

- The `files-dnd-cancel-*` cases fail on stock Niri; use the pinned fork.
- Print harnesses need a caller-provided disposable CUPS scheduler and queue; no script sets one up.
- The Flatpak harnesses need an existing fixture-owned Flatpak store and runtime; no script creates one.
- `test-lockscreen-device.py` needs a physical FIDO device and is never run automatically.
