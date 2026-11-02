# 0036: Supported Canopa session and Settings

Status: accepted; physical installation acceptance remains separate.

Canopa Settings is an ordinary independently launched Iced application, sharing
appearance and controls. It does not own compositor, wallpaper, authentication or
idle lifetimes. Existing owners validate and apply their own configuration.

The system installation has a dedicated Canopa compositor user unit/session
launcher. Companion units use system binary paths and start only as part of that
session. Existing per-user companion installation remains supported separately.
Packaging selects ADR0024's pinned compatible compositor at a private path;
installation does not replace the distribution's Niri or enable greetd.

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

Arch is the first packaging target. Private staging and nested acceptance do not
establish real DRM/VT, suspend, package-manager upgrade or distribution PAM
acceptance; those outcomes must retain their own evidence.
