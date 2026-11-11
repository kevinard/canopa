# Unified launcher

The bundled `Applications` menu searches installed applications, local file and
folder names, open windows, desktop controls, calculations, and unit conversions. It uses the existing Panel
placement, size, theme, focus, scrolling, and dismissal settings of its originating
bar. The bundled launcher uses a wider panel. A bar can set `panel.launcher-width`
in logical pixels to size launcher menus separately.

Open it from the spine or with `canopa-shell --toggle-menu Applications`. For a
global shortcut, configure Niri to spawn that command. Custom configurations can
opt in by changing their menu source:

```kdl
menus {
    menu "Applications" show-icons=#true prompt="Search apps and files" {
        source "launcher"
    }
}
```

`source "installed-apps"` remains the application-only source. Existing configured
menus, dmenu, and whole-panel providers retain their behavior.

## Searching and actions

Type normally to search applications, open windows, controls and file/folder names.
Choose a scope below the search field to narrow the current query. Choose **All**
to search across sources again. You can also type a scope directly, as in
`apps firefox`, `windows notes`, `settings volume`, or `files budget`.
Window rows show the application and workspace. Enter switches to the existing
window and a secondary action can launch another instance. Up and Down select a
result. Enter performs the selected result’s first action. Press Tab to filter the selected result’s
actions. Click a result to run its primary action. The command bar shows the
primary action, the action menu, and available shortcuts such as Copy path. A
single application match can show its desktop-entry description.
Ctrl+Enter performs its second action; Ctrl+1 through Ctrl+5 perform the respective
named action. Escape clears a nonempty query, then closes on a second press.

Matching ignores case and accents in every script: `терм` finds “Терминал”,
`cafe` finds “Café”, and `ρυθμισεις` finds “Ρυθμίσεις”. Text is lowercased with
Unicode rules (also folding `ß` to `ss` and final `ς` to `σ`), then accented
letters are reduced to their base letter, as canonical decomposition followed by
removal of combining marks would. Without a Unicode normalization crate in the
workspace, a built-in table covers precomposed Latin (including Vietnamese),
modern Greek and Cyrillic letters; separate combining marks are dropped in any
script. Letters that do not decompose, such as `ł`, `ø` or `đ`, stay distinct.
Scripts without case, such as Chinese, Japanese and Korean, match as typed. The
highlight in a result title covers the original characters that matched.

Applications use their existing desktop-entry launch behavior through
`gio launch`. Entries with `Terminal=true`, such as htop or nmtui, are listed and
open in the user's terminal through `xdg-terminal-exec` when it is installed;
otherwise GLib's own list of known terminals is tried. Entries whose `TryExec`
program is missing from `PATH` or not executable are hidden. Before handing a
launch to Niri, the launcher rereads the entry and checks its `TryExec` and `Exec`
programs and, for terminal entries, that a terminal launcher exists. A failure
keeps the panel open and names the application and reason, such as “Could not
start htop: it runs in a terminal and no terminal launcher is installed; install
xdg-terminal-exec”. A spawn that Niri rejects or does not acknowledge is reported
the same way. Programs are resolved against Shell's `PATH`, while the app inherits
Niri's environment. A program that starts and then exits or crashes is not
reported, because Niri discards its output.

Files and folders
open through their default desktop handlers; ordinary text paths also offer Copy
path. Files and folders also offer Reveal in file manager. Files offer Open with
and Quick Look. Open with searches file-capable installed applications within the
same panel, with MIME recommendations first; Escape clears its query, then returns to the original file search.
Choosing an application opens that file without changing MIME defaults. Quick Look
opens a Files Browser Window selecting the file, followed by its existing owned
preview. Dismissing the preview returns focus to that Browser Window. Missing files
show an error instead of previewing a neighboring file. Non-UTF-8 paths can be opened using their original bytes but omit Copy path.
Displayed escaped/replacement characters never become the activation path.

