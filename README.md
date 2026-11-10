# Canopa

Canopa is a desktop environment built around the Niri Wayland compositor. Its Rust
workspace currently contains Canopa Shell (the shell, Wi-Fi, audio and media providers), Canopa Notify, Canopa Files,
a wallpaper daemon, lockscreen, login greeter, polkit agent and GnuPG pinentry, Canopa Settings,
the `canopa` configuration tool, Settings/FileChooser/AppChooser/Access/Account/Background/DynamicLauncher/Lockdown/Wallpaper/Screenshot portal backends,
a [Notification portal served by Notify](docs/notification-portal.md), and shared configuration, appearance, Iced controls
and panel protocol crates. Local icon lookup and bounded raster preparation are
shared through `canopa-assets`; battery/media widget data through `canopa-widgets`; see the [asset contracts](docs/adr/0005-shared-assets.md).

Use Rust 1.95.0, selected by `rust-toolchain.toml`. On Debian/Ubuntu:

```sh
sudo apt-get install build-essential pkg-config libpulse-dev libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev libpam0g-dev fonts-dejavu-core dbus at-spi2-core
bash scripts/cargo.sh build --workspace --bins --locked
```

Binaries are in root `target/debug/`: `canopa-shell`, `canopa-wifi-provider`, `canopa-audio-provider`, `canopa-media-provider`,
`canopa-notify`, `canopa-files`, `canopa-settings`, `canopa-wallpaper`, `canopa-lock`, `canopa-greeter`, `canopa-polkit`, `canopa-pinentry`, `canopa-idle`, `canopa`, and `xdg-desktop-portal-canopa`. Build one application with `bash scripts/cargo.sh build --locked -p canopa-shell` or
`bash scripts/cargo.sh build --locked -p canopa-notify`. Notify's core build remains available with
`bash scripts/cargo.sh build --locked -p canopa-notify --no-default-features`.
Files' core can be checked with explicitly selected `--lib` or `--test` targets
and `--no-default-features` when desktop dependencies are unavailable.
See [Files runtime and installation](docs/files.md) for the Iced migration status,
PDF preview tools and launcher registration.

The [Arch installation guide](docs/installation.md) describes the system packages,
dedicated Canopa login session, compatible compositor and recovery procedure.
[Canopa Settings](docs/settings.md) provides displays, keyboard/pointer,
personalization and power preferences. Open it from the launcher or `canopa settings`.

Run from the repository root:

```sh
bash scripts/check.sh
bash scripts/check.sh --files --test transfers --test undo_changed_copy
bash scripts/check.sh --files --lib operations::fingerprint::receipt_tests
bash scripts/check.sh --workspace
bash scripts/check.sh --core
python3 scripts/fork_sources.py fetch niri xdg-desktop-portal cups --archives target/fork-archives
```

The default runs the focused Files Copy/fingerprint checks. Select the relevant
Files library or integration tests for other edits. Development checks enable
incremental debug compilation and reuse root `target/`; keep features and profiles
consistent between edits. The explicit `--workspace` gate runs all applications,
test targets and tooling for CI/releases, shared dependencies or broader changes.
Files-only edits normally need their affected tests rather than that gate.
The last command downloads and checksum-verifies the pinned fork archives of the
separately built compositor, portal frontend and CUPS components; see
[upstream dependency forks](docs/vendor-maintenance.md).

Ordinary integration cases share one executable per crate. The Files script keeps
the existing `--test transfers` case selectors and translates them into module
filters. Direct Cargo usage is `cargo test -p canopa-files --test integration cases::transfers::`.
See [test layout and selection](docs/testing.md) for native exceptions and examples.

Development and test builds keep incremental compilation and source file/line
backtraces with smaller `line-tables-only` debug information. Use
`bash scripts/cargo.sh build --profile debugging --locked -p PACKAGE` when you
need full debugger information. The Cargo wrapper and check/update scripts stop
builds before or during low-space conditions without deleting artifacts. See
[build profiles and disk guard](docs/building.md) for reserves and configuration.

Tests use private D-Bus sessions. Full transport tests additionally need
`libglib2.0-bin`, `pipewire`, `pipewire-bin`, `pipewire-pulse`, and `pulseaudio-utils`.
The Settings frontend fixture also needs `xdg-desktop-portal` and `python3-gi`.
Portal desktop builds require PipeWire 0.3.64 or newer development files and
libclang for its Rust bindings (`libpipewire-0.3-dev` and `libclang-dev` on Ubuntu
24.04). The desktop executable links the system PipeWire library.
The optional graphical acceptance workflow uses a dedicated runner with Niri, Sway,
grim, Weston 15+, Cage, wtype, Python GI/AT-SPI/Pillow, bubblewrap, Linux-PAM headers, and software Mesa;
see `scripts/check-desktop-runner.py`.
The native runner accepts `--niri PATH --niri-sha256 HEX` and preserves that exact
executable with its results. Hosted dispatch requires both values. See
[the fixture guide](scripts/desktop-fixture/README.md) for the pinned Niri
cancellation correction and compositor selection; no compositor is installed by
these checks.

