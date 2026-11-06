# Lockdown preparation

Status: implemented and advertised. Nine native groups pass with the shipped
descriptor and activation service in dark/100% and light/125%.
ADR0036 records the portal-owned runtime policy source. The investigation below
preceded that decision and remains evidence of the alternatives considered.

The [official interface](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.impl.portal.Lockdown.html)
and installed XML expose seven boolean properties, all marked **readwrite**:
`disable-printing`, `disable-save-to-disk`, `disable-application-handlers`,
`disable-location`, `disable-camera`, `disable-microphone` and
`disable-sound-output`. There is no version property in this XML. The original
roadmap's shorthand about read-only reporting is insufficient to specify the
wire contract or policy ownership.

The [current GNOME source](https://github.com/GNOME/xdg-desktop-portal-gnome/blob/main/src/lockdown.c)
binds these properties to desktop lockdown, location and privacy settings.
Bindings use default bidirectional behavior, with location inverted. A Rust
replacement should define both observation and mutation ownership, persistence,
change publication and behavior when the selected policy source is not writable.
Constant false values or temporary in-memory setters would not replace that
source-backed behavior.

Canopa's current shared configuration envelope permits shell, notifications,
appearance and files namespaces only. Its Component enum has the three existing
application owners; SharedConfiguration exposes appearance and source metadata.
There is no portal policy owner yet. Reusing GNOME GSettings would retain a
desktop dependency the project is replacing. A portal-owned namespace/source
needs an integration decision before this interface is advertised.

Investigate a first-class Canopa portal policy source using the existing bounded
KDL intake and source/fingerprint model. Define explicit defaults, source
discovery, startup rejection, reload retention, persistence and concurrent-file
edit behavior. Property Set must commit to its authoritative writable source
before publishing a changed value; a non-writable administrative source needs
an explicit permission failure. Confirm the frontend's property-consumption and
change-signal behavior against the installed version before choosing the seam.

Acceptance should cover all seven exact property names/types/access annotations,
Get/GetAll/Set, changed-value signals, no-op changes, persistence/restart,
concurrent edits, invalid/rejected reloads and read-only-source failures. Exercise
real frontend enforcement with private configuration and harmless operations,
including a save/handler policy affecting the existing portals. Shared config
changes require the cross-application gate. Tests must never change the live
user policy or GNOME settings.

The investigation alone made no policy changes. Implementation now lives in
canopa-portal's lockdown module and uses a private transactional runtime database,
following the existing separation between mutable runtime state and common KDL.

## Source seams inspected after DynamicLauncher implementation

`canopa-config::Document` selects an application-owned namespace and preserves
whole-source fingerprints, but its text becomes the embedded fallback when that
namespace is absent. A policy writer must retain/re-read the authoritative whole
source; writing Document.text() in that case would erase the user's other blocks.
`Component` and the envelope currently contain only Shell/Notifications/Files
plus shared appearance. A first-class Portals component needs shared-only discovery
(no invented legacy source), an owning parser/validator and CLI sibling validation.
`canopa config check` currently snapshots a common file and theme once and invokes
three owning executables; the portal binary currently has no check-config mode.
These are cross-application changes, requiring the shared gate after implementation.

Persistence requires a deliberate decision, not an optimistic fingerprint check
called a compare-and-swap. Ordinary read/hash followed by atomic rename still has
a race with an uncoordinated editor's rename/write. Define the actual concurrency
contract or choose an authoritative mutable store that avoids overwriting unrelated
human configuration. Explicit --defaults, read-only administrative sources,
symlink-selected dotfiles, missing discovered files and file replacement all need
specified Set behavior before publication. A policy must not become a temporary
in-memory value when persistence fails. No configuration or policy source has been
modified by this inspection.
