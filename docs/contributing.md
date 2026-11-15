# Contributing

This page is the starting point for working on Canopa. Read
[architecture overview](architecture/overview.md) first, then the architecture
page and glossary of the area you are changing.

## Set up

1. Install Rust with rustup; `rust-toolchain.toml` selects Rust 1.95.0 with
   rustfmt and clippy.
2. Install the system libraries. On Arch, the `makedepends` of
   `packaging/arch/PKGBUILD.in` are the complete list. On Debian/Ubuntu:

   ```sh
   sudo apt-get install build-essential pkg-config ripgrep python3 \
     libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev xkb-data \
     libpam0g-dev libpolkit-agent-1-dev libpulse-dev libpipewire-0.3-dev libclang-dev \
     libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
     fonts-dejavu-core dbus at-spi2-core libglib2.0-bin python3-gi xdg-desktop-portal \
     pipewire pipewire-bin pipewire-pulse pulseaudio-utils
   ```

   The workspace needs PipeWire 0.3.64 or newer development files and libclang
   for the PipeWire bindings. `.github/workflows/check.yml` holds the exact package
   lists CI uses.
3. Build and run the default checks from the repository root:

   ```sh
   bash scripts/cargo.sh build --workspace --bins --locked
   bash scripts/check.sh
   ```

Always compile through `bash scripts/cargo.sh`; it stops the build before the disk
fills (see [building](building.md)).

## Repository layout

| Path | Contents |
| --- | --- |
| `crates/` | every workspace crate (applications and shared libraries); see the [component map](architecture/overview.md#component-map) |
| `config/`, `examples/` | the shipped default `canopa.kdl` and example configurations and themes |
| `packaging/` | systemd units, D-Bus services, portal descriptors, PAM and greetd files, the Arch recipe, fork pins (`forks.json`) |
| `scripts/` | build guard, check gates, packaging, the updater, and native test harnesses (`desktop-fixture/`) |
| `docs/` | user guides, developer guides, [architecture](architecture/overview.md), glossaries and decision records |
| `.github/workflows/` | [CI](ci.md) |

## Workflow

- **Issues** live on GitHub: <https://github.com/kevinard/canopa/issues>. Each has a
  type label (`bug`, `enhancement`, `documentation`, `accessibility`,
  `verification`) and an `area: …` label. `verification` issues are checklists of
  implemented behaviour that still needs a hands-on check on real hardware.
- **Branches and commits**: branch from `main`; keep commits focused, with a
  subject in the imperative ("Keep attributes when copying in Files").
- **Tests**: run the focused tests for the code you change while you work
  ([testing](testing.md)). Run `bash scripts/check.sh --workspace` once at the end
  for changes to shared crates, forks, tooling or several applications. Record
  which checks you ran in the pull request.
- **Formatting and lints**: `cargo fmt --all` and
  `cargo clippy --workspace --all-targets --locked -- -D warnings` must be clean.
- **User-visible strings** go through Fluent; `python3 scripts/check-i18n.py`
  rejects literals in UI code. See [translations](translations.md).
- **Dependency forks**: patched upstream code lives in GitHub forks, never in this
  repository. Follow [fork maintenance](vendor-maintenance.md).

## Documentation

Documentation describes the code as it is now. Update it in the same change as
the behaviour:

- user guides (`docs/*.md`, listed in [the index](README.md)) for anything a user
  or packager sees: commands, flags, configuration keys, files, units;
- the [configuration reference](configuration.md) for every config key;
- the area's [architecture page](architecture/overview.md) for structural changes;
- the area's glossary (`docs/shell/CONTEXT.md`, `docs/notify/CONTEXT.md`,
  `docs/files/CONTEXT.md`) for new domain terms.

Do not commit validation records, test output, measurement logs or screenshots of
runs; put those in the pull request or issue instead. The user guides are packaged
into releases by `scripts/package-documentation.py`, which rejects links to files
that don't exist; add a new guide to its `FILES` list.

## Decision records

Record a significant decision (a new ownership boundary, protocol, persistence
format or a rejected alternative that people will propose again) as a short
decision record in [`docs/adr/`](adr/README.md), and update the affected
architecture page. Records state context, decision and consequences; they carry
no progress status, test counts or evidence.
