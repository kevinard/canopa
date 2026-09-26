# Canopa

Canopa is a desktop environment built around the Niri Wayland compositor. Its Rust
workspace currently contains Niribar (the shell and Wi-Fi provider), Canopa Notify,
the `canopa` configuration tool, and shared configuration, appearance, Iced controls
and panel protocol crates.

Use Rust 1.95.0, selected by `rust-toolchain.toml`. On Debian/Ubuntu:

```sh
sudo apt-get install build-essential pkg-config libpulse-dev libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev dbus at-spi2-core
cargo build --workspace --bins --locked
```

Binaries are in root `target/debug/`: `niribar`, `niribar-wifi-provider`, and
`canopa-notify`, plus `canopa`. Build one application with `cargo build --locked -p niribar` or
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
The optional graphical acceptance workflow uses a dedicated runner with Niri, Sway,
grim, Weston, wtype, Python GI/AT-SPI/Pillow, and software Mesa;
see `notify/scripts/check-desktop-runner.py`.

There is one lockfile, one set of dependency forks, and one panel protocol source.
Cargo unifies dependency features when building applications together: combined GUI
builds include Niribar's theme-detection feature; Notify-only builds retain its
disabled theme-detection default. Always select `-p canopa-notify` for core-only
validation. The rotation prototype retains its independent experimental build.

Both applications discover `$XDG_CONFIG_HOME/canopa/config.kdl` (with the usual
`~/.config` fallback). Existing application files still work when that shared file
is absent. Run `canopa config check` to validate both schemas, and see the
[configuration guide](docs/configuration.md) for explicit migration and reloads.
Shell changes require restarting Niribar; Notify supports reload while running.
Notify continues to deliver banners while the shell is stopped. Both UIs use the
shared `appearance` settings for dark/light palettes and an optional accent color.
See [the appearance decision](docs/adr/0003-shared-appearance.md) and the remaining
phases in the [integration proposal](docs/canopa-integration-proposal.md).

Follow the [shell guide](niribar/README.md) and [notification guide](notify/README.md)
for feature options, using root `target/` paths. Runtime state paths are unchanged.

Build a release with `cargo build --workspace --bins --locked --release`, then run
`bash scripts/package-release.sh target/release dist`. The archive contains all four
binaries, current example configurations, service units, manuals, and source license
notices. Service units retain their existing executable locations; choose installation
paths accordingly. Packaging does not install or restart anything.
Check the archive with `bash scripts/smoke-release.sh dist/canopa-x86_64-unknown-linux-gnu.tar.gz`.

Start development with [CONTEXT-MAP.md](CONTEXT-MAP.md), the
[workspace decision](docs/adr/0001-shared-workspace-and-upstream-forks.md), and
[fork maintenance](docs/vendor-maintenance.md). Both original Git histories are
preserved as parents of the import commit and through `legacy/niribar` and
`legacy/notify` branches. [Source licenses](LICENSES.md) remain unchanged.

See the [integration validation record](docs/integration-validation.md) for local
test results, graphical fixture coverage and release-packaging evidence.

The [shared configuration validation record](docs/shared-configuration-validation.md)
covers discovery, migration, namespace isolation, diagnostics, and reload behavior.

The [shared appearance validation record](docs/shared-appearance-validation.md)
covers palette contrast, shared controls, appearance reloads and desktop fixtures.
