# Search backend research — 2026-10-15

Scope: existing Linux software behind a Spotlight-style Canopa Shell **Panel**.
This is an evidence review and proposal, not an accepted architecture decision.
No backend was installed, benchmarked, or exercised against the user's files.
Sources are first-party documentation and source; moving branches and cached web
responses require checking again when selecting an exact packaged version.

## Recommended direction

**First candidate to benchmark: DankSearch (`dsearch`) for indexed file/folder
names and optional text-file content, alongside Canopa's existing application
and Niri window sources.** It already powers another shell's Spotlight and has
a standalone service/API, avoiding the need to build a filesystem indexer.
Its published feature scope is a closer match to a small independent shell than
adopting a full launcher UI. This recommendation is an integration judgment,
not a measured performance or reliability claim. [DankSearch overview](https://danklinux.com/docs/danksearch/),
[repository](https://github.com/AvengeMedia/danksearch).

If searching **inside PDF/Office documents, email and archives** is part of the
initial requirement, evaluate **Recoll** immediately. For an established desktop
file/metadata service with a query API, **GNOME LocalSearch** is another strong
candidate. Their differences concern document coverage, dependencies and API
integration, not whether Canopa can own the UI. [Recoll features](https://www.recoll.org/pages/features.html),
[LocalSearch overview](https://gnome.pages.gitlab.gnome.org/localsearch/overview.html).

## Comparison

| Candidate | Actual reusable boundary | Search scope | Assessment for Canopa |
| --- | --- | --- | --- |
| DankSearch | Headless service, HTTP API/OpenAPI, JSON CLI | File/folder names; optional text-file content; image EXIF | Best lightweight first investigation; needs real workload/API compatibility testing |
| LocalSearch / former Tracker miners | Session D-Bus SPARQL endpoint through TinySPARQL | Files, structured metadata, extracted document text | Strong maintained desktop service; GNOME libraries/extractors and indexing configuration to package |
| Recoll | `recollq` / `recoll -t`; Python API | Names and rich document content, including containers | Strongest investigated document-search option; extraction helper packages and result-opening semantics matter |
| Baloo | C++ `KF6::Baloo` query library; `baloosearch` CLI | Files and optionally contents/metadata | Good if already using its index; Qt/KDE integration overhead for this shell |
| Elephant | Unix sockets + protobuf query/activation/subscription | Applications, path search and many utility providers | Designed for this split, but upstream explicitly says development is on hold |
| plocate | CLI querying a periodically rebuilt filename database | Paths only | Simple optional filename adapter, with weaker freshness/ranking |
| FSearch | GTK application; public CLI can preset the GUI query | Names/paths and file attributes | Useful standalone application; a stable headless query-result API was not verified |
| Zeitgeist | D-Bus activity log, query and monitoring APIs | Recorded subjects and event metadata; recency/frequency | Optional history/ranking source; filesystem discovery and document-content search remain separate |

The table summarizes the individually cited evidence below. None of the file
indexers automatically supplies application discovery, shell commands, calculator
results, open windows and universal cross-category ranking as one complete service.

## DankSearch

- The project describes a single portable Go binary, MIT licensed, with a CLI
  and REST API; its documented consumer is DankMaterialShell's Spotlight. The
  repository describes Bleve indexing and a systemd **user** service. This is
  genuinely a reusable backend; adopting DankMaterialShell is unnecessary.
  [Overview](https://danklinux.com/docs/danksearch/), [README](https://github.com/AvengeMedia/danksearch).
- `dsearch serve` runs the API and change watcher. CLI queries support JSON,
  limits, filename/body fields, typo tolerance, directory filters and sorting.
  The running service's `/docs` exposes its OpenAPI 3.1 contract. Use that
  contract and a pinned package version when implementing the adapter:
  the usage page's `.[].path` example differs from the README's `.hits[].id`.
  [Usage](https://danklinux.com/docs/danksearch/usage), [README](https://github.com/AvengeMedia/danksearch).
- Indexed text means configured text extensions. Binary formats are indexed by
  filename; adding `.pdf` to a text-extension list does not create PDF extraction.
  The inspected `readDocument` reads bytes directly into `Body`, with a configured
  byte cap. Directory documents and `dir`/`file`/`all` filters exist. Body storage
  and term vectors are disabled in the inspected mapping, so matched snippets
  should not be assumed. [Configuration](https://danklinux.com/docs/danksearch/configuration),
  [indexer source](https://raw.githubusercontent.com/AvengeMedia/danksearch/master/internal/indexer/indexer.go).
- Configuration has per-root depth, exclusion and watching controls. The documented
  HTTP listener defaults to `127.0.0.1:43654` and has no authentication; a Canopa
  package should keep its selected local transport private. Unix-socket support
  was not independently verified in this research. [Configuration](https://danklinux.com/docs/danksearch/configuration).
- Existing shell integration and shipped service documentation are adoption signals,
  not a substitute for validation. Startup/warm-query latency, rename handling,
  resource use and compatibility across upgrades remain unmeasured here.

## LocalSearch / TinySPARQL

- LocalSearch is GNOME's filesystem indexer; TinySPARQL supplies its query library.
  Current documentation specifies the `org.freedesktop.LocalSearch3` D-Bus name
  and `SparqlConnection.bus_new(...)`. Its graphs cover filesystem, documents,
  audio, video, pictures and software. This is a real headless query endpoint,
  not GNOME Shell UI filtering. Older Tracker packages need a separate version
  check rather than assuming the new names. [Endpoint](https://gnome.pages.gitlab.gnome.org/localsearch/endpoint.html).
- Files are searchable by name/attributes; format-specific extractors add metadata.
  Default indexing excludes hidden files, Git repositories and some mountpoints.
  Coverage must be configured explicitly for the user's intended roots.
  [Indexed data](https://gnome.pages.gitlab.gnome.org/localsearch/indexed-data.html).
- TinySPARQL provides full-text matching, rank and snippets, suitable for bounded
  queries with backend ranking. This is token-based content retrieval; it is not
  the same matching contract as Canopa's existing fuzzy menu labels.
  [FTS functions](https://gnome.pages.gitlab.gnome.org/tinysparql/sparql-functions.html).
- Session integration is explicit: desktop environments should start
  `localsearch-3.service`. The CLI also supports indexed-root management, bounded
  searches and indexing status. Main query/indexing and sandboxed extraction are
  separate processes. [Command reference](https://gnome.pages.gitlab.gnome.org/localsearch/commandline.html),
  [process overview](https://gnome.pages.gitlab.gnome.org/localsearch/overview.html).
- Build metadata includes GLib/GIO, TinySPARQL and format-dependent libraries
  such as Poppler and libgsf. It does not require adopting GNOME Shell's UI.
  Source licenses distinguish LGPL-2.1-or-later extract/common code from
  GPL-2.0-or-later miners. Official NEWS records 3.12.0 on 2026-08-31, including
  sandbox, indexing-configuration and memory fixes. [Build metadata](https://raw.githubusercontent.com/GNOME/localsearch/main/meson.build),
  [COPYING](https://raw.githubusercontent.com/GNOME/localsearch/main/COPYING),
  [NEWS](https://raw.githubusercontent.com/GNOME/localsearch/main/NEWS).

## Recoll

- Recoll finds both names and document contents, including nested archive/email
  content; its site identifies Xapian as the underlying engine and GPL as its
  license. Current site news announces 1.44.2 on 2026-09-01.
  [Project overview/news](https://www.recoll.org/index.html).
- The documented headless commands are `recollq` and `recoll -t`. They support
  bounded slices, filename mode, abstracts and a `-F` field-output format using
  base64 values explicitly recommended for program consumers. This is a
  practical subprocess adapter, not screen-scraping the Qt GUI.
  [CLI search](https://www.recoll.org/usermanual/webhelp/docs/RCL.SEARCH.COMMANDLINE.html).
- A Python API can keep a database connection, execute queries, fetch result
  metadata and generate abstracts. A small persistent bridge is possible if
  process startup proves significant; it would be Canopa integration code,
  not an existing Recoll wire protocol.
  [API example](https://www.recoll.org/usermanual/webhelp/docs/RCL.PROGRAM.PYTHONAPI.SEARCH.EXAMPLE.html).
- The GUI can be excluded with `-Dqtgui=false`; the headless stack still needs
  Xapian and extraction dependencies. PDF text uses Poppler's `pdftotext`;
  additional formats have different helpers. Real-time indexing is
  `recollindex -m`, with user-service examples; outside X11 the documentation
  requires `-x`. [Build prerequisites](https://www.recoll.org/usermanual/webhelp/docs/RCL.INSTALL.BUILDING.PREREQS.html),
  [format helpers](https://www.recoll.org/pages/features.html),
  [indexing service](https://www.recoll.org/usermanual/webhelp/docs/RCL.INDEXING.MONITOR.html).
- Nested results may identify a document inside a container, not a normal file
  suitable for a generic opener. Either limit the first adapter to ordinary
  file results or support extraction/opening explicitly. This is an integration
  implication of Recoll's container model, not a missing indexer capability.

## Baloo

- Baloo is KDE Plasma's file index/search framework, optionally indexing content.
  Its supported query API is the C++ `Baloo::Query` library linked as `KF6::Baloo`,
  with filters, limits and iteration. The query object's JSON serialization is
  not itself a remote JSON result API. [Overview](https://api.kde.org/baloo-index.html),
  [Query API](https://api.kde.org/baloo-query.html).
- `baloosearch` really executes that query and prints paths; its inspected source
  supports limit/type/directory options, but not a machine-readable JSON result
  flag or NUL-delimited paths. Its current sort defaults emphasize mtime rather
  than assuming Spotlight-style relevance. [CLI source](https://raw.githubusercontent.com/KDE/baloo/master/src/tools/baloosearch/main.cpp),
  [sorting API](https://api.kde.org/baloo-query.html).
- The build requires Qt6, KDE Frameworks including FileMetaData/KIO and LMDB,
  and optionally builds a background session indexer. Query headers have an
  LGPL license expression; verify individual files when reusing source.
  Good established ecosystem fit, but less attractive to introduce solely for
  Canopa's first search panel. [Build metadata](https://github.com/KDE/baloo/blob/master/CMakeLists.txt),
  [query license](https://github.com/KDE/baloo/blob/master/src/lib/query.h).

## Elephant and existing launcher UIs

- Elephant is explicitly a backend for custom frontends, using Unix sockets and
  protobuf with query, activation and subscription messages. Providers include
  applications, files, calculator, clipboard and web search. It is GPL-3.0 and
  needs a correctly initialized user environment. Crucially, the current README
  says **“Development is on hold.”** This weakens it as a new default dependency.
  [README](https://github.com/abenz1267/elephant).
- Its query schema contains provider names, query text, result limits, result
  identity, score, icons and actions. File search's inspected implementation
  indexes paths using `fd` and SQLite, then scores paths; text/PDF previews do
  not mean document-content indexing. Watching is configurable and defaults
  false in the inspected source. [Query schema](https://raw.githubusercontent.com/abenz1267/elephant/master/pkg/pb/query.proto),
  [file setup](https://raw.githubusercontent.com/abenz1267/elephant/master/internal/providers/files/setup.go),
  [file query](https://raw.githubusercontent.com/abenz1267/elephant/master/internal/providers/files/query.go),
  [file database](https://raw.githubusercontent.com/abenz1267/elephant/master/internal/providers/files/db.go).
- Walker is a GTK4/Rust frontend requiring Elephant. Running it as a separate
  launcher can provide immediate functionality, but does not put the results
  in Canopa's own **Panel**. [Walker README](https://github.com/abenz1267/walker).
- Longer term, GNOME's `org.gnome.Shell.SearchProvider2` is an existing D-Bus
  provider convention with result IDs, metadata and activation. Supporting it
  could reuse application search providers, but requires discovery and lifecycle
  handling; it is not a universal filesystem backend by itself.
  [GNOME provider tutorial](https://developer.gnome.org/documentation/tutorials/search-provider.html).

## Narrow filename options

- plocate provides a CLI over an `updatedb` database; the official homepage
  announces 1.1.25 on 2026-09-06. NUL output, result limits and existence checks
  make process integration feasible. Updates are normally periodic, so it does
  not provide live freshness or document contents. Its own performance example
  is not a benchmark of this machine. [Project](https://plocate.sesse.net/),
  [query manual](https://plocate.sesse.net/plocate.1.html),
  [update manual](https://plocate.sesse.net/updatedb.8.html).
- FSearch is a GTK3 filename-search application with regex, filters and indexed
  root controls. Its CLI `--search` is routed into a window's text entry in the
  inspected code. A documented headless result-returning API was not established
  here; reusing internal C code would be a different, GPL-governed integration.
  [README](https://github.com/cboxdoerfer/fsearch),
  [application source/license](https://raw.githubusercontent.com/cboxdoerfer/fsearch/master/src/fsearch.c).

## Plocate integration follow-up

Plocate is a credible first file-name provider when an existing system index has
useful coverage. A Rust worker can invoke its CLI and parse NUL-delimited paths.
Matching is substring/glob rather than fuzzy; short queries under three bytes and
regex can scan the whole database. Retrieval limits apply before Canopa ranking,
so ranking a bounded subset cannot guarantee globally best results.
[Query manual](https://plocate.sesse.net/plocate.1.html).

Use the installed CLI to retain system-index visibility handling. Freshness and
coverage depend on update cadence and configured exclusions. `--existing` removes
stale results but does not discover new paths. A private user database is supported
through `updatedb -U/-o` and `plocate -d`; its refresh lifecycle would need ownership.
[Database manual](https://plocate.sesse.net/updatedb.8.html),
[exclusions](https://plocate.sesse.net/updatedb.conf.5.html).

For an MVP, accepting substring file matching avoids a new crawler. If fuzzy file
matching or immediate new-file discovery is required, evaluate an additional path
cache/watch mechanism or another backend. Keep content search and activity ranking
as separate providers. This is a proposal, not an accepted dependency decision.

Local read-only inspection found plocate 1.1.25 and a 41,573,858-byte system database
modified on 2026-10-15. A query for this repository's context-map path was denied
database access in the current sandbox; system-timer inspection was also blocked.
Those checks do not establish query performance, index coverage or timer health.

## Ripgrep as a Rust search component

Follow-up question: can a Rust backend, especially ripgrep, replace the indexer?
**Ripgrep is a useful on-demand content provider.** It performs recursive
line-oriented matching; each query searches the selected files rather than
consulting a persistent full-text index. Consequently, broad interactive content
search needs workload measurements and cancellation. This performance implication
follows from its scanning design; no Canopa benchmark has been run.
[README](https://github.com/BurntSushi/ripgrep),
[searcher API](https://docs.rs/grep-searcher/latest/grep_searcher/).

Separate three jobs in a Rust-first implementation:

| Job | Reusable component | Remaining Canopa work |
| --- | --- | --- |
| Discover file/folder paths | [`fd`](https://github.com/sharkdp/fd) as a process, or ripgrep's [`ignore`](https://docs.rs/ignore/latest/ignore/) walker as a library | Root/exclusion policy, cached path candidates, refresh/watch lifecycle and fuzzy ranking |
| Search text-file contents | `rg` with [JSON Lines output](https://docs.rs/grep-printer/latest/grep_printer/struct.JSON.html), or [`grep-searcher`](https://docs.rs/grep-searcher/latest/grep_searcher/) with [`grep-regex`](https://docs.rs/grep-regex/latest/grep_regex/) | Query scheduling/cancellation, bounded results, deduplication, content snippets and source ranking |
| Search an enduring full-text index | [`Tantivy`](https://github.com/quickwit-oss/tantivy) | Filesystem crawling, content extraction, watching, index schema/updates and recovery; Tantivy is a library rather than a desktop indexing service |

`rg --files` lists candidate files; it does not itself supply fuzzy filename
ranking or folder results. `fd` or `ignore` covers the file/folder discovery job.
Canopa already owns fuzzy menu semantics. [`nucleo-matcher`](https://github.com/helix-editor/nucleo)
is another Rust matching component to evaluate if necessary; introducing it should
preserve deliberately chosen shell matching behavior rather than silently changing
existing menu semantics. These are reusable parts, not a complete Spotlight backend.

For rich formats, [`ripgrep-all` (`rga`)](https://github.com/phiresky/ripgrep-all)
is a Rust wrapper around ripgrep. It uses extraction adapters such as Poppler for
PDF and Pandoc for DOCX/ODT/EPUB, and caches extracted text subject to size limits.
That cache avoids repeated conversion; queries still scan text rather than using
an inverted search index. External format converters remain dependencies, and
archive-contained matches still need explicit opening/extraction behavior.
The package declares AGPL-3.0-or-later in its [manifest](https://raw.githubusercontent.com/phiresky/ripgrep-all/master/Cargo.toml).

Suggested Rust-first proof of concept: retain app/window sources; populate and
refresh a bounded path cache off the UI thread; fuzzy-match that cache during
typing; run cancellable, debounced content queries over configured roots using
`rg --json`. Start with literal input unless the user explicitly selects regex.
Choose hidden/ignored-file behavior explicitly because ripgrep's defaults target
code search. Respect per-query time, result and output-byte budgets. A result cap
can return incomplete scan results rather than the globally most relevant files;
the UI should reflect that distinction. If repeated scans fail the desired latency
or CPU budget, evaluate a persistent index before expanding content scope.
[Filtering and preprocessing](https://github.com/BurntSushi/ripgrep/blob/master/GUIDE.md),
[structured results](https://docs.rs/grep-printer/latest/grep_printer/struct.JSON.html).

## Zeitgeist compared with the proposed search panel

The freedesktop Zeitgeist project supplies desktop activity history through D-Bus.
Its corpus consists of events involving subjects such as files, websites or
conversations. It can help answer when/how often an item was used.
[Project overview](https://zeitgeist.freedesktop.org/),
[subject model](https://zeitgeist.freedesktop.org/documentation/latest/C/ZeitgeistSubject.html).

| Requirement | Zeitgeist's contribution | What Canopa still needs |
| --- | --- | --- |
| Recent/frequent items | Queries can group subjects or applications and sort by recency/popularity | Incorporate that evidence into shell ranking and empty-query suggestions |
| Find an arbitrary file/folder | Can return subjects that have recorded activity | A filesystem path source for items without recorded events |
| Find text inside documents | FTS searches recorded event metadata, including subject display text and actor application metadata | ripgrep/rga or an indexer that reads document content |
| Installed applications / live windows | May contain application activity | Existing desktop-entry discovery and current Niri window state |
| Shell appearance and activation | Headless data APIs | Native panel, live query/selection lifecycle and typed launch/open/focus actions |

The first row is supported by documented `MOST_RECENT_SUBJECTS`,
`MOST_POPULAR_SUBJECTS` and actor sort/group modes.
[Result types](https://zeitgeist.freedesktop.org/documentation/latest/Vala/zeitgeist-2.0/Zeitgeist.ResultType.html).
The remaining rows are integration implications of an event-based corpus, rather
than measurements of the service.

Its **Full Text Search** name needs careful interpretation: the API searches the
event index, with `name`/`title` targeting subject text, `app` application metadata
and `site` URI domains. A subject's text is a short display representation.
This does not provide general PDF/body extraction or a crawl of every file.
For example, an unreported, never-opened PDF has no event through which to be
found; a phrase found only in its body needs a separate content provider.
[Index API](https://zeitgeist.freedesktop.org/documentation/latest/C/ZeitgeistIndex.html),
[subject text](https://zeitgeist.freedesktop.org/documentation/latest/C/ZeitgeistSubject.html).

Activity coverage depends on event producers. Canopa could record its own search
activations; cross-application file, browser and conversation history requires
working collectors/app integrations. Installing the daemon alone does not establish
that coverage in a Niri session. D-Bus access allows a Rust adapter; the existing
service is not a Rust backend. The packaged stack includes Vala/GLib components,
SQLite and Xapian, with additional distro-selected dependencies.
[Data sources](https://zeitgeist.freedesktop.org/documentation/latest/C/zeitgeist-2.0-data-source.html),
[log API](https://zeitgeist.freedesktop.org/documentation/latest/C/ZeitgeistLog.html),
[Arch package/build dependencies](https://archlinux.org/packages/extra/x86_64/zeitgeist/).

Maintenance evidence: the latest published upstream release verified here is
**1.0.4, January 12, 2022**. Arch still packages 1.0.4 with a 2026 package update;
that date reflects packaging, not a new upstream release. Current upstream GitLab
tags/NEWS were inaccessible, so this review does not establish current commit
activity or claim abandonment.
[Upstream releases](https://launchpad.net/zeitgeist/+download),
[Arch package](https://archlinux.org/packages/extra/x86_64/zeitgeist/).

Recommendation: retain file discovery/content search as independent providers.
For the first version, a small local record of Canopa result activations can supply
last-used/use-count ranking; make history retention and clearing explicit. An
accepted launch handoff records activation, not proof that an application opened
a document. Evaluate Zeitgeist as an optional provider when existing collectors or
a broader cross-application activity timeline are desired. This proposal reuses its
history concepts without making the filesystem search depend on activity coverage.

## Implementing an independent Zeitgeist-compatible service

Zeitgeist publishes a D-Bus API and semantic data model that could guide an
independent implementation. The official reference lists Log, Monitor, Index and
DataSourceRegistry interfaces. Event documentation defines actors, interpretation,
manifestation, subjects, engine-assigned IDs and millisecond timestamps, plus
template-based query semantics; ontology URI definitions supply shared type names.
[D-Bus interfaces](https://zeitgeist.freedesktop.org/documentation/latest/C/ch05.html),
[event model](https://zeitgeist.freedesktop.org/documentation/latest/Vala/zeitgeist-2.0/Zeitgeist.Event.html),
[ontology definitions](https://zeitgeist.freedesktop.org/documentation/latest/C/zeitgeist-2.0-ontology-uris.html).

This review found project API documentation rather than a verified self-contained,
normative interoperability specification or certification suite. In particular,
the generated Log reference renders incomplete wire signatures and exposes async
implementation parameters. Exact compatibility would require inspecting a pinned
release's declarations and live D-Bus introspection, then testing behavior against
the reference daemon and selected real clients. This review did not verify the
current 1.0.4 interface declarations because the relevant raw source endpoints were
inaccessible. No exact wire types are inferred from obsolete designs here.
[Generated Log reference](https://zeitgeist.freedesktop.org/documentation/latest/C/docs-org-gnome-zeitgeist-Log.html).

Archived API blueprint pages need particular care: SeparateServicesAPI is explicitly
obsolete, while EngineAPIDraft is explicitly a draft. Their signatures and proposed
multi-log/repository APIs must not be substituted for the released implementation.
[Obsolete blueprint](https://wiki.gnome.org/Projects%282f%29Zeitgeist%282f%29Blueprint%282f%29SeparateServicesAPI.html),
[API draft](https://wiki.gnome.org/Projects%282f%29Zeitgeist%282f%29Blueprint%282f%29EngineAPIDraft.html).

Two Canopa proposals remain distinct:

- A native Rust activity store can adopt the useful event concepts and supply
  recency/frequency to search. It would have its own documented contract.
- A Zeitgeist compatibility adapter could accept existing producers and serve
  existing consumers. It must match the selected clients' required service names,
  object paths, signatures, errors, properties, event IDs, matching/grouping/order,
  monitors and advertised extensions. Internal storage can be designed separately;
  database-format compatibility is an additional migration requirement, not implied
  by matching a D-Bus interface. A narrow partial implementation should declare its
  supported profile and be tested before being described as a replacement.

For the search feature, decide whether concrete existing Zeitgeist clients need
interoperability before committing to that compatibility surface. The public event
model is still useful when only Canopa consumes the new activity store. Either
choice leaves filename and document-content search with their separate providers.
No implementation or architectural decision was made in this review.

## Existing Canopa foundations and remaining work

Local source review used workspace commit `5467ff6`. The following is a proposed
implementation path; it does not change the accepted provider or launch contracts.
The existing [visual-direction decision](../../adr/0014-canopa-visual-direction.md)
already leaves a combined app/window/settings launcher for a later slice.

| Foundation | What can be reused |
| --- | --- |
| Applications | [Desktop discovery](../../../crates/canopa-shell/src/desktop_apps.rs) retains desktop IDs, localized names, descriptions, keywords and icons, with desktop-entry visibility rules. |
| Searchable app menu | [Menu sessions](../../../crates/canopa-shell/src/app/panel_menu_session.rs) and [matching](../../../crates/canopa-shell/src/app/panel_search.rs) already provide fuzzy ranking and keyboard selection. Their installed-app source is a fixed snapshot during an open menu, and matching is ASCII case-insensitive. |
| Open windows | [Niri state](../../../crates/canopa-shell/src/niri.rs) already caches live windows. Search still needs a window-result projection and focus activation route. |
| Shell presentation | [Foreground](../../../crates/canopa-shell/src/app/foreground.rs) owns replacement/dismissal; existing panels supply placement, theme, keyboard and accessibility foundations. A new search view must keep those owners. |
| Activation | [Application launching](../../../crates/canopa-shell/src/command/applications.rs) hands application lifetime to Niri or a user scope, and handles desktop-entry/D-Bus activation. Search must preserve this boundary. File/folder opening needs its own typed activation path. |
| Entry points | The bundled [Applications menu](../../../crates/canopa-shell/config/default.kdl) can already be opened using `canopa-shell --toggle-menu Applications`. Its shell keybind is focus-scoped; invocation from anywhere needs a Niri global binding. A unified-search IPC/action entry point remains new work. |

The main implementation pieces would be:

1. A native **Search Panel** with one focused query field and results showing
   category, label, icon and path/application context. Reuse the existing panel
   surface and Foreground lifecycle. A centered Spotlight-style placement would
   be an additional placement policy; the existing panel is origin-anchored.
2. A search session/adapter that combines existing applications and Niri windows
   with bounded file results from the selected indexer. Keep indexing alive in
   its separately owned user service across panel closes. Keep source ranking
   meaningful, merge categories deliberately, and retain selection by stable ID.
   A calculator can reuse [Qalculate's `qalc` CLI](https://qalculate.github.io/manual/qalc.html)
   for arithmetic and unit conversion rather than adding an expression engine.
3. Explicit live-query semantics in the panel integration. Current
   [selectable-list search](../../../crates/canopa-shell/src/app/external_panel_session.rs)
   changes host-local state without sending the query to the provider. A separate
   `TextInputChanged` event does reach providers and could support a proof of
   concept. However, [v5](../../../crates/canopa-panel-protocol/src/v5.rs) separates
   Form input from Rich results, and its [revision rules](../../../crates/canopa-panel-protocol/src/v5/session.rs)
   advance when result lists change. Production search needs persistent query
   identity, latest-query generations, stable focus, and result-specific activation
   guards so result refreshes neither lose queued typing nor activate stale rows.
   This is an extension to design, not permission to bypass v5 stale-event checks.
4. Packaging/configuration for the chosen backend: indexed roots and exclusions,
   content scope, service availability, indexing progress, default openers and
   optional history/clear-history behavior. An unavailable file backend should
   leave app/window search usable. Distinguish an incomplete index from no matches.

Suggested first implementation: applications, open windows and indexed file/folder
names, followed by calculator and interaction polish. Include ordinary text-file
content only when wanted; if PDF/Office contents are required immediately, evaluate
Recoll/LocalSearch in that same first slice. Benchmark the backend before accepting
it as the supported dependency. No application code was changed for this research.

## Integration questions and validation before selection

The backend should own indexing and content extraction. Canopa still needs a
persistent query session, request generations/cancellation, result identities,
category ranking, keyboard selection, activation and explicit unavailable/indexing
states. Fetching a bounded backend result set differs from filtering a fixed menu
locally; applying filename fuzzy filtering again would incorrectly discard content
matches whose visible label lacks the query.

Compare a pinned DankSearch build with Recoll or LocalSearch on representative
roots: cold startup, query latency, index size, idle/indexing CPU and memory,
rename/delete freshness, accents/non-ASCII names, permission failures, symlinks and
backend restart. Test results while typing quickly before expanding providers.
Select exclusions and content scope explicitly, then package one user service.

Prefer consuming installed processes/APIs to copying indexer internals. Licenses
above inventory upstream declarations; source copying/linking and distributing
modified backends require their own dependency/license review. No license decision
or architectural ADR is made by this note.