After building Canopa Shell, `python3 scripts/verify-menu-scrolling.py --output
/tmp/canopa-scroll-new` checks installed-apps drawer scrolling in a private
desktop with PNG/SVG icons. It verifies keyboard scrolling without a blinking
search cursor, touchpad travel from steady and bursty input, and long gestures
across cached-row boundaries.
Screenshots and results are saved in the new output directory.

The Canopa applications share one lockfile, one set of dependency forks, and one
panel protocol source. The optional private Niri compatibility build uses its
pinned upstream lockfile; see [vendor maintenance](docs/vendor-maintenance.md).
All supported packages live under `crates/`, including `crates/canopa-shell/` and
`crates/canopa-notify/` and `crates/canopa-files/`. Repository tooling is in `scripts/`, service units and
release metadata are in `packaging/`, and application documentation is in
`docs/shell/`, `docs/notify/` and `docs/files/`. Run development commands from this root.
Cargo unifies dependency features when building applications together: combined GUI
builds include Canopa Shell's theme-detection feature; Notify-only builds retain its
disabled theme-detection default. Always select `-p canopa-notify` for core-only
validation. The rotation prototype in `experiments/focused-window-rotation/`
retains its independent experimental build.

All applications discover `$XDG_CONFIG_HOME/canopa/config.kdl` (with the usual
`~/.config` fallback). Existing application files still work when that shared file
is absent. Run `canopa config check` to validate all five schemas, and see the
[configuration guide](docs/configuration.md) for explicit migration and reloads.
Canopa Shell reloads saved configuration automatically, with `canopa-shell reload`
for an explicit result. Notify supports its own reload while running.
Files also reloads saved settings and appearance automatically.
Notify continues to deliver banners while the shell is stopped. All UIs use the
shared `appearance` settings for dark/light palettes and an optional accent color.
The optional `media` module shows the current track and opens [Media controls](docs/media.md) for existing MPRIS players.