Type `12 * (8 + 3)` or `= 12 * (8 + 3)` to get `132`. Enter copies the result.
Arithmetic supports `+`, `-`, `*`, `/`, `^`, parentheses, signed numbers, and
decimal points or decimal commas, postfix `%`, `pi`, `e`, and the functions
`sqrt`, `abs`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `ln`, `log10`, `exp`,
`floor`, `ceil`, and `round`. Functions require parentheses; trigonometry uses
radians. For example, `sqrt(144)` gives `12`, `100*25%` gives `25`, and `200+10%`
gives `220`. Function results can be amounts in conversions, such as
`sqrt(144) m to cm`. Both decimal separators are interpreted as
decimal points; thousands separators and scientific-notation input are unsupported.
Explicit `=` mode shows incomplete/invalid expression and dependency errors.
Calculations require `qalc` from `libqalculate`, included in the Arch Canopa package.
The adapter accepts numeric arithmetic and a defined unit-conversion grammar,
isolates calculator state, and disables network updates. It does not expose the
calculator command language.

### Unit conversions

Type a numerical amount, a source unit, `to` or `in`, and a target unit. `->` and
`→` also work as conversion separators. Conversions are recognized during ordinary
search and in explicit `=` mode. Enter copies the displayed value including its
unit, using the same calculator action.

| Query | Result |
| --- | --- |
| `10 cm to m` | `0.1 m` |
| `1 mile in kilometers` | `1.609344 km` |
| `32 F to C` | `0 °C` |
| `1 GiB to MiB` | `1024 MiB` |
| `36 km/h -> m/s` | `10 m/s` |
| `1 m² to cm^2` | `10000 cm²` |
| `(5+3) ft to m` | `2.4384 m` |

Supported units include:

| Quantity | Symbols and common English names |
| --- | --- |
| Length | `m`, `km`, `cm`, `mm`, `µm`/`um`, `nm`, `in`, `ft`, `yd`, `mi`; meters/metres, inches, feet, miles, and their usual plural forms. |
| Mass | `kg`, `g`, `mg`, `µg`/`ug`, `lb`, `oz`, `t`; grams, pounds, ounces, metric tonnes. |
| Time | `s`, `ms`, `µs`/`us`, `min`, `h`, `d`, `week`; seconds, minutes, hours, days, weeks. |
| Temperature | `C`/`°C`/Celsius, `F`/`°F`/Fahrenheit, `K`/Kelvin; absolute temperatures with offsets. |
| Information | `bit`/`b`, `kbit`/`kb`, `Mbit`/`Mb`, `Gbit`/`Gb`, `Tbit`/`Tb`; `B`, `kB`/`KB`, `MB`, `GB`, `TB`, `KiB`, `MiB`, `GiB`, `TiB`; bit/byte names. |
| Area and volume | Length units squared/cubed (`m²`, `ft^2`, `m³`), `ha`, `acre`, `L`, `mL`, `gal` (US gallon); square/cubic meter names. |
| Pressure | `Pa`, `kPa`, `bar`, `atm`, `psi`; pascals, bars, atmospheres. |
| Energy and power | `J`, `kJ`, `cal`, `kcal`, `Wh`, `kWh`, `W`, `kW`; joules, thermochemical calories, watt hours, watts. |
| Angle and speed | `rad`, `deg`/`°`, `mph`; radians, degrees, miles per hour, and compound ratios such as `km/h` or `m/s`. |

Symbols are case sensitive: `MB` is megabytes and `Mb` is megabits. English unit
names are case insensitive. Decimal information prefixes use powers of 1000;
binary prefixes such as `MiB` use powers of 1024. Unit ratios may use `/` or `per`,
including rates such as `MB/s` and powers such as `m/s^2`. Temperature units are
supported on their own, without powers or ratios. Numerical amounts may include
the arithmetic operators above; compound quantities such as `5 ft 2 in` are not
supported. Currency conversion is not included.

