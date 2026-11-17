# Launcher

Canopa Shell's **Applications** menu is a unified launcher. It searches installed
applications, open windows, file and folder names, desktop controls, Settings pages
and session actions, and it calculates, converts units, searches the web, answers
date and time questions, and keeps clipboard history and snippets.

Open it with Super+Space, the **Search** button at the top of the bar, by typing in
Niri's overview, or with `canopa-shell --toggle-menu Applications`. It opens in the
bar's panel; `panel { launcher-width N; }` gives launcher menus their own width
(580 logical pixels in the shipped layout).

Any menu can use the launcher as its source:

```kdl
menus {
    menu "Applications" show-icons=#true prompt="Search apps and files" {
        source "launcher"
    }
}
```

`source "installed-apps"` is the application-only source. See the
[configuration reference](configuration.md#menus) for menus.

## Searching and actions

Type normally to search applications, open windows, controls, Settings pages,
session actions and file/folder names.
Choose a scope below the search field to narrow the current query. Choose **All**
to search across sources again; its results carry the matching scope's icon at
the row's end, so apps, files, windows and settings stay distinguishable. You can also type a scope directly, as in
`apps firefox`, `windows notes`, `settings volume`, or `files budget`.
The English scope keywords (`apps`, `files`, `windows`, `settings`, `clip`,
`snip`, `emoji`, `symbol`) always work; the session language adds its own
(`shell-keyword-*` in the Shell's translations), such as `fichiers budget` or
`presse-papiers` in French. An extension with the same keyword keeps priority.
Controls and session actions also match search words in the session language
beside the English ones.
Window rows show the application and workspace. Enter switches to the existing
window and a secondary action can launch another instance. Up and Down select a
result. Enter performs the selected result’s first action. Press Tab to filter the selected result’s
actions. Click a result to run its primary action. A selected result with more
than one action carries an Actions button (Tab) on its own row; the command bar
shows the primary action and available shortcuts such as Copy path. In mixed
results, a running application keeps the applications mark even though its first
action switches to its window; only open-window rows carry the windows mark. A
single application match can show its desktop-entry description.
Ctrl+Enter performs its second action; Ctrl+1 through Ctrl+5 perform the respective
named action. Escape clears a nonempty query, then closes on a second press.

Applications match on their name, generic name, comment and keywords (the session
language's `Keywords[xx]` plus the untranslated `Keywords`), and, with
the lowest priority, on their desktop-file id and the file name of their `Exec`
program: `nautilus` finds Files and `org.gnome.Nautilus` does too.

Matching ignores case and accents in every script: `терм` finds “Терминал”,
`cafe` finds “Café”, and `ρυθμισεις` finds “Ρυθμίσεις”. Text is lowercased with
Unicode rules (also folding `ß` to `ss` and final `ς` to `σ`), then accented
letters are reduced to their base letter, as canonical decomposition followed by
removal of combining marks would. A built-in table covers precomposed Latin (including Vietnamese),
modern Greek and Cyrillic letters; separate combining marks are dropped in any
script. Letters that do not decompose, such as `ł`, `ø` or `đ`, stay distinct.
Scripts without case, such as Chinese, Japanese and Korean, match as typed. The
highlight in a result title covers the original characters that matched.

### Browsing all apps

With an empty query the launcher lists Pinned results first, then up to five
Recent apps (from selection history, unless history is off), then every
installed app from A to Z. The list scrolls and has no row limit; Up and Down
move through it. Typing `apps ` (or choosing the **Apps** scope) with no search
term lists only the A–Z apps. Pin an app with **Pin in Launcher** in its actions
(Tab); pins are stored with the launcher preferences in
`$XDG_CONFIG_HOME/canopa/launcher.json`, and **Unpin from Launcher** removes
one. Open windows are not part of the empty list; type a name, or use the
`windows ` scope, to find them.

### Desktop actions and running apps

Desktop entries can declare extra actions in `[Desktop Action]` groups, such as
Firefox's “New Private Window”. They appear, with their localized names, as
further actions of the app's result: press Tab on the app, or use Ctrl+2 to
Ctrl+5. A query of three or more letters that names an action also lists it as
its own row, so `private` or `firefox private` offers “Firefox — New Private
Window”. Actions of D-Bus-activatable apps go through
`org.freedesktop.Application.ActivateAction`; other actions are started through
GIO like the app itself.

When an app already has a window, Enter on its result says **Switch to** and
focuses that window through Niri. The next action opens a new window: the
entry's own `new-window` action when it has one, otherwise a second launch.
Entries with `SingleMainWindow=true` offer only Switch to. A window belongs to
an app when its app id is the desktop-file id or the entry's `StartupWMClass`,
ignoring case.

### Launching

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

Each app runs in its own systemd user scope named
`app-canopa-<desktop id>-<random>.scope`, following systemd's naming convention
for desktop applications, for example
`app-canopa-org.mozilla.firefox-3f2a9c1d0b7e4a65.scope`. Characters other than
letters, digits, `:`, `_` and `.` in the id are escaped as `\xNN`. Niri spawns a
small `canopa-shell --app-scope` helper, which waits for Niri to place it,
moves itself into the named scope and then starts `gio launch`. Without a
systemd user manager the app starts in Niri's scope. List them with
`systemctl --user list-units 'app-canopa-*'`.

D-Bus-activatable apps receive an xdg-activation token in `platform-data`
(`activation-token` and `desktop-startup-id`), so an app that is already running
raises its window. The token comes from the key press or click that chose the
result, so keyboard-only launches get one too. **Reveal in file manager** passes
the same token as the `ShowItems` startup id, so an open Files window is raised.

### Files and folders

Files and folders open with their default applications. They also offer Copy path
and Reveal in file manager; files also offer Open with and Quick Look. Open with
searches applications that can open files in the same panel, with the file type's
recommended applications first; Escape clears its query, then returns to the file
search. Choosing an application opens the file without changing your defaults.
Quick Look opens a Canopa Files window that selects the file and shows its preview;
closing the preview returns to that window. A file that no longer exists shows an
error. Paths that are not valid UTF-8 open correctly but do not offer Copy path.

### Calculator

Type `12 * (8 + 3)` to get `132`; no prefix is needed, and the result leads the
other matches. `= 12 * (8 + 3)` does the same in explicit mode. Enter copies the
result. Arithmetic supports `+`, `-`, `*`, `/`, `^`, parentheses, signed numbers, and
decimal points or decimal commas, postfix `%`, `pi`, `e`, and the functions
`sqrt`, `abs`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `ln`, `log10`, `exp`,
`floor`, `ceil`, and `round`. Functions require parentheses; trigonometry uses
radians. For example, `sqrt(144)` gives `12`, `100*25%` gives `25`, and `200+10%`
gives `220`. Typed signs are accepted too: `×`, `·` and an `x` standing between
two operands (`3x4`, `3 x 4`) multiply, `÷` divides and `−` subtracts; a leading
`0x` is left alone. Function results can be amounts in conversions, such as
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
search and in explicit `=` mode. Results are shown with the session language's
grouping and decimal separator (`3,600,000 J`; `3 600 000 J` in French); Enter
copies the plain value with its unit (`3600000 J`), using the same calculator
action.

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

Recognized calculations and bundled web/date answers lead. While searching,
native application, window, file, control, Settings page and session results
share 20 visible slots, ordered by match quality, then recent/frequent
selection, then stable source order. Exact and prefix matches stay ahead of weaker
matches regardless of history. Each configured extension then contributes up to
20 results in configuration order. Refine the query to narrow additional matches.
Once you navigate, late results preserve the selected result's identity.
An old result cannot execute after the query or displayed results have changed.

## Controls and selection history

Search `settings` to browse shortcuts to Wi-Fi, sound, notifications/Do Not
Disturb, Bluetooth, power and brightness, and every Settings page. These open
their existing panels or application pages; no setting changes merely by
searching. Settings pages come from the Settings page registry: their titles,
descriptions and search keywords are matched, so `night light`, `resolution`
or `keybindings` find the page that declares them, and new pages appear without
a launcher change. The Shell reads the registry once with
`canopa-settings pages`, which prints it as JSON without opening a window, and
opens a page with `org.canopa.Settings.OpenPage`, starting Settings through
D-Bus activation if needed (or `canopa-settings --page ID` without a session
bus service).

### Session actions

Type `lock`, `log out`, `suspend`, `hibernate`, `reboot`/`restart` or
`power off`/`shut down` to get the session actions the bar's buttons offer.
Lock runs `canopa-lock` (or `loginctl lock-session` when Canopa Lock is not
installed). Suspend and Hibernate ask logind directly, like the bar buttons.
Log out, Reboot and Power off open the configured confirmation menu, the one
whose items log out (`session "log-out"`, or a configuration's own
`niri msg action quit`) or name `session "reboot"` or
`session "power-off"`; without such a menu they are not offered. Rows follow
logind's `Can*` answers, read again when the launcher opens: an action logind
refuses is hidden, and Hibernate needs a plain “yes”. Failures are reported
the same way as from the bar. A configured extension with the keyword `windows` or
`settings` takes priority over those scopes.

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

The file-name index covers the home directory. Hidden entries, symlinks,
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
[`launcher-projects.py`](../crates/canopa-shell/examples/launcher-projects.py)
(installed as `/usr/share/canopa-shell/examples/launcher-projects.py`).

After a 250 ms pause in typing, the shell launches one process per query, writes this
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
selects a returned action. Actions are executed by the Shell; protocol version 1
has no activation callback or persistent extension process. An extension that needs
a persistent index can keep it in a separate backend. Programs that need a whole
panel use the [external panel provider protocol](shell/external-panel-providers.md).

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

The launcher bundles two extensions that are configured like executable
extensions, but the Shell answers their queries itself after a 100 ms pause, so
typing starts no processes for them (zoned times still run GNU `date`).
Calculations wait for a 250 ms pause before starting `qalc`. Their results appear in the same Applications panel.

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

Select a result and press Tab, or choose the Actions button on its row. Type an action name such
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
The Settings **Launcher** page lists bundled and configured extensions, their
keywords, available/observed action names and last safe provider status. Configured
providers report their action names after returning results. Start Shell to publish
the actual configured catalog. Refreshing discovery preserves typed drafts.

Enable/disable extensions, change routing keywords, edit the default web search URL,
and add/remove keyword search templates, then save. Executable argv stays in the
Shell configuration. Changes apply to Shell within
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

History deduplicates copies.
Text entries are limited to 16 KiB of UTF-8. Image history accepts PNG, JPEG, GIF
and WebP offers up to 8 MiB and 4096 pixels per side, with a 32 MiB total budget
for encoded images and thumbnails. Shell skips offers marked sensitive.
Recording requires `wl-clipboard` (`wl-paste` and `wl-copy`) and Wayland data
control support. History is kept in memory only and is lost when the Shell
restarts. Enabled
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

## Known limitations

- File search indexes names only, refreshes every 60 seconds while open, and stops
  at 100,000 paths per scan by default.
- Calculations need `qalc`; zoned times need GNU `date`.
- Currency conversion, compound quantities (`5 ft 2 in`) and scientific-notation
  input are not supported.
- Clipboard history is not kept across Shell restarts.
