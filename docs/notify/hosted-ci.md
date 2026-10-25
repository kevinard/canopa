# Hosted CI setup

Run the following commands from the Canopa workspace root. Local validation does
not establish a hosted workflow result. Earlier Notify release and desktop evidence
remain in [release verification](notification-release-verification.md).

The workflows use Rust 1.95.0, selected by the root toolchain file. `Canopa checks`
runs core and GUI checks on GitHub's Ubuntu 24.04 workers on push, PR,
or manual dispatch. `Desktop acceptance` is manually dispatched to a Linux
runner with the custom label `canopa-desktop`.

## Provision the dedicated machine

1. Select a Linux machine or VM with software Mesa, a writable runner account,
   Rustup, Git, Python 3, D-Bus, Niri 26.04 (the local acceptance version), Sway,
   grim, Weston, wtype, wl-clipboard, Poppler tools, GTK4 >= 4.12, C compiler, pkg-config, wayland-scanner, AT-SPI launcher/registry,
   Python Pillow and GI exposing Gio and Atspi. Media acceptance also needs FFmpeg,
   GTK's GStreamer backend and decoder plugins, PipeWire/Pulse, policy-only
   WirePlumber and PulseAudio capture tools. Include ALSA, udev, Wayland, XKB and PulseAudio
   development packages for both Rust builds. On Ubuntu the common dependencies
   are `build-essential pkg-config libasound2-dev libudev-dev libwayland-dev
   libxkbcommon-dev libpulse-dev dbus at-spi2-core python3-gi gir1.2-atspi-2.0
   sway grim weston wtype wl-clipboard poppler-utils libgtk-4-dev python3-pil libgl1-mesa-dri
   libgtk-4-media-gstreamer gstreamer1.0-plugins-base gstreamer1.0-plugins-good
   gstreamer1.0-plugins-bad gstreamer1.0-libav ffmpeg pipewire pipewire-pulse
   wireplumber pulseaudio-utils desktop-file-utils`. Install a compatible Niri separately if the
   distribution does not supply it.
2. Check out the Canopa repository. It contains the desktop applications, the shared
   panel protocol, dependency forks and a single root `Cargo.lock`. Record the
   workspace commit ID; a separate host checkout is no longer required.
3. Select the Niri executable and record its SHA-256. Run
   `dbus-run-session -- python3 scripts/check-desktop-runner.py --niri PATH --niri-sha256 HEX`
   as the runner account, followed by `bash scripts/check-panel-protocol.sh`.
   These are prerequisite checks, not a graphical pass.
   Stock Niri 26.04 reproduces the strict drag-source cancellation failure. The
   pinned compatibility build and its source/lockfile receipt are described in
   [vendor maintenance](../vendor-maintenance.md) and
   [ADR0024](../adr/0024-niri-drag-source-lifetime.md). A different compositor
   build must pass the same gate. Do not replace a live session implicitly.
4. In the hosting repository, use **Settings → Actions → Runners → New
   self-hosted runner → Linux** and the repository-generated download and
   registration commands. Add `--labels canopa-desktop` to registration. Install
   and start the runner service as documented by GitHub. Registration tokens
   are temporary credentials; do not put them in this repository.

GitHub's [runner registration guide](https://docs.github.com/en/actions/how-tos/manage-runners/self-hosted-runners/add-runners)
provides the current runner download and service setup. The fixture creates
its own bus, XDG directories, Sway and nested Niri; the runner does not need
access to a user's live desktop. Desktop jobs are serialized by workflow
concurrency and restricted to manual dispatch.

## Run and record the gates

Push the reviewed source to the chosen repository. Put the root workflow files on
its default branch before dispatch: GitHub requires this for
[manual workflows](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#workflow_dispatch).
Run **Canopa checks**, then **Desktop acceptance**. Supply the runner's absolute
Niri path and matching executable SHA-256 as dispatch inputs; no checkout-path
input is needed. Both preflight and execution use that selection. The latter
builds the desktop applications and drag transport example, then uploads
`desktop-acceptance` artifacts, including the exact private Niri copy and its
identity in `summary.json`. The full default matrix remains required; selected
local scenarios and workflow configuration do not establish a hosted pass.
The serialized job has a twelve-hour budget for the complete inventory; it
retains all scenarios. This budget does not establish actual runtime or success.

Record the run URLs, exact revisions, runner/toolchain versions, result, and
artifact location in the release record. A queued job, preflight pass or local
execution does not close the hosted gate. Orca acceptance remains the separate
[recorded screen-reader procedure](orca-isolated-fixture-research.md).