Units must measure compatible quantities. `= 1 m to kg`, for example, shows an
error and offers no copy action. An incomplete or unsupported conversion is quiet
during ordinary search; explicit `=` mode explains it.

Recognized calculations and bundled web/date answers lead. Native application, window, file and control
results share 20 visible slots, ordered by match quality, then recent/frequent
selection, then stable source order. Exact and prefix matches stay ahead of weaker
matches regardless of history. Each configured extension then contributes up to
20 results in configuration order. Refine the query to narrow additional matches.
Once you navigate, late results preserve the selected result's identity.
An old result cannot execute after the query or displayed results have changed.

## Controls and selection history

Search `settings` to browse shortcuts to Wi-Fi, sound, notifications/Do Not
Disturb, and Settings pages for displays, input, personalization, and power.
These open their existing panels or application pages; no setting changes merely
by searching. Existing extensions named `windows` or `settings` keep their keyword.

Successful application and file selections improve ranking among comparable
matches. The shell stores at most 256 identities, counts, and selection order in
`$XDG_STATE_HOME/canopa/shell/launcher-history.json` (default
`~/.local/state/canopa/shell/launcher-history.json`), using owner-only permissions.
Ranking metadata is separate from pins, aliases, and clipboard data. Deliberate
pins and aliases store their chosen label and target separately.
Search `settings clear launcher history` to erase it, or disable learning and
ranking for this source:

```kdl
source "launcher" {
    history #false
}
```

Disabling history leaves the existing file untouched. Clear history erases saved
selections and waits for the serialized write before reporting success.

## File coverage

The initial file-name index covers the home directory. Hidden entries, symlinks,
other filesystem devices, and entries named `target`, `node_modules`, `vendor`, or
`build` are excluded. Contents are not read or indexed. An unreadable location is
skipped and reported without disabling the other sources.

The shell builds a memory-only path cache off the UI thread. It reuses the cache
when reopening the same launcher configuration and refreshes it every 60 seconds
while open. Newly created, renamed, or deleted paths become searchable after a
refresh; opening rechecks existence. No indexing service runs while the launcher
is closed. Each scan stops at 100,000 paths, ten seconds, or 32 directory levels;
partial indexes are indicated in the panel. Files are fuzzy-matched by name first,
with parent-path matches as secondary text, using the same case and accent
folding as other results.

Override roots and extend basename exclusions within the source:

```kdl
source "launcher" {
    root "/home/alex/Documents"
    root "/home/alex/Projects"
    exclude "dist" "generated"
    max-paths 200000
}
```

Declaring roots replaces the default home root; `root "~"` explicitly selects
home. Other roots must be absolute paths. Exclusions add to the defaults.
`max-paths` accepts 1 through 1,000,000. Changing configuration retires the active
session; cached results from different root/exclusion settings are not reused.

## Executable extensions

An extension is an explicitly configured executable that receives one query and
returns declarative results. It runs as the user and can be written in any
language. Configuration uses literal argv, without shell expansion:

```kdl
source "launcher" {
    extension "Projects" keyword="p" {
        argv "python3" "/absolute/path/to/launcher-projects.py" "/home/alex/Projects"
    }
}
```

Use `p query` to search that extension. `global=#true` also sends it unscoped
queries. By default, an extension receives only queries for its keyword. Names and
keywords are unique within the source; `apps`, `files`, and `=` are reserved.
Up to eight extensions may be configured. An example is provided in
[`launcher-projects.py`](../../crates/canopa-shell/examples/launcher-projects.py).

After a 100 ms debounce, the shell launches one process per query, writes this
JSON line to stdin, then closes stdin:

```json
{"protocol_version":1,"query":"canopa","limit":20}
```

The process writes one JSON line to stdout and exits successfully:

```json
{"protocol_version":1,"results":[{"id":"project-canopa","label":"Canopa","description":"Project · /home/alex/Projects/canopa","icon":"folder","actions":[{"label":"Open","action":{"type":"open","path":"/home/alex/Projects/canopa"}},{"label":"Copy path","action":{"type":"copy","text":"/home/alex/Projects/canopa"}}]}],"more":false}
```

