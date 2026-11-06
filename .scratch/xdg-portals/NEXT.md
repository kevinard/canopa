# Next portal slices

Prepared 2026-10-29 while the AppChooser shared gate runs. No additional interface
is implemented or advertised by this note.

## Access

The [Access wire contract](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.impl.portal.Access.html)
has one method, `AccessDialog(o,s,s,s,s,s,a{sv}) -> (u,a{sv})`, with title,
subtitle, body, modal, deny/grant labels, symbolic icon and the same serialized
choices used by FileChooser. It has no published version property.

Use the shared Requests pool and process lifecycle, with a private Iced helper.
Extract the existing choice parser/result validation into a portal-local common
module rather than copying it. Grant returns validated choices; Deny, Escape,
Close and caller loss return cancellation with no grant. The public frontend
retains permission storage and resource policy. Do not implement permission
persistence in the dialog backend.

Use plain text, a bounded symbolic icon prepared on a worker, a scrollable body
and choices, and always-visible Deny/Grant actions. Grant must require deliberate
action; an unhandled initial Enter must not grant permission. Preserve title and
application context without treating caller text as markup.

Verification: exact wire argument/result types; bool/combo choices; malformed
options; Close/owner loss/concurrency through the common host; grant and deny by
real input; long text/narrow window; dark/100% and light/125% captures; ensure
opening/Enter/closing alone never returns grant. Start with backend-native checks,
then exercise a real frontend consumer independently. A successful Access dialog
alone does not establish camera/device capture or permission persistence.

## Later Niri capture adapter

The repository's verified Niri 26.04 source overlay includes an existing Rust
`org.gnome.Mutter.ScreenCast` service implemented by Niri itself. A future Rust
portal can talk to that compositor service without running the GNOME portal.
This is an architectural route, not a working Canopa ScreenCast claim.

Primary source at pinned commit `8ed0da44d974c32c6877d2f4630c314da0717ecb`:

- [ScreenCast service](https://github.com/YaLTeR/niri/blob/8ed0da44d974c32c6877d2f4630c314da0717ecb/src/dbus/mutter_screen_cast.rs):
  `CreateSession`, session `RecordMonitor`/`RecordWindow`, `Start`/`Stop`, `Closed`,
  and stream `PipeWireStreamAdded`. Version 4, cursor Hidden/Embedded/Metadata;
  RecordWindow uses the Niri window ID. Remote-desktop session coupling is
  explicitly rejected. Retain one compositor session per authorized portal
  session; stop it on Close, owner loss, failed startup and compositor loss.
- [D-Bus registration](https://github.com/YaLTeR/niri/blob/8ed0da44d974c32c6877d2f4630c314da0717ecb/src/dbus/mod.rs):
  interfaces normally start for a session instance. The debug flag
  `dbus-interfaces-in-non-session-instances` enables capture-related interfaces
  for a private non-session test, avoiding a live session's service ownership.
  ScreenCast is conditional on the `xdp-gnome-screencast` build feature.
- [Capture startup](https://github.com/YaLTeR/niri/blob/8ed0da44d974c32c6877d2f4630c314da0717ecb/src/screencasting/mod.rs):
  `prepare_pw_cast` requires a GBM device before initializing PipeWire. Existing
  software-rendered nested fixtures therefore cannot establish real media flow;
  plan a separate GBM-backed native acceptance environment.
- [Screenshot service](https://github.com/YaLTeR/niri/blob/8ed0da44d974c32c6877d2f4630c314da0717ecb/src/dbus/gnome_shell_screenshot.rs):
  Niri implements screenshot and color-picking calls itself. A Canopa adapter
  still needs explicit consent/selection, result-file validation/ownership,
  cancellation and a tested relationship to Niri's own selection UI.

Before implementation, verify the installed compositor's actual interfaces and
build features on a disposable bus. Do not infer RemoteDesktop/InputCapture from
ScreenCast support, or advertise stream support based only on D-Bus signatures.


### Runtime capability probe

`/tmp/canopa-niri-portal-capabilities-02/result.json` records a passing probe on
installed Niri in a disposable Weston/Niri session and private D-Bus. The actual
ScreenCast service reports version 4. An empty session can be created and stopped;
after Stop, Introspect returns UnknownObject, proving object retirement. The
screenshot interface also introspects successfully. XML snapshots are retained
in that directory. `capabilities-01` initially treated this correct UnknownObject
response as a fixture failure; the corrected probe distinguishes it explicitly.
No frames, PipeWire stream, permission decision or live desktop capture was
requested, so this is capability/lifecycle evidence only.
