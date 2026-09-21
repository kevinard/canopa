# Hosted CI setup

Status on 2026-09-21: no GitHub repository exists and no dedicated runner machine
has been selected. No hosted workflow result is claimed. The local release and
desktop evidence remain in [release verification](notification-release-verification.md).

The workflows use Rust 1.95.0, matching the verified release toolchain. `Rust
checks` runs core and GUI checks on GitHub's Ubuntu 24.04 workers on push, PR,
or manual dispatch. `Desktop acceptance` is manually dispatched to a Linux
runner with the custom label `canopa-desktop`.

## Provision the dedicated machine

1. Select a Linux machine or VM with software Mesa, a writable runner account,
   Rustup, Git, Python 3, D-Bus, Niri 26.04 (the local acceptance version), Sway,
   grim, C compiler, pkg-config, wayland-scanner, AT-SPI launcher/registry, and
   Python GI exposing Gio and Atspi. Include ALSA, udev, Wayland, XKB and PulseAudio
   development packages for both Rust builds. On Ubuntu the common dependencies
   are `build-essential pkg-config libasound2-dev libudev-dev libwayland-dev
   libxkbcommon-dev libpulse-dev dbus at-spi2-core python3-gi gir1.2-atspi-2.0
   sway grim libgl1-mesa-dri`. Install a compatible Niri separately if the
   distribution does not supply it.
2. Place the compatible Niribar checkout at `/opt/canopa/niribar` (or supply its
   actual path at dispatch). Its `Cargo.lock` and matching v4 protocol/vendor
   changes must be present. Record the exact Notify and Niribar commit IDs.
3. Run `python3 scripts/check-desktop-runner.py --niribar-root /opt/canopa/niribar`
   as the runner account, followed by `bash scripts/check-panel-protocol.sh
   /opt/canopa/niribar`. These are read-only checks, not a graphical pass.
4. Once the repository exists, use **Settings → Actions → Runners → New
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

Push the reviewed source to the chosen repository. Put both workflow files on
its default branch before dispatch: GitHub requires this for
[manual workflows](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#workflow_dispatch).
Run **Rust checks**, then **Desktop acceptance**, supplying `niribar_root`.
The latter builds both adapters and uploads `desktop-acceptance` artifacts.

Record the run URLs, exact revisions, runner/toolchain versions, result, and
artifact location in the release record. A queued job, preflight pass or local
execution does not close the hosted gate. Orca acceptance remains the separate
[recorded screen-reader procedure](orca-isolated-fixture-research.md).