Result IDs are unique within the response and should stay stable for the same
item across queries. `description`, `icon`, and `more` are optional. Icons are
theme names, not file paths. Actions are ordered; the first is the default:

| Type | Fields | Behavior |
| --- | --- | --- |
| `open` | `path`: absolute local path | Open with the desktop's default handler. |
| `copy` | `text`: string | Copy plain text using the shell's clipboard. |
| `open_url` | `url`: HTTP(S) URL without credentials | Offer opening in the default browser. |
| `launch` | `argv`: array of literal strings | Hand a program to the desktop session so it can outlive the panel. |

An extension searches during a query and performs no domain action until the user
selects a returned action. Actions are executed by the host; this first protocol
has no activation callback or persistent extension process. Persistent indexes
can be owned by an extension's separate backend. Whole-panel interactions continue
to use the [external panel provider contract](external-panel-providers.md).

Each process has a two-second deadline and a 256 KiB stdout limit. A response has
at most 20 results and four actions per result. IDs are 1–128 bytes, labels at most
512 bytes, descriptions at most 2048 bytes, action labels at most 64 bytes, and
copied text at most 16 KiB. Launch argv has at most 64 strings of at most 4096 bytes
each and cannot contain NUL bytes. Queries are limited to 1024 bytes. Diagnostics
must go to stderr; stderr is discarded by this protocol version.

Typing a new query or closing the launcher cancels the previous process and its
owned descendants. A malformed response, missing executable, or timeout produces
a provider status message while other sources remain available. This is process
isolation, with the permissions of the configured user executable.

## Web, dates and time

The launcher bundles two executable extensions using the same JSONL protocol as
configured providers. Their results appear in the same Applications panel.

- Paste `https://example.com/path` to offer Open in browser and Copy URL.
- `web rust async tutorial` searches using the configured search URL. The default
  is DuckDuckGo. `wiki term` is a bundled keyword shortcut; additional keywords
  such as `docs` can use any HTTP(S) search URL containing one `{query}` placeholder.
- Ordinary application/file queries do not become web searches. Typing constructs
  a result only; the browser request starts when you select Open in browser.
- `time in Tokyo`, `time in Europe/Paris`, or `date time in UTC` copies a zoned time.
  IANA identifiers and unambiguous city names use the installed system zone catalog.
- `today`, `tomorrow`, `yesterday`, `90 days from today` and `2 weeks ago` copy a
  calendar date in the local time zone. Day/week amounts may be signed integers.
- `timestamp now` copies Unix seconds. `timestamp 0` gives
  `1970-01-01T00:00:00Z`; `timestamp 2026-11-03T12:00:00+01:00` gives Unix seconds.
  ISO input requires an explicit UTC offset; ambiguous local date/time input is
  unsupported. Relative dates are evaluated when the query runs.

Date/time is local and requires system time-zone data; zoned formatting also uses
GNU `date` from coreutils. Explicit `date` mode accepts the same examples without
sending other searches to a remote service. Disable either bundled extension or
change its keyword in Settings. Saved shortcuts are independent of provider
querying. Manage them with **Unpin from Launcher** or **Edit search alias**.
Up to eight configured executable extensions and 16 additional web keywords are
supported alongside the two bundled extensions.

## Search actions, pins and aliases

Select a result and press Tab, or choose its action menu button. Type an action name such
as `copy`, `open with`, `pin`, or `alias`, then press Enter. The original result
remains the action target. Escape clears the action query, then returns to the
original search. Action selection and alias editing use the existing panel.

