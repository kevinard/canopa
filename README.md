# Canopa

Canopa is a desktop environment built around the Niri Wayland compositor. Its Rust
workspace currently contains Canopa Shell (the shell, Wi-Fi, audio and media providers), Canopa Notify,
the `canopa` configuration tool, a Settings portal backend, and shared configuration, appearance, Iced controls
and panel protocol crates. Local icon lookup and bounded raster preparation are
shared through `canopa-assets`; see the [asset contracts](docs/adr/0005-shared-assets.md).

Use Rust 1.95.0, selected by `rust-toolchain.toml`. On Debian/Ubuntu:

```sh
sudo apt-get install build-essential pkg-config libpulse-dev libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev dbus at-spi2-core
cargo build --workspace --bins --locked
```

Binaries are in root `target/debug/`: `canopa-shell`, `canopa-wifi-provider`, `canopa-audio-provider`, `canopa-media-provider`,
`canopa-notify`, `canopa`, and `xdg-desktop-portal-canopa`. Build one application with `cargo build --locked -p canopa-shell` or
`cargo build --locked -p canopa-notify`. Notify's core build remains available with
`cargo build --locked -p canopa-notify --no-default-features`.

Run from the repository root:

```sh
bash scripts/check.sh
bash scripts/check.sh --core
python3 scripts/check-vendor.py --archives /path/to/verified-crate-archives
```

Tests use private D-Bus sessions. Full transport tests additionally need
`libglib2.0-bin`, `pipewire`, `pipewire-bin`, `pipewire-pulse`, and `pulseaudio-utils`.
The Settings frontend fixture also needs `xdg-desktop-portal` and `python3-gi`.
The optional graphical acceptance workflow uses a dedicated runner with Niri, Sway,
grim, Weston, wtype, Python GI/AT-SPI/Pillow, and software Mesa;
see `scripts/check-desktop-runner.py`.

After building Canopa Shell, `python3 scripts/verify-menu-scrolling.py --output
/tmp/canopa-scroll-new` checks installed-apps drawer scrolling in a private
desktop with PNG/SVG icons. It verifies keyboard scrolling without a blinking
search cursor, touchpad travel from steady and bursty input, and long gestures
across cached-row boundaries.
Screenshots and results are saved in the new output directory.

There is one lockfile, one set of dependency forks, and one panel protocol source.
All supported packages live under `crates/`, including `crates/canopa-shell/` and
`crates/canopa-notify/`. Repository tooling is in `scripts/`, service units and
release metadata are in `packaging/`, and application documentation is in
`docs/shell/` and `docs/notify/`. Run development commands from this root.
Cargo unifies dependency features when building applications together: combined GUI
builds include Canopa Shell's theme-detection feature; Notify-only builds retain its
disabled theme-detection default. Always select `-p canopa-notify` for core-only
validation. The rotation prototype in `experiments/focused-window-rotation/`
retains its independent experimental build.

Both applications discover `$XDG_CONFIG_HOME/canopa/config.kdl` (with the usual
`~/.config` fallback). Existing application files still work when that shared file
is absent. Run `canopa config check` to validate both schemas, and see the
[configuration guide](docs/configuration.md) for explicit migration and reloads.
Canopa Shell reloads saved configuration automatically, with `canopa-shell reload`
for an explicit result. Notify supports its own reload while running.
Notify continues to deliver banners while the shell is stopped. Both UIs use the
shared `appearance` settings for dark/light palettes and an optional accent color.
The optional `media` module shows the current track and opens [Media controls](docs/media.md) for existing MPRIS players.

The volume module opens [Sound controls](docs/audio.md) for speakers, headphones and
microphones. The default notification module opens the Center and controls DND without provider
wiring. See [notifications and session setup](docs/notifications.md).
See [the appearance decision](docs/adr/0003-shared-appearance.md) and the remaining
phases in the [integration proposal](docs/canopa-integration-proposal.md).
The [Settings portal](docs/settings-portal.md) exposes shared appearance to
applications, with explicit backend selection and its own SIGHUP reload.

Follow the [shell guide](crates/canopa-shell/README.md) and [notification guide](crates/canopa-notify/README.md)
for feature options, using root `target/` paths. Existing Niribar installations
should follow the [shell rename guide](docs/shell-rename.md) before updating.

Build a release with `cargo build --workspace --bins --locked --release`, then run
`bash scripts/package-release.sh target/release dist`. The archive contains all six
binaries, current example configurations, service units, manuals, and source license
notices. The companion user services use `~/.local/bin`; `canopa-session.target` starts
both. See the session setup guide before enabling them. Packaging does not install or restart anything.
Check the archive with `bash scripts/smoke-release.sh dist/canopa-x86_64-unknown-linux-gnu.tar.gz`.

For subsequent updates of an existing local companion installation, run:

```sh
./scripts/update.sh
```

This builds the current checkout, including uncommitted changes, and installs
`canopa-shell`, `canopa-wifi-provider`, `canopa-audio-provider`, `canopa-media-provider`, `canopa-notify`, and `canopa` together. It does
not fetch or pull Git commits. The updater requires Python 3, both companion user
services, their standard `~/.local/bin` executable paths, and default configuration,
control socket and state discovery in the same graphical-session environment.
Custom service arguments or environment overrides selecting other paths require
a manual update.

The new binaries validate your configuration before either service is stopped.
The updater preserves configuration and service definitions, backs up the previous
binaries, shared config and stopped notification state under
`$XDG_STATE_HOME/canopa-backups/` (default `~/.local/state/canopa-backups/`), and
restarts running or failed services. Intentionally stopped services stay stopped. Startup checks include
Notify's renderer status and the identity of each running executable. Failed
startup restores the previous binaries and state; state produced by the failed
update is retained in the backup. Keep these backups for database downgrades.

Successful updates reuse the release build cache, so unchanged packages are not
recompiled. Local packages (including shared vendor forks) are rebuilt on the
first update with this cache tracking, after a failed build, or when an outside
build changes their Cargo cache records. This prevents stale artifacts from
another checkout. Use `./scripts/update.sh --rebuild` to force that local rebuild.
Package discovery stays offline. Registry dependencies remain cached; Cargo
downloads missing build dependencies as needed.
Compilation uses one job by default, or
`CARGO_BUILD_JOBS` when set. Use `./scripts/update.sh --jobs 2` to override it.

Start development with [CONTEXT-MAP.md](CONTEXT-MAP.md), the
[workspace decision](docs/adr/0001-shared-workspace-and-upstream-forks.md),
[consolidated layout](docs/adr/0009-consolidated-repository-layout.md), and
[fork maintenance](docs/vendor-maintenance.md). Both original Git histories are
preserved as parents of the import commit and through `legacy/niribar` and
`legacy/notify` branches. [Source licenses](LICENSES.md) remain unchanged.

See the [integration validation record](docs/integration-validation.md) for local
test results, graphical fixture coverage and release-packaging evidence.
The [repository layout validation](docs/repository-layout-validation.md) records
checks of the consolidated crate, tooling and documentation paths.

The [shared configuration validation record](docs/shared-configuration-validation.md)
covers discovery, migration, namespace isolation, diagnostics, and reload behavior.

The [shared appearance validation record](docs/shared-appearance-validation.md)
covers palette contrast, shared controls, appearance reloads and desktop fixtures.

The [integrated notification validation record](docs/integrated-notifications-validation.md)
covers summary bounds, attention persistence, companion setup and restart recovery.

The [shared asset validation record](docs/shared-assets-validation.md) covers format
isolation, image budgets, icon lookup compatibility and real PNG/SVG rendering.
