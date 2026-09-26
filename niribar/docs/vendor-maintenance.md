# Vendored dependency maintenance

The root Cargo manifest patches `iced_layershell`, `layershellev`, and
`accesskit_unix` to local sources. `canopa-panel-protocol` is a path dependency.
These sources are part of the standalone build; do not remove the patches as
routine dependency cleanup.

- `vendor/accesskit_unix/LOCAL-PATCHES.md` records the accessibility changes and
  their upgrade checks.
- `vendor/iced_layershell` and `vendor/layershellev` are based on the published
  0.18.0 crates. Their verified source baseline and complete local change
  inventory are recorded below and in `vendor/provenance.json`.
- `vendor/canopa-panel-protocol` is the shared Canopa wire schema. Follow its
  README and the provider guide for the source-drift check when upgrading it.

Run formatting, strict Clippy, and the full locked test suite after upgrades.
Then verify popup placement, keyboard focus, output removal, mixed scaling,
rotated content, and AT-SPI interaction in a real compositor session.

## Layer-shell source baseline

Upstream repository: <https://github.com/waycrate/exwlshelleventloop>.
Both published manifests declare MIT licensing. The retained `LICENSE` files
were added separately in repository commit `e4280a1`; neither published archive
includes a license file or `.cargo_vcs_info.json`. Consequently, no original
Git commit is asserted. The immutable crate version and archive checksum are
the reproducible baseline.

| Published archive | SHA-256 |
| --- | --- |
| [iced_layershell 0.18.0](https://static.crates.io/crates/iced_layershell/iced_layershell-0.18.0.crate) | `e9af23def8028c786e97b72dc3b596968ad23bca4faa865ec96a134ffe3d71ca` |
| [layershellev 0.18.0](https://static.crates.io/crates/layershellev/layershellev-0.18.0.crate) | `aa93feb9ad2defd5a7420460a8491649cb81709d84cb3759135069624f8ee9a1` |

On 2026-09-23, downloaded archives matched the respective Cargo registry index
checksums. Every archive file was compared with the local snapshot. The JSON
manifest records upstream and local SHA-256 values for the union of paths;
`null` means absent. Unchanged files are included, so unnoticed modifications,
additions, and removals also fail verification.

### Complete local change inventory

Paths below are relative to each vendored crate.

| Crate / path | Change from the published archive |
| --- | --- |
| Both: `Cargo.lock`, `Cargo.toml.orig` | Omitted; the root lockfile and normalized manifests govern this build. |
| Both: `LICENSE` | Added MIT license text with upstream copyright notice. |
| iced_layershell: `Cargo.toml` | Adds the sibling `layershellev` path patch. The root manifest also supplies the effective workspace patch. |
| iced_layershell: `src/actions.rs` | Adds the popup `grab` setting. |
| iced_layershell: `src/event.rs` | Defines named `WaylandOutputSnapshot` events and separates them from window events. |
| iced_layershell: `src/lib.rs` | Exports `WaylandOutputSnapshot`. |
| iced_layershell: `src/build_pattern/application.rs`, `src/build_pattern/daemon.rs` | Requires application messages to accept output snapshots. |
| iced_layershell: `src/multi_window.rs` | Routes output snapshots into application messages and forwards popup grab settings, including menu grabs. |
| layershellev: `src/events.rs` | Adds popup grab configuration and its test, plus internal/public output snapshot events and conversion. |
| layershellev: `src/lib.rs` | Emits sorted, deduplicated output names on name/removal changes; handles popup dismissal; requests grabs with an available input serial. |
| layershellev: `src/seat.rs` | Remembers the pointer press serial used by popup grabs. |

No other local file differs from these release archives. In particular, existing
clipboard opt-out and keyboard conversion code is upstream code, not a local
patch. Repository commits `3f8f303` and `3cfbedf` record the popup and output work.

### Verification and upgrades

Offline CI and local verification:

```sh
python3 scripts/check-vendor.py
```

To also verify the upstream side, download the two archive links above into a
directory, retaining `NAME-VERSION.crate` filenames, then run:

```sh
python3 scripts/check-vendor.py --archives /path/to/archives
```

The checker never downloads or extracts archives. For an intentional vendor
change, inspect the upstream-relative diff, update this inventory and the
affected local hashes in `vendor/provenance.json`, and review those together.
For an upgrade, verify the replacement archive against the Cargo registry
checksum, regenerate both sides of the file inventory, and port only patches
still needed. Do not update hashes merely to silence an unexpected CI failure.

The rotation prototype and its independent renderer copies are exploratory
code, outside the root crate's test suite and release support.

The project MIT license applies to Niribar's code. Each third-party directory
retains its upstream license; this document is not a dependency-license audit.
