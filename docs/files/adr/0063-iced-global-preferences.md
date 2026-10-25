# Iced global interface preferences

Status: accepted under delegated implementation authority, 2026-10-18.
Implementation and verification are in progress. The full product brief remains
required; this extends ADR 0011 into the supported Iced adapter.

Preferences is one application-owned ordinary Iced window, opened by Ctrl+comma,
the folder menu or a configured toolbar command. Reopening focuses the same
window. It has Views, Toolbar, Locations and Shortcuts pages; Alt+1 through Alt+4
select them. Shared Canopa controls and synchronous traversal keep forms usable
in narrow tiles. Pages scroll vertically and command rows wrap. Source/error
notices occupy fixed slots so they cannot reparent a native text input.
Page accelerators are intercepted in the form widget before native text handling,
with synchronous unfocus. A native failure showed Alt+1 appending `1` to the
focused width field before the application key listener ran. That listener is
too late to protect text or reset traversal; it no longer handles page shortcuts.
Both editors reserve 12 logical pixels at the right of vertical form content so
the overlaid scrollbar cannot clip an input's right border. This follows actual
capture review after the scoped behavioral workflows passed.

Keep one transactional draft across pages. Width and shortcut text can remain
incomplete or invalid while editing; Save validates the entire candidate before
requesting a write. Ctrl+S saves, Ctrl+R reloads, and Escape/Ctrl+W requests close.
Changed drafts ask before reload or close; Keep editing is the default. A busy
writer keeps the application alive and cannot be closed through normal UI.
Explicit built-in defaults are visibly read-only and the underlying writer
refuses them. Publications update Browsers, appearance and controls, never the
editor's unsaved draft. An external source change remains visible and stale Save
retains the draft rather than replacing the newer source.

Use the existing single configuration worker for both action and interface saves.
A successful interface save uses ConfigurationSnapshot::save_settings, preserving
custom commands, synchronizing explicit action toolbar positions and preserving
other Canopa namespaces. Each completion belongs to its request and window;
late callbacks cannot clear a newer operation. The worker's existing bounded
mailbox and stale-source/target checks remain authoritative. There is no second
writer and no GUI-thread configuration I/O.

Toolbar items and locations can be selected, reordered, added and removed.
Exactly one location field remains available. A searchable list offers built-in
commands and existing custom actions; duplicate or unknown positions are refused.
Bookmarks have editable names and local paths. Keep their draft path in the same
byte-preserving local file URI representation as the location bar; parse only at
Save. Accept absolute paths, ~/ paths and local file URIs, without evaluating shell
text or treating arbitrary remote schemes as local relative folders. Missing
local folders may remain bookmarked. Bookmark this folder opens the Locations
draft with a new bookmark and still requires Save. It never writes immediately.

Shortcuts accepts multiple accelerators separated by semicolons, explicit empty
bindings to disable a command, a recorder and Use defaults per command. Canonical
validation covers every built-in and custom action together, retaining selection,
focus and typing keys. Switching commands preserves uncommitted shortcut text.
Keyboard activation of Record must not become the recorded shortcut itself.
The shared control frame can publish a distinct keyboard-activation message;
both editors use it to ignore that activating key while retaining the immediate
first-key behavior of mouse activation. Native verification reproduced the
previous Enter-to-Record feedback as an unintended `Return` shortcut and a real
conflict. Controller regressions cover both editors and both activation sources.

Locations in the folder menu replaces that same popup's contents and has Back;
it does not create a nested popup or depend on sidebar visibility. It and the
sidebar share path resolution and raw bookmark identities. Personal folders now
use the prepared XDG snapshot in [ADR0064](0064-xdg-personal-folder-locations.md).
Trash, devices,
all remaining Iced interfaces and full-product acceptance remain required.

The original light-theme Add failure is causally diagnosed: queued keys crossed
an application page transition before its message applied. All 13 Tabs traversed
the old Views tree; no filter edit or Add occurred. The shared
[input transition boundary](../../adr/0020-iced-input-transition-boundaries.md)
keeps subsequent keys for the rebuilt page. Twelve cold uninstrumented fast
probes pass on the preceding clean executable. Active diagnostics are removed.

The Preferences checkpoint candidate also guards current-folder scroll feedback using the
captured producer viewport, measured native bounds and current model viewport.
Two opposite-order controller regressions fail before the guard and pass after;
settled deliberate scrolling remains available. Files ADR0062 records its native
trace, causal limits and the later verified scrollbar-gutter follow-up.

On 483 unchanged inputs, 47 adapter tests, 17 public tests, all-target Clippy
lints, build, formatting and tracked whitespace pass. Fourteen sequential native
workflows pass 198 declared checks, including both complete Preferences workflows,
with an additional twenty-cycle resize stress case. All 17 application logs are
empty. Eight selected current captures were inspected. Executable:
`a85b1012557e515143f27a8bc7940b48ce273a2381df70c86854920595d77ae4`.
See [source-bound scoped evidence](../../../.scratch/file-manager/iced-preferences-verification.json)
and [checkpoint](../../../.scratch/file-manager/ICED-PREFERENCES-WIP.md).

Personal-folder resolution is subsequently verified in ADR0064. Full preference focus/accessibility/
source-authority and large-catalog acceptance, remaining Iced interfaces, durable
runtime and full shared-stack/workspace/release/review gates remain required.
Four existing operation-helper warning groups remain unsuppressed. The entire
product brief and its completion checkboxes stay open.
