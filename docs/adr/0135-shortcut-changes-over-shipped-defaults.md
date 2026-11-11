# 0135 Shortcut changes over shipped defaults

Status: accepted. Supersedes the shortcut materialization in ADR 0129.

ADR 0129 had the first shortcut save copy the shipped bindings into the user's
`niri.kdl`, because Niri overrides an inherited key but cannot remove one. Users
who saved one shortcut then stopped receiving new and changed default bindings,
and any later difference from the compiled defaults made the editor read-only.

Settings now stores only the user's changes in `~/.config/canopa/shortcuts.kdl`:
added or changed bindings, and the shipped actions that were moved or turned off.
It records the shipped defaults file it applies them to. Settings generates
`niri-shortcuts.kdl` from the current shipped defaults and those changes: shipped
bindings minus the removed actions and the keys the user assigned, followed by
the user's bindings. A user binding therefore wins any conflict over its key.
Removal matches the shipped action, not its key, so a moved action stays moved
when an update changes its default key, and a new shipped binding on a freed key
still arrives.

While changes exist, `niri.kdl` includes the generated file in place of the
shipped defaults; with no changes it includes the shipped file directly. Session
start (`canopa-settings init-session`) regenerates the file, so package updates
reach users at their next login. Every candidate is validated with Niri before
it is published, and a failed update keeps the last valid files without blocking
the login.

Session start also migrates the legacy copy. The legacy defaults are kept in the
Settings source because the copy must be compared with the release it came
from, not with the current defaults. Bindings identical to that release become
inherited again. Everything else becomes a change, and the result must reproduce
the copied block exactly before anything is written. Copied non-binding sections
the user edited stay in `niri.kdl` after the include so they keep overriding the
shipped values. The previous `niri.kdl` is saved as
`niri.kdl.before-shortcut-changes`. A top-level binding block that is not a
recognizable copy is never rewritten.

When a release replaces a shipped action, Settings keeps a list of the retired
action and its replacement. Once the shipped defaults have the replacement and
no longer the old action, saved changes and migrated copies that still name the
old action follow it: a moved binding keeps its key with the new action, a
turned-off one stays off, and a carried binding that now equals a shipped one is
inherited. Bindings whose action the user wrote themselves are kept as written.
The first retired actions are the `wpctl` volume keys, replaced by Canopa Shell
device keys ([Shell ADR 0025](../shell/adr/0025-device-keys-and-on-screen-display.md)).
