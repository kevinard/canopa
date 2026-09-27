# Canopa source licenses

The combined repository preserves the original source licenses:

- Niribar, including its original scripts and documentation: MIT, in
  `crates/niribar/LICENSE`.
- Canopa Notify, including its original scripts and documentation: MIT OR Apache-2.0,
  in `crates/canopa-notify/LICENSE-MIT` and `crates/canopa-notify/LICENSE-APACHE`.
- Canopa panel protocol: MIT, selecting the original v4 MIT option and retaining
  Notify and Niribar attribution in `crates/canopa-panel-protocol/LICENSE`.
  The original v4 license texts are also retained in its crate directory.
- Canopa local assets: MIT, with Notify and Niribar attribution in `crates/canopa-assets/LICENSE`.
- Canopa configuration, appearance, UI, notification client, bounded file intake, portal, and CLI: MIT OR Apache-2.0,
  with both texts in their crate directories.
- Shared workspace tooling and integration documentation: MIT OR Apache-2.0,
  using the texts in `crates/canopa-notify/`.
- Vendored dependencies retain their upstream notices and license texts in each
  `vendor/` directory. See `docs/vendor-maintenance.md` for provenance.

Moving code into the shared workspace does not change its existing license.
