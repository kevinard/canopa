# 0129: Supported Canopa session and Settings

Status: accepted; physical installation acceptance remains separate.

The shortcut materialization below is superseded by
[ADR 0135](0135-shortcut-changes-over-shipped-defaults.md), which stores shortcut
changes as a delta over the shipped defaults. The page registry, page patterns
and the single-window D-Bus contract are in
[ADR 0136](0136-settings-pages-patterns-and-activation.md).

Canopa Settings is an ordinary independently launched Iced application, sharing
appearance and controls. It does not own compositor, wallpaper, authentication or
idle lifetimes. Existing owners validate and apply their own configuration.

The system installation has a dedicated Canopa compositor user unit/session
launcher. Companion units use system binary paths and start only as part of that
session. Existing per-user companion installation remains supported separately.
Packaging selects the pinned combined desktop-sharing compositor at a private
path, including the Files drag fixes from ADR0024. The Canopa session selects
Canopa portal backends, with a separate Notify backend, GTK for unimplemented
interfaces, and a keyring provider for Secret. The packaged descriptor enables
ScreenCast, RemoteDesktop and Clipboard. The systemd portal service dispatches
to the pinned frontend inside the Canopa session and the distribution frontend
in other sessions. It uses the launcher's session marker and
`XDG_SESSION_DESKTOP`, since Niri publishes its own `XDG_CURRENT_DESKTOP`.
The private frontend restores the Canopa desktop name for its routing lookup.
The session retires stale portal processes before starting
the compositor so they pick up the new environment. Packaging installs the
WirePlumber policy from ADR0112 to preserve frontend-owned grants.

Canopa's Niri entry configuration lives under the user's Canopa configuration
directory, with separate managed display and input includes. Existing Niri
configuration remains untouched. Custom compositor options live in a separate
user include. Settings refuses to imply persistence in an unrelated Niri session.

Display testing is owned by a separate short-lived helper process. It snapshots
the selected compositor/outputs, applies temporary IPC changes and requires an
explicit Keep decision before saving. Timeout, GUI disconnect and errors restore
the previous output configuration. The helper serializes trials, binds output
identities and keeps rollback alive when the settings window crashes. Niri owns
all actual output application; configuration is validated before persistence.

Appearance/lockscreen changes preserve other shared-file namespaces and validate
with their owning schemas. Wallpaper uses its existing picker/runtime. Idle
controls delegate to Canopa Idle/Stasis; native Rune remains the policy source.
Edits preserve custom policy and reject ambiguous or concurrently changed input.

Settings edits the packaged shortcut set in the Canopa session's main Niri
configuration. The first explicit save materializes the known defaults in
place so a reassigned key stops invoking the previous action. Custom includes
retain their precedence and contents. Shortcut edits and the removable Niri
animation override each use a validated main-file replacement and compositor
reload. Shell panel motion and shared text scaling are independent appearance
preferences in the common Canopa configuration.

Quick settings opens Bluetooth and power controls as built-in v5 providers.
BlueZ owns device discovery and pairing. Canopa registers an agent only for
its selected pairing attempt, and closing the provider releases its connection
and cancels unfinished pairing. logind controls the built-in backlight and
Power Profiles daemon owns system power policy.

The optional oo7 prompter owns a single D-Bus name on the user bus. It uses
GCR for the prompt protocol and encrypted credential exchange. The Canopa
session launcher publishes its PID so the prompter can resolve its logind
session at prompt time. Native presentation requires that session to be the
user's active graphical display and verifies the Wayland socket against the
Canopa compositor unit's PID. The prompt worker receives that connected socket
and reuses the Pinentry dialog and bounded secret input. Other desktops use
GCR's GTK dialog. The user installs an activation link to opt into this routing.

Arch is the first packaging target. Private staging and nested acceptance do not
establish real DRM/VT, suspend, package-manager upgrade or distribution PAM
acceptance; those outcomes must retain their own evidence.
