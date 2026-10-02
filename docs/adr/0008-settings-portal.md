# 0008: Settings portal backed by shared appearance

Status: accepted for the first portal interface (2026-09-27).

The shell restart requirement below is superseded by
[Shell ADR 0021](../shell/adr/0021-live-configuration-reload.md). The portal retains
its own explicit reload contract.

The first Canopa portal backend implements Settings. It exposes the appearance
already used by Niribar and Notify to applications through the existing XDG portal
frontend. The common file remains authoritative, and the backend has no UI or
shell runtime dependency. It needs no new configuration namespace. Other portal
interfaces remain with the selected external backends.

The workspace crate `canopa-portal` builds `xdg-desktop-portal-canopa`, an independent
D-Bus session process. It owns `org.freedesktop.impl.portal.desktop.canopa` and serves
`org.freedesktop.impl.portal.Settings` at `/org/freedesktop/portal/desktop`. The
backend implements version 1, `Read`, filtered `ReadAll`, and `SettingChanged`.
The frontend owns the public interface, including its version-2 `ReadOne` method;
Canopa does not claim the frontend's bus name or duplicate its routing.

Only two keys are published in `org.freedesktop.appearance`: `color-scheme` is an
unsigned integer (dark 1, light 2); `accent-color` is the effective palette RGB
normalized to three doubles in [0,1]. When the user has no explicit accent, this
reports the same preset accent used by the shell and banners. Contrast and reduced
motion are absent until Canopa has settings and consumers that honor them. Unknown
keys and namespaces produce `org.freedesktop.portal.Error.NotFound` with a constant
message. `ReadAll` supports empty filters, exact names, `*`, and trailing namespace
sections such as `org.freedesktop.*`.

`canopa-config::SharedConfiguration` provides bounded shared intake without choosing
a feature namespace. Discovery uses the Canopa file or shared defaults, and never
consults either application's legacy file. Existing application discovery preserves
its old precedence through the same candidate-discovery helper. The common reader,
KDL parser, envelope validator, appearance parser, diagnostics and fingerprint logic
are reused. Invalid unrelated feature options remain each owner's responsibility;
invalid syntax, envelope or appearance rejects the shared candidate. A dangling
symlink remains a present invalid source. Explicit legacy input is rejected.

Startup rejects invalid configuration before owning the D-Bus name. SIGHUP prepares
a candidate off the async executor, serially installs the complete snapshot, and
emits a signal for each changed effective value. No-op reloads emit nothing; failed
reloads retain the old values and log only safe diagnostics. Discovery is repeated
on reload, so creating or removing the common file selects it or the shared defaults.
An explicitly selected path stays selected and a missing file is a rejected reload.
There is no file watcher or global atomic reload promise. The shell remains
restart-required and Notify keeps its existing reload interface.

The process fails rather than replacing an existing owner and exits on session-bus
loss, SIGTERM or SIGINT. The object server serializes installation and reads; domain
state and an application-facing write interface are absent.

Packaging includes the fifth executable, a Settings-only `.portal` descriptor,
D-Bus activation metadata, and an on-demand systemd user unit. The companion target
still starts only the shell and Notify. Installation and backend selection are
explicit; no build, test or binary writes the user's portal configuration. Existing
backend choices for other interfaces are retained. The supplied systemd unit uses
`~/.local/bin` consistently with the companions; direct D-Bus activation uses PATH.
Distributors may replace these executable locations for their chosen prefix.

Upstream contracts checked during this implementation:

- [Settings backend](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.impl.portal.Settings.html)
- [Public Settings interface](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Settings.html)
- [Backend metadata and activation](https://flatpak.github.io/xdg-desktop-portal/docs/writing-a-new-backend.html)
- [Per-interface backend selection](https://flatpak.github.io/xdg-desktop-portal/docs/portals.conf.html)

This extends ADRs 0002 and 0003. Interactive portal request lifetimes, parent windows,
cancellation and queuing remain separate work for the first interface needing UI.

See [validation evidence](../settings-portal-validation.md).
