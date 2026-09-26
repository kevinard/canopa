# Shared upstream dependency forks

Canopa maintains one copy of each patched dependency in root `vendor/`. Both
applications use the root Cargo patches and lockfile. The stable, non-yanked versions
below were checked directly against the official crates.io sparse index on 2026-09-26.

| Crate | Release | Upstream revision |
| --- | --- | --- |
| iced_layershell | 0.19.1 | `5e39311c8c5e652bd493a8f78e68e62b8929d9cf` (`v0.19.1`) |
| layershellev | 0.19.1 | `5e39311c8c5e652bd493a8f78e68e62b8929d9cf` (`v0.19.1`) |
| accesskit_unix | 0.24.0 | `ce8164ba92995cfa86005b6259115e08c8244253` (crate VCS metadata) |

The two layer-shell crate source trees were compared byte for byte with their tag
subdirectories, excluding Cargo-generated manifests/lockfiles and mapping
`Cargo.toml.orig` to the upstream manifest. Their MIT license is copied from that
tag. AccessKit's MIT and Apache license texts were retrieved from its recorded
revision. Published archive SHA-256 values match the official registry checksums.

[Machine-readable provenance](../vendor/provenance.json) records exact download URLs,
archive checksums, and upstream/local hashes for every path. [Patches](../vendor/patches/)
show source changes relative to the published crates. `Cargo.lock` is omitted from
vendored packages because the root lockfile governs the build; licenses and the
AccessKit patch explanation are packaging/documentation additions, recorded separately
in the inventory. No nested source checkout is required to build the local forks.

Retained behavior:

| Patch | Why it remains | Verification |
| --- | --- | --- |
| Named Wayland output snapshots | The shell plans Bar Surfaces from output names even before a window exists; upstream does not expose this snapshot to application messages. | Shell eligible-output/surface reconciliation tests and compositor output checks. |
| Activation token context | Banner actions need the originating surface and recent input serial on the existing connection. | Notification activation tests and isolated desktop action checks. |
| Retired-window cleanup | Compositor output loss may remove a backend unit before Closed is delivered; completed closes must not stay in the adapter retry queue. | The retained vendor adapter regression plus notification surface lifecycle tests. |
| AT-SPI cache signal wrapping | AddAccessible and RemoveAccessible require one structure argument; upstream 0.24.0 still sends unwrapped structures. | Actual native bus signature/accessibility tests in both applications. |

Removed local patches:

- Popup grab serial tracking and popup dismissal: upstream 0.19.1 supplies them.
  Niribar uses the upstream position constructor with explicit anchor/gravity to
  preserve its calculated top-left placement.
- Notify's `OutputOption::Compositor`: upstream's `Active` layer-surface option now
  passes a null output, so the compositor selects placement directly.
- Niribar's vendored-manifest sibling path patch: the root workspace patch supplies
  the common layershellev source.
- The second AccessKit and generic panel protocol snapshots: each has one source.

Run `python3 scripts/check-vendor.py` for offline file and patch integrity. With
downloaded `NAME-VERSION.crate` files, use `--archives DIRECTORY` to also verify the
published archive checksums and contents. The checker performs no downloads. For an
intentional upgrade, inspect old patches against the new upstream source, remove
upstreamed behavior, update application calls, and regenerate the inventory and
patches together. Never update hashes just to suppress an unexpected difference.

Run `bash scripts/check.sh` and `bash scripts/check.sh --core`, followed by the
isolated desktop fixtures. Compilation alone does not establish popup grabs,
activation, output removal, fractional scaling, physical input regions, or native
screen-reader behavior. The experimental rotation prototype's vendored renderer
remains outside the supported workspace.

After building root debug binaries, run these fixtures sequentially:

```sh
python3 notify/scripts/test-desktop.py --notify target/debug/canopa-notify --niribar target/debug/niribar --artifacts /tmp/canopa-notifications-new
python3 scripts/test-fork-desktop.py --niribar target/debug/niribar --artifacts /tmp/canopa-forks-new
python3 niribar/scripts/verify-overlay-placement.py --binary target/debug/niribar --output /tmp/canopa-placement-new --edge top bottom left right --scale 1.25
```

Each artifact directory must be new. The first and third fixtures use Niri; the
second uses disposable Sway outputs to remove and restore every output without
terminating the compositor. It checks actual rendered frames and Wayland popup
grabs/dismissal. These are local adapter checks; Orca and physical monitor/device
acceptance remain separate procedures.

`scripts/test-vendor.py` runs library tests from temporary copies because Cargo
cannot test an excluded dependency's development targets in the root workspace.
It seeds the temporary lockfile from the root lockfile, gives the upstream Iced
test dependency the combined application's features, and rejects any active
registry dependency version absent from the root lockfile. Source files and
tracked vendor manifests are unchanged by this fixture.
