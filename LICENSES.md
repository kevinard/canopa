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
- Launcher emoji and symbol data: [Unicode License v3](crates/canopa-shell/data/characters/LICENSE-UNICODE),
  with [source attribution](crates/canopa-shell/data/characters/README.md).
- Canopa widget data: MIT, preserving extracted Canopa Shell attribution in
  `crates/canopa-widgets/LICENSE`.
- Canopa launcher contracts, Files, Settings, session tools, session presentation, greeter, polkit agent, pinentry,
  configuration, appearance, UI, notification client, bounded file intake, portal,
  and CLI: MIT OR Apache-2.0,
  with both texts in their crate directories.
- Shared workspace tooling and integration documentation: MIT OR Apache-2.0,
  using the texts in `crates/canopa-notify/`.
- Canopa Shell and Canopa Settings link the GPL-3.0-or-later `niri-ipc` crate.
  Their source stays under its MIT or MIT OR Apache-2.0 terms, but distributed
  executables of those two applications are covered by GPL-3.0-or-later as a
  whole. The Arch `canopa` package declares that license accordingly.
- Forked dependencies keep their upstream licenses and notices in their
  GitHub forks (see `docs/vendor-maintenance.md`).
  `docs/dependency-attributions.md` names each fork crate by its fork URL and
  commit, and packaging collects its license texts like any other dependency.
- The Niri source-overlay recipes share Niri's GPL-3.0-or-later and Smithay's MIT
  texts in `vendor/source-overlays/niri-source-cancellation/`. The Arch
  `canopa-compositor` package builds the `niri-remote-touch` recipe and is
  GPL-3.0-or-later. Its executable is built separately from the Canopa applications.
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

The virtual-keyboard acceptance protocol in `scripts/desktop-fixture/` retains
its upstream MIT notice (Wayland/wlroots contributors). It is test infrastructure,
not part of the greeter executable.

Canopa Polkit links the distribution’s `libpolkit-agent-1` and `libpolkit-gobject-1`
(LGPL-2.0-or-later), GLib/GIO and the existing Wayland libraries. These system
libraries and the privileged polkit backend retain their distribution notices.
