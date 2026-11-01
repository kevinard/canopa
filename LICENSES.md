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
- Canopa widget data: MIT, preserving extracted Canopa Shell attribution in
  `crates/canopa-widgets/LICENSE`.
- Canopa Files, session tools, session presentation, greeter, polkit agent, pinentry,
  configuration, appearance, UI, notification client, bounded file intake, portal,
  and CLI: MIT OR Apache-2.0,
  with both texts in their crate directories.
- Shared workspace tooling and integration documentation: MIT OR Apache-2.0,
  using the texts in `crates/canopa-notify/`.
- Vendored dependencies retain their upstream notices and license texts in each
  `vendor/` directory. See `docs/vendor-maintenance.md` for provenance.
- The optional Niri source-overlay recipe preserves Niri's GPL-3.0-or-later and
  Smithay's MIT texts in `vendor/source-overlays/niri-source-cancellation/`.
  Its companion executable is built separately from the Canopa applications.

Moving code into the shared workspace does not change its existing license.

The virtual-keyboard acceptance protocol in `scripts/desktop-fixture/` retains
its upstream MIT notice (Wayland/wlroots contributors). It is test infrastructure,
not part of the greeter executable.

Canopa Polkit links the distribution’s `libpolkit-agent-1` and `libpolkit-gobject-1`
(LGPL-2.0-or-later), GLib/GIO and the existing Wayland libraries. These system
libraries and the privileged polkit backend retain their distribution notices.
