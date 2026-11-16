# Releasing and packaging

Canopa ships two ways: Arch Linux packages (the supported system installation)
and a portable release archive with companion user services. End-user
installation is described in [installation](installation.md); this page is for
maintainers building them.

## Versioning

Every crate is at version `0.1.0`; the packages take their version from
`crates/canopa-settings/Cargo.toml`. A release is a tag
`vX.Y.Z` on `main`, which triggers the [release workflow](ci.md#release). There is
no changelog or configuration-compatibility policy yet; see
[issue #43](https://github.com/kevinard/canopa/issues/43).

## Release archive

```sh
bash scripts/cargo.sh build --workspace --bins --locked --release
bash scripts/package-release.sh target/release dist
bash scripts/smoke-release.sh dist/canopa-x86_64-unknown-linux-gnu.tar.gz
```

`package-release.sh BUILD_DIR OUTPUT_DIR` stages all fifteen binaries, the
systemd user units and `canopa-session.target`, D-Bus service files, portal
descriptors, desktop entries, the greetd and PAM samples, the man page, example
configurations, the user documentation (with links rewritten for the installed
layout by `scripts/package-documentation.py`), source licenses and the generated
dependency attributions. It then runs `--version`, `--help` and configuration
checks on the staged binaries. It installs and restarts nothing.

Optional components:

| Option | Adds |
| --- | --- |
| `--cups-runtime RECEIPT` | the private CUPS client used by the opt-in Print portal (`lib/canopa/print/`); see [printing](printing.md) |
| `--cups-ipp-backend RECEIPT` | the matching IPP backend (requires `--cups-runtime`) |
| `--desktop-niri RECEIPT --desktop-frontend RECEIPT` | the forked compositor and portal frontend for screen sharing, with the extended portal descriptor, routing fragment and frontend service override; see [desktop sharing](desktop-sharing.md) |
| `--extra-portals` | with the desktop-sharing companions, also advertise InputCapture, GlobalShortcuts and Usb |

Each `RECEIPT` is the `canopa-build.json` written by the corresponding builder
(`scripts/build-cups-authentication.py`, `build-print-ipp-backend.py`,
`build-niri.py`, `build-xdg-portal-compat.py`); packaging accepts only receipts of
the pinned fork commits. `smoke-release.sh ARCHIVE` checks the archive contents
and documentation links; add `--require-print-runtime`, `--require-print-backend`,
`--require-desktop-sharing` or `--require-extra-portals` to require the optional
parts. It needs `desktop-file-validate`.

The tag workflow builds the archive with the Print runtime and backend; it does
not include the desktop-sharing companions.

## Arch packages

```sh
python3 scripts/package-arch.py --output NEW_DIR          # source package only
python3 scripts/package-arch.py --output NEW_DIR --build  # also run makepkg
```

`package-arch.py` creates a checksummed source tarball from the files Git tracks
(plus untracked, non-ignored files), renders `packaging/arch/PKGBUILD.in`, writes
`.SRCINFO`, and with `--build` runs `makepkg` under the free-space guard without
installing anything or fetching dependencies. The recipe produces four packages:

| Package | Contents |
| --- | --- |
| `canopa` | every Canopa program in `/usr/bin`, the session launcher `canopa-session`, user units (without `[Install]` sections; `canopa-session.target` starts them), portal descriptor and routing, the forked portal frontend at `/usr/lib/canopa/xdg-desktop-portal`, PAM file for the lockscreen, documentation |
| `canopa-compositor` | the forked Niri at `/usr/lib/canopa/niri` |
| `canopa-login` | greetd configuration for Canopa Greeter under the bundled Niri (Cage and Weston samples too), the `canopa-greeter` system user, the login display helper and its polkit action |
| `canopa-oo7` | optional oo7 secret-service integration for the login keyring |

`build()` honours `CARGO_BUILD_JOBS` (default 1), `CANOPA_PACKAGE_TARGET_DIR`
(default `$PWD/target`) and `CANOPA_NIRI_WORK_DIR`. The file layout is staged by
`scripts/install-system.py --destdir DIR --component desktop|compositor|login|oo7`,
which refuses to write to `/` and records fork provenance under
`/usr/share/canopa/`.

## Moving a fork pin

The compositor, portal frontend and CUPS components are pinned in
`packaging/forks.json` and in `packaging/arch/PKGBUILD.in`
(`_niri_commit`, `_portal_commit`, `sha256sums`); `scripts/test-fork-sources.py`
keeps them in step. Follow [fork maintenance](vendor-maintenance.md#separately-built-programs).

## Before tagging

1. `bash scripts/check.sh --workspace` and `bash scripts/check.sh --core` pass.
2. The [desktop acceptance](ci.md#desktop-acceptance) workflow passes against the
   compositor you ship.
3. `docs/dependency-attributions.md` is current (the workspace gate checks it;
   regenerate with `python3 scripts/dependency-attributions.py --output docs/dependency-attributions.md`).
4. A local `package-release.sh` + `smoke-release.sh` and `package-arch.py --build`
   succeed.
