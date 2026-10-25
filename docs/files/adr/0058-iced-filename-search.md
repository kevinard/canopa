# Iced filename search and tab-owned results

Status: accepted under delegated implementation authority, 2026-10-18.
The Iced filename-search slice is implemented and verified. Quick Look and the
complete migration remain in progress.

Use the existing filename-only search model from ADR0002. Ctrl+F or Search opens
a query field in the current Tab. Search initially covers the current folder;
Include subfolders is explicit. Emptying the query shows ordinary browsing while
the search controls remain open. Escape, Close or Back leaves search and restores
the folder's selection and scroll. Editing a location with Ctrl+L deliberately
closes search; navigating into a result or its containing folder also leaves it.

Keep one reader thread and one replaceable pending query per Tab, rather than
one thread per keystroke. Changing query/scope cancels the prior request. A bounded
channel delivers the first progress snapshot and subsequent updates at most every
100ms while the reader is running. Stale results and controls cannot target another
tab or a reopened search. Closing a tab stops its worker; an inactive tab's explicit
search may continue without changing another tab's selection or input focus.

Search results temporarily use one list, with filename and containing location
relative to the known search root. This keeps equal filenames distinguishable
inside narrow tiles. Unreadable-location labels use the same relative treatment.
Selection and activation retain original Unix path bytes. Search has its own
selection/scroll state; generation-tagged list events cannot overwrite ordinary
folder state after search closes. Return or Down moves from query editing into
results. Window titles distinguish searching, stopping, stopped and completed
queries so users can identify them while switching between tiled windows.

Stop preserves results already found. The initial budget is 50,000 matches;
More results explicitly increases it by 50,000 and re-reads the location. Preserve
prior results and selection if that larger read is stopped before revisiting them.
Report unreadable locations alongside matches, with an inspectable detail list.
Both result and failure row construction remain bounded by the viewport.

Expose Include subfolders, Stop, More results, Show in containing folder and
Unreadable locations as configurable commands. They have no new default bindings,
so existing valid custom-action shortcuts retain their meaning. The native fixture
assigns private bindings through the real shared configuration to verify this
interface. Full graphical keyboard focus/accessibility and Quick Look integration
remain required migration gates; this search port does not claim them complete.

Fourteen adapter tests cover ownership, stale messages, selection/restoration,
cancellation and bounded widget construction. Twenty-two focused public tests
cover browsing, search, preferences and custom-action shortcut compatibility.
Native search verification passes sixteen checks in each theme, at 100% and
125% scale, using a 360-pixel tile and 50,005 real matching files. These checks
include the actual toolbar click, clear painted space before Search, raw-byte
launch/reveal, unreadable folders, independent tabs, the initial result limit,
Stop during More and an explicit larger budget. Screenshot review caught the
unclipped location text; a clipped label and native pixel assertion repair it.
The compiler build, all-target Clippy lints and formatting also pass. Four existing
dead-code warnings belong to the unported operations adapter. See the
[search proof](../../../.scratch/file-manager/iced-search-verification.json).
