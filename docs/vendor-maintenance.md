# Vendored dependency maintenance

The root Cargo manifest patches `iced_layershell`, `layershellev`, and
`accesskit_unix` to local sources. `canopa-panel-protocol` is a path dependency.
These sources are part of the standalone build; do not remove the patches as
routine dependency cleanup.

- `vendor/accesskit_unix/LOCAL-PATCHES.md` records the accessibility changes and
  their upgrade checks.
- `vendor/iced_layershell` and `vendor/layershellev` are the 0.18.0 layer-shell
  integration used by the bar. Repository history records local changes to
  popup/window behavior and keyboard handling. Before upgrading, compare the
  replacement with the vendored tree and retain the local behavior; the
  original upstream revision and a complete upstream-relative patch inventory
  have not yet been established.
- `vendor/canopa-panel-protocol` is the shared Canopa wire schema. Follow its
  README and the provider guide for the source-drift check when upgrading it.

Run formatting, strict Clippy, and the full locked test suite after upgrades.
Then verify popup placement, keyboard focus, output removal, mixed scaling,
rotated content, and AT-SPI interaction in a real compositor session.

The rotation prototype and its independent renderer copies are exploratory
code, outside the root crate's test suite and release support.

The project MIT license applies to Niribar's code. Each third-party directory
retains its upstream license; this document is not a dependency-license audit.
