# Continuous integration

Three GitHub Actions workflows live in `.github/workflows/`. All use Rust 1.95.0
(the version in `rust-toolchain.toml`) through `dtolnay/rust-toolchain`.

| Workflow | File | Trigger | Runner | What it runs |
| --- | --- | --- | --- | --- |
| Canopa checks | `check.yml` | push, pull request, manual | `ubuntu-24.04`, matrix `core` / `desktop` | `bash scripts/check.sh --core` or `bash scripts/check.sh --workspace` |
| Desktop acceptance | `desktop.yml` | manual only | self-hosted, labels `linux`, `canopa-desktop` | runner preflight, desktop build, the native Settings parser tests and `scripts/run-desktop-acceptance.py` |
| Release | `release.yml` | tags `vX.Y.Z` | `ubuntu-24.04` | release build, private CUPS client and IPP backend, `package-release.sh`, `smoke-release.sh`, GitHub release upload |

The current state of these workflows, and the known failures, are tracked in
[issue #50](https://github.com/kevinard/canopa/issues/50).

## Canopa checks

The `core` job installs the headless dependencies and runs the `--core` gate
(no desktop rendering stack). The `desktop` job adds GStreamer, GTK4 and Poppler
and runs the full `--workspace` gate. See [testing](testing.md) for what each gate
contains. The apt package lists in `check.yml` are the authoritative set of Ubuntu
build and test dependencies.

## Desktop acceptance

`desktop.yml` is dispatched by hand with three inputs:

| Input | Default | Meaning |
| --- | --- | --- |
| `niri` | empty | Absolute path of a Canopa compositor build to test with; empty builds the pinned fork |
| `niri_sha256` | empty | SHA-256 of that executable (required with `niri`); the run refuses a mismatch |
| `stasis` | `/usr/bin/stasis` | Stasis idle daemon used by the idle cases |

Jobs are serialized (concurrency group `canopa-desktop`) and have a 12-hour
budget. Each scenario starts its own D-Bus session, XDG directories, Sway and
nested Niri, so the runner never touches a user's live desktop. The job uploads
the `desktop-acceptance` artifact (the `desktop-artifacts` directory): one log per case, `summary.json`, and the
identity of the exact Niri copy used.

The workflow tests the compositor Canopa ships, not a distribution Niri: stock
Niri lacks the drag-source cancellation fix that the `files-dnd-cancel-*` cases
need. Without `niri`, the first step builds the pinned fork in release mode with
`scripts/build-niri.py` (see [fork maintenance](vendor-maintenance.md#niri-compositor))
and passes the receipt's path and SHA-256 on; the artifacts record that checksum.
To reuse a build, pass its path and SHA-256, for example `/usr/lib/canopa/niri`
from the installed package.

### Provisioning the runner

1. Use a Linux machine or VM with software Mesa and a dedicated runner account.
   Install Rustup, Git, Python 3 and the tools the preflight lists: D-Bus, Sway,
   grim, Weston 15 or newer, Cage, wtype, wl-clipboard, Poppler tools, GTK4 ≥ 4.12,
   a C compiler, pkg-config, wayland-scanner, bubblewrap, Linux-PAM headers,
   GnuPG, FFmpeg, PipeWire with PipeWire-Pulse and WirePlumber, PulseAudio
   utilities, desktop-file-utils, the AT-SPI bus launcher and registry, Python
   Pillow and Python GI exposing Gio, Atspi and Gtk/Gdk 4.0, plus the
   development packages for ALSA, udev, Wayland, XKB, PulseAudio and polkit-agent.
2. Check out the repository and check the machine:

   ```sh
   dbus-run-session -- python3 scripts/check-desktop-runner.py --niri PATH --niri-sha256 HEX
   bash scripts/check-panel-protocol.sh
   ```

   `python3 scripts/test-desktop-runner.py [--niri PATH]` checks the runner
   handoff without starting a desktop.
3. In the repository, open **Settings → Actions → Runners → New self-hosted
   runner → Linux**, and run the generated download and registration commands with
   `--labels canopa-desktop`. Install the runner as a service as GitHub documents.
   Registration tokens are temporary credentials: never commit them.

The workflow files must be on the default branch before GitHub allows a manual
dispatch.

## Release

Pushing a tag such as `v0.2.0` runs `release.yml`. It checks the fork pins
(`scripts/test-fork-sources.py`), builds the workspace in release mode for
`x86_64-unknown-linux-gnu`, builds the private CUPS client and IPP backend from the
pinned CUPS fork, packages everything with `scripts/package-release.sh`, verifies
the archive with `scripts/smoke-release.sh --require-print-runtime
--require-print-backend`, and attaches `dist/*.tar.gz` to a GitHub release. It
does not build the compositor or the portal frontend; those ship in the Arch
packages. See [releasing](releasing.md).
