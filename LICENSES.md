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
  configuration, appearance, UI, translations and formatting (`canopa-i18n`),
  notification client, bounded file intake, portal,
  and CLI: MIT OR Apache-2.0,
  with both texts in their crate directories. The Fluent translation files
  under each crate's `i18n/` directory are under the same terms as the crate.
- Locale data compiled into the formatting crates (ICU4X with CLDR data) is
  Unicode-3.0, as recorded in `docs/dependency-attributions.md`.
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
- The compositor, portal frontend and optional CUPS components are built from
  the pinned GitHub forks in `packaging/forks.json`, whose sources keep their
  upstream licenses. Packaging copies the original texts from those fork
  sources, checked against the checksums pinned there:
  - The Arch `canopa-compositor` package is built from kevinard/niri and is
    GPL-3.0-or-later; it installs Niri's `LICENSE` and the MIT `LICENSE.txt` of
    kevinard/smithay (taken from Niri's locked Cargo git checkout) under
    `/usr/share/licenses/canopa-compositor/`, with the other dependency notices
    beside them. Its executable is built separately from the Canopa applications.
  - The patched portal frontend from kevinard/xdg-desktop-portal is
    LGPL-2.1-or-later; the `canopa` package installs its `COPYING` under
    `/usr/share/licenses/canopa/xdg-desktop-portal/`. Release bundles with the
    optional desktop-sharing companions carry the Niri, Smithay and portal
    texts under `share/licenses/canopa-desktop-sharing/`.
  - The optional private CUPS client from kevinard/cups keeps the upstream
    Apache-2.0 `LICENSE` and CUPS `NOTICE`. Bundles containing it include those
    original texts and a `CHANGES.md` naming the fork commit, its upstream base
    and the complete change set under `share/licenses/canopa-print-cups/`. Its
    system dependencies retain their distribution notices.
  - The optional matching CUPS IPP backend, built from the same fork commit,
    ships the same texts and a `CHANGES.md` under
    `share/licenses/canopa-print-ipp-backend/`. Staging it configures no CUPS
    service or printer queue. The test-only IPP receiver is built from that
    source too and is never installed.

Moving code into the shared workspace does not change its existing license.

The virtual-keyboard acceptance protocol in `scripts/desktop-fixture/` retains
its upstream MIT notice (Wayland/wlroots contributors). It is test infrastructure,
not part of the greeter executable.

Canopa Polkit links the distribution’s `libpolkit-agent-1` and `libpolkit-gobject-1`
(LGPL-2.0-or-later), GLib/GIO and the existing Wayland libraries. These system
libraries and the privileged polkit backend retain their distribution notices.