The volume module opens [Sound controls](docs/audio.md) for speakers, headphones and
microphones. The default notification module opens the Center and controls DND without provider
wiring. See [notifications and session setup](docs/notifications.md).
See [the appearance decision](docs/adr/0003-shared-appearance.md) and the remaining
phases in the [integration proposal](docs/canopa-integration-proposal.md).
The [Settings portal](docs/settings-portal.md) exposes shared appearance to
applications, with explicit backend selection and its own SIGHUP reload. The
[FileChooser portal](docs/file-chooser-portal.md) provides Rust/Iced open and save
dialogs using the Files browsing model. The
[AppChooser portal](docs/app-chooser-portal.md) provides searchable application
selection and fresh Wayland activation tokens. The
[Access portal](docs/access-portal.md) provides explicit permission questions and
serialized choice controls.
The [Account portal](docs/account-portal.md) provides an editable profile-sharing
draft with field choices and a private, normalized avatar image.
The [Background portal](docs/background-portal.md) observes verified Niri application
windows and presents background-operation decisions through Notify and Iced.
The [DynamicLauncher portal](docs/dynamic-launcher-portal.md) provides launcher
name/icon review, with installation and token ownership retained by the frontend.
The [Lockdown portal](docs/lockdown-portal.md) owns persistent runtime privacy
switches and publishes changes to participating portal frontends.
The [Wallpaper portal](docs/wallpaper-portal.md) reviews frozen local/remote images
in Iced and applies desktop/lockscreen selections through Shell's persistent owner.
The [Screenshot portal](docs/screenshot-portal.md) reviews frozen Screen/Area
captures and confirms original pixel colors in Iced without changing the clipboard.
The [experimental USB backend](docs/usb-portal.md) provides Rust/Iced device selection
and read-only downgrade. The optional companion package's `--extra-portals`
selection also includes [InputCapture](docs/input-capture-portal.md) and
[GlobalShortcuts](docs/global-shortcuts-portal.md).
The [experimental ScreenCast backend](docs/screencast-portal.md) has Rust sessions
and Iced source consent and ongoing stop controls. Display/window video, static
resume, all cursor modes and crash cleanup pass on a pinned compatibility
compositor, with runtime cursor capability gating. The
[Arch packages](docs/installation.md#desktop-portals) select the compatible
compositor and frontend by default, including RemoteDesktop and Clipboard.
Portable installations can select the
[desktop-sharing component](docs/desktop-sharing.md) explicitly.
The [Print backend in development](docs/print-portal.md) has Rust/Iced consent and
isolated document workers. `--experimental-print` enables it in the normal backend
for explicit integration; normal packaged routing awaits its remaining acceptance.
`--printer-jobs` opens its Iced review to sign in to an existing held job without
sending the document again.

Follow the [shell guide](crates/canopa-shell/README.md) and [notification guide](crates/canopa-notify/README.md)
for feature options, using root `target/` paths. Existing Niribar installations
should follow the [shell rename guide](docs/shell-rename.md) before updating.

Build a release with `bash scripts/cargo.sh build --workspace --bins --locked --release`, then run
`bash scripts/package-release.sh target/release dist`. The archive contains all fifteen
binaries, current example configurations, service units, manuals, and source license
notices. The companion user services use `~/.local/bin`; `canopa-session.target` starts
Shell, Notify, Wallpaper and the logind lock listener. Install the lockscreen PAM service before using Lock;
see [session tools](docs/session-tools.md) before enabling them. Packaging does not
install or restart anything.
Check the archive with `bash scripts/smoke-release.sh dist/canopa-x86_64-unknown-linux-gnu.tar.gz`.
The release workflow also includes the [private Print client runtime](docs/print-runtime.md)
and its original CUPS notices. Local packaging selects it explicitly with
`--cups-runtime target/cups-authentication-compat/canopa-build.json`; omitting that
option keeps system CUPS. Print remains opt-in.
The optional desktop-sharing companions are selected together with
`--desktop-niri NIRI_BUILD/canopa-build.json --desktop-frontend FRONTEND_BUILD/canopa-build.json`.
Their package includes the extended descriptor, routing fragment and frontend
service override described in the [desktop-sharing guide](docs/desktop-sharing.md).
Append `--extra-portals` to include InputCapture, GlobalShortcuts and Usb, which
the pinned Niri fork provides; verify that selection with `--require-extra-portals`
on the archive smoke check.

For subsequent updates of an existing local companion installation, run:

```sh
./scripts/update.sh
```

This builds the current checkout, including uncommitted changes, and installs
`canopa-shell`, `canopa-wifi-provider`, `canopa-audio-provider`, `canopa-media-provider`, `canopa-notify`, `canopa-files`, `canopa-settings`, and `canopa` together. It does
not fetch or pull Git commits. The updater requires Python 3, both companion user
services, their standard `~/.local/bin` executable paths, and default configuration,
control socket and state discovery in the same graphical-session environment.
Custom service arguments or environment overrides selecting other paths require
a manual update.
Running Files windows and transfers continue in their existing process; the next
launch after that process exits uses the new executable. Initial Files launcher
registration follows the separate [installation guide](docs/files.md).

The new binaries validate your configuration before either service is stopped.
The updater preserves configuration and service definitions, backs up the previous
binaries, shared config and stopped notification state under
`$XDG_STATE_HOME/canopa-backups/` (default `~/.local/state/canopa-backups/`), and
restarts running or failed services. Intentionally stopped services stay stopped. Startup checks include
Notify's renderer status and the identity of each running executable. Failed
startup restores the previous binaries and notification state; state produced by the failed
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

Wallpaper and lockscreen are provided by `canopa-wallpaper` and `canopa-lock`.
See [session tools](docs/session-tools.md) for live wallpaper controls, PAM setup,
lock preview, themes and shared widgets. The wallpaper picker supports per-monitor
previews and saved assignments; shared authentication input supports layouts and
accessibility. Optional [Stasis integration](docs/idle.md) coordinates automatic
locking, sleep readiness and shell idle controls.

Canopa Greeter provides a login screen backed by greetd, sharing the lockscreen
presentation, automatic user/session discovery and selected-user avatars. See [greeter setup](docs/greeter.md) for the dedicated account, Cage/Weston
configuration and system installation.

Canopa Polkit supplies graphical action-authorization dialogs using the system
polkit backend. See [agent setup](docs/polkit.md) for session integration and testing.

Canopa Pinentry provides on-demand GnuPG PIN/passphrase and confirmation dialogs,
using shared session presentation. See [pinentry setup](docs/pinentry.md). The optional `pinentry` configuration namespace owns
`font` and `auth-timeout`; `canopa config check` validates it with the other owners.
