# Canopa source licenses

The combined repository preserves the original source licenses:

- Canopa Shell (originally Niribar), including its original scripts and documentation:
  MIT, in `crates/canopa-shell/LICENSE`.
- Canopa Notify, including its original scripts and documentation: MIT OR Apache-2.0,
  in `crates/canopa-notify/LICENSE-MIT` and `crates/canopa-notify/LICENSE-APACHE`.
- Canopa panel protocol: MIT, selecting the original v4 MIT option and retaining
  Notify and Niribar attribution in `crates/canopa-panel-protocol/LICENSE`.
  The original v4 license texts are also retained in its crate directory.
- Canopa local assets: MIT, with Notify and Niribar attribution in `crates/canopa-assets/LICENSE`.
- Canopa Files, configuration, appearance, UI, notification client, bounded file intake, portal, and CLI: MIT OR Apache-2.0,
  with both texts in their crate directories.
- Shared workspace tooling and integration documentation: MIT OR Apache-2.0,
  using the texts in `crates/canopa-notify/`.
- Vendored dependencies retain their upstream notices and license texts in each
  `vendor/` directory. See `docs/vendor-maintenance.md` for provenance.
- The optional Niri source-overlay recipe preserves Niri's GPL-3.0-or-later and
  Smithay's MIT texts in `vendor/source-overlays/niri-source-cancellation/`.
  Its companion executable is built separately from the Canopa applications.
- The optional private CUPS client preserves the upstream Apache-2.0 license and
  CUPS exceptions in `vendor/source-overlays/cups-authentication/LICENSE` and
  `NOTICE`. Bundles containing this modified client include those original texts,
  a modification summary and its complete source patches under
  `share/licenses/canopa-print-cups/`. Its system dependencies retain their
  distribution notices.
- The test-only CUPS IPP receiver overlay references those same original CUPS
  LICENSE/NOTICE assets. Its source patch is under
  `vendor/source-overlays/cups-ipp-receiver/`; its builder installs nothing.
- The optional matching CUPS IPP backend shares those original LICENSE/NOTICE
  assets. Bundles containing it retain them, a modification summary and complete
  `resolution.patch` under `share/licenses/canopa-print-ipp-backend/`. Staging the
  component configures no CUPS service or printer queue.

Moving code into the shared workspace does not change its existing license.