Applications, files, URLs, desktop controls and literal launch actions offer
**Pin in Launcher** and **Set search alias**. Pins rise ahead of other
matches of comparable quality and appear in the empty-query list. Exact/prefix
matches still precede weaker matches.
**Set search alias** opens an editor. Type a unique single word such as `work` and
press Enter. Searching that alias offers the saved target directly. Use **Edit
search alias** with an empty value to remove it. **Unpin from Launcher**
removes the pin. Clear selection history leaves deliberate pins and aliases
intact. Calculator/clipboard values
and transient window IDs are not saved as shortcuts; a window's application can
be pinned when known.

Saved shortcuts retain the selected label and target, including original path
bytes, URLs or literal command arguments. The clipboard scopes below handle
recent copies and saved text snippets separately.

## Extension preferences

Search `settings launcher extensions`, or run `canopa-settings --page launcher`.
The existing Settings application lists bundled and configured extensions, their
keywords, available/observed action names and last safe provider status. Configured
providers report their action names after returning results. Start Shell to publish
the actual configured catalog. Refreshing discovery preserves typed drafts.

Enable/disable extensions, change routing keywords, edit the default web search URL,
and add/remove keyword search templates, then Save changes. This does not replace
executable argv in the Shell source configuration. Changes apply to Shell within
about one second and cancel affected queries. Configured extension names identify
preferences across menus; duplicate enabled keywords are rejected when saving.
An existing extension using `web` or `date` takes precedence over the bundled route;
rename or disable the conflicting route in Settings to resolve the status message.

Preferences are bounded owner-only JSON in
`$XDG_CONFIG_HOME/canopa/launcher.json` (default `~/.config/canopa/launcher.json`).
Settings rejects stale extension/search edits and preserves pins/aliases updated
by Shell while the editor was open. Invalid preferences leave the last valid Shell
snapshot active and show an error. Shell publishes descriptions/action names/status
without queries in `$XDG_STATE_HOME/canopa/shell/launcher-extensions.json`.

## Emoji and symbols

In Applications, type `emoji` or `symbol` to browse suggested characters. Add
a name or keyword to narrow the list. Try `emoji check mark button`,
`emoji thumbs up medium skin tone`, `symbol em dash` or `symbol infinity`.
Press Enter to copy the selected character or complete emoji sequence.

Search works offline with a bundled Emoji 17.0 catalog and English keywords.
Glyph appearance depends on installed fonts. A configured extension that uses
`emoji` or `symbol` keeps priority for searches with that keyword.

## Clipboard history and snippets

Open **Canopa Settings → Launcher → Clipboard history** to choose whether to
record history, include images, and retain between 1 and 64 recent entries.
Select **Save clipboard settings** to apply those options to the running Shell.
Both recording and image capture are off by default. You can also type `clip`
in Applications and select **Turn on clipboard history**.

Use `clip invoice` to find recent text and `clip PNG` to find PNG images. Image
rows show a thumbnail, format and dimensions. Enter copies the selected entry
to the system clipboard. Images retain their original bytes and format. Tab
offers **Remove from history** for either kind and **Save as snippet** for text.
**Clear clipboard history** removes all recent copies. Turning off recording
clears the list, while turning off image capture clears its images.

History deduplicates copies and keeps them in memory for the Shell session.
Text entries are limited to 16 KiB of UTF-8. Image history accepts PNG, JPEG, GIF
and WebP offers up to 8 MiB and 4096 pixels per side, with a 32 MiB total budget
for encoded images and thumbnails. Shell skips offers marked sensitive.
Recording requires wl-clipboard and Wayland data control support. Enabled
recording can capture the current clipboard selection when Shell starts.

Type `snip` and select **New snippet from clipboard** to save the current text
with a name. This works with recording off. For example, `snip invoice footer`
searches snippet names and text. Enter copies the original text, including
line breaks and indentation. Use Tab to rename or delete a snippet.

The `recording`, `images` and `max_entries` preferences share an owner-only
file with up to 64 named text snippets. The file is
`$XDG_STATE_HOME/canopa/shell/clipboard.json`, with
`~/.local/state` as the default state directory. `clip` and `snip` are reserved
local search keywords.
