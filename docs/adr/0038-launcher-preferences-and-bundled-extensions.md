# Launcher preferences and bundled extensions

Date: 2026-11-03
Status: accepted

Shell retains the existing Panel Menu, Foreground, placement and action owners.
Searchable actions, alias editing and Open with are child modes within that same
menu. Tab opens the selected result's named actions, filtered by the same search
field. Back restores its parent query. Result/session revisions protect child
activation; disappearing live windows retire their pending action menu.

Bundle Web and Date/time providers as executable modes of canopa-shell. They
receive the existing version-one bounded JSONL request and return the same typed
results through the normal debounce, deadline and process-group cancellation.
Web constructs URLs only. Ordinary queries are quiet unless they are explicit
HTTP(S) URLs; an explicit configured keyword builds a search URL with escaped query
bytes. No network request occurs during matching. A new additive open_url action
accepts validated HTTP(S) targets; selecting it delegates to the default handler.
Other executable providers can use it too. Older hosts refuse an unknown action.

Date/time uses a closed local grammar: calendar-day/week offsets, IANA time-zone
names or unambiguous city names, and Unix seconds/RFC3339 conversion. Chrono owns
calendar and timestamp conversion. The system zone catalog and GNU date supply
zone formatting in a bounded child process; the outer provider owns its deadline.
No arbitrary date command language, global TZ mutation or network lookup is used.

The canopa-launcher crate contains shared preference/discovery schemas and private
storage, not application execution or presentation policy. Preferences live at
XDG_CONFIG_HOME/canopa/launcher.json. Pins and aliases store a chosen stable target,
label and optional icon. Files preserve original Unix bytes. Automatic selection
history remains separate and does not save queries. Explicit saved shortcuts may
contain the chosen URL, command arguments or label, as requested by the user.

Shell and Settings serialize cooperating writes with an owner-only lock, reread
before applying a field-level edit, validate budgets/keywords/targets, then atomically
publish owner-only durable JSON. Settings refuses stale extension/search edits and
preserves concurrent Shell shortcut changes. Shell rechecks preferences off-thread
once per second, retains the last valid snapshot on errors and retires affected
query processes when routes change. Existing executable argv remains in the Shell
source configuration; Settings does not install or silently execute discovered code.

Settings adds an ordinary Launcher page to its existing application. Shell publishes
a bounded read-only extension catalog with descriptions, supported/observed named
actions and safe provider statuses, without queries or result payloads. Settings
refreshes this catalog without replacing drafts. Configured extension names identify
their overrides across launcher menus. Bundled IDs are separate from configured
names. A configured keyword retains precedence over a colliding bundled default;
conflicts are shown in the panel and can be resolved in Settings. Browser targets
and child-mode actions remain Shell policy; Settings only edits their preferences.
