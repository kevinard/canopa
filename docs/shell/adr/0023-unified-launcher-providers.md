# Unified launcher providers in existing panels

Date: 2026-11-03
Status: accepted

The launcher combines application launching, local file/folder name search,
arithmetic, and executable extensions. The user explicitly required the existing
panel facilities and placement. Add a `launcher` Panel Menu Source; the bundled
Applications menu opts in while retaining its name, CLI/IPC entry points, and
spine action. Existing Panel/Foreground owners keep placement, window lifetime,
theme, scrolling, dismissal, and focus. No new placement policy is introduced.

A launcher source owns a session with a unique identity, a query generation,
provider result batches, and displayed result revisions. Existing application
discovery/matching adapts to this source. Bounded result groups use deterministic
ordering rather than comparing provider scores. Same-query updates preserve
selection by provider/result identity. Pointer actions capture the displayed
revision and action identity. Asynchronous results require the current session,
query, and, for files, index revision. Changed configuration, Foreground replacement,
and logical dismissal retire interaction; subscriptions exist only for the active
panel. Late results cannot revive it.

File-name search initially uses a bounded native memory cache, avoiding a new
indexer service dependency. A blocking worker scans configured roots without
following symlinks or crossing devices, with path-count, depth, and elapsed-time
limits. The shell retains the latest matching configuration's cache across opens
and refreshes it every minute while open. Cancellation reaches blocking scans
through a drop guard. The UI distinguishes partial coverage and unavailable roots.
This is eventually refreshed filename search, not a content index or a promise
of immediate filesystem freshness. Files retains in-application browsing/search,
operations, and Undo. A future shared indexing service requires an integration ADR.

Qalculate's installed CLI supplies arithmetic and unit conversions. Validate a
complete numeric grammar before passing an expression as a literal argument,
bound its time/output, and accept only finite numerical output. Unit conversions
require a numeric amount and source/target units from the host's supported catalog,
with compatible dimensions. Unit aliases, squared/cubed factors, and simple ratios
are normalized into trusted Qalculate unit tokens; arbitrary conversion commands
never reach the backend. The backend owns conversion factors and temperature offsets.

Arithmetic disables all definitions. Conversions load built-in units while
disabling currencies, datasets, functions, and variables; both modes isolate
configuration/data/cache locations from personal state and disable network updates.
Absolute temperature mode applies Celsius/Fahrenheit offsets. Explicit targets
disable mixed-unit output, and converted output must have a finite number and the
exact requested unit after alias normalization. Copy includes the unit.
`qalc -n` can return status 1 with a valid arithmetic result; only arithmetic accepts
that status and still validates output. `libqalculate` is the runtime dependency.

Launcher extensions use a separate, versioned request/response JSONL contract.
Each explicitly configured executable receives one routed query and returns a
bounded result list with typed Open, Copy, or Launch actions. The host executes
the action only after selection. Start with one process per debounced query and
cancel its owned process group when the subscription retires. Persistent protocol
sessions and activation callbacks are deferred until there is a concrete need.
This narrows the earlier draft protocol proposal while supplying an extensible
result/action experience now. It does not change external panel protocol v1/v4/v5
or weaken their revision guards. Whole-panel providers keep their own UI content.

Application and opener launches retain ADR 0012's desktop-session handoff. Local
file URIs encode original Unix path bytes, preserving non-UTF-8 targets without
turning display text into a path. Clipboard ownership remains in the shell, so
copied values can survive panel dismissal. Successful action completion closes
the same session/query only; failures keep it open. Clipboard task completion
does not provide a compositor delivery acknowledgement.

The [launcher guide](../launcher.md) defines coverage, refresh behavior, limits,
configuration, and the extension protocol. Validate the core state machine,
provider processes and cleanup, existing shell consumers, and actual input/opening/
clipboard behavior in a private compositor fixture.

Unit-conversion syntax and the mixed-unit target modifier follow the
[Qalculate CLI reference](https://qalculate.github.io/manual/qalc.html).

The next native sources add live Niri windows and desktop controls. Native results
share a quality-first merge, using bounded private recent/frequent identities only
within comparable match classes; calculations remain first and executable result
ordering stays unchanged. Selection revisions also retire on window/history updates.
Only acknowledged successful application/file actions are learned; no query text or
clipboard payload enters history. One worker serializes reads, writes and clear so
queued writes cannot restore cleared entries. `history #false` opts out per source.

The closed arithmetic grammar additionally admits percentages, constants pi/e,
and a documented allowlist of built-in math functions with parentheses. These
built-ins work without loading external function or variable definitions. Qalculate
still owns evaluation and percentage semantics; trigonometry is in radians.

Open with is a child mode of the same Panel Menu. GIO supplies file-capable installed choices with MIME recommendations first and existing desktop-entry launching delivers the original URI,
including D-Bus Application.Open where applicable. No association is changed.
Reveal uses FileManager1; Quick Look uses the additive Files activation described
in [shared ADR 0037](../../adr/0037-launcher-file-actions-and-settings-pages.md).
The existing panel remains the only launcher placement and lifetime owner.

Bundled web/date providers, searchable action child modes, pins/aliases and
Settings preference/discovery exchange follow [shared ADR 0038](../../adr/0038-launcher-preferences-and-bundled-extensions.md).

Clipboard history and saved text snippets use reserved `clip` and `snip` scopes
inside the existing Panel Menu. The scoped query, observations and image
thumbnails stay with Shell. The history list is opt-in, deduplicated and bounded
in memory. Owned wl-paste watchers accept bounded text and optional raster image
frames and ignore offers marked sensitive. Image selection copies the original
bytes and MIME type. Snippet naming is a child mode. Clipboard preference editing
and shared persistence follow [shared ADR 0039](../../adr/0039-launcher-clipboard-settings.md).

Emoji and symbols are native search scopes in the same Panel Menu. Shell matches
bundled Unicode sequences and English search terms locally, caps the visible
results, and uses the existing clipboard action to copy the full sequence.
Configured extensions keep priority when they use either search keyword.
