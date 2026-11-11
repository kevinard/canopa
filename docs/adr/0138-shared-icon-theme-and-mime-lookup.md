# 0138 Shared icon-theme and MIME lookup

Status: accepted.

The launcher, Notify, Files, the portal and the Shell's menus, tray and
focused-window icons already shared the bounded lookup in `canopa-assets`
([ADR 0005](0005-shared-assets.md)). Each caller pinned it to `hicolor`,
rejected absolute `Icon=` paths, and Notify accepted only PNG/JPEG files with an
extension. Files chose 8 icons by extension. This decision extends that one
lookup instead of adding another, and adds shared-mime-info detection beside it.

## Placement

| Piece | Crate | Notes |
| --- | --- | --- |
| Icon-theme selection | `canopa-file::icon_theme` | No UI or image dependency, so Notify core can use it |
| MIME detection | `canopa-file::mime` | `globs2`, `magic`, aliases, subclasses, `icons`, `generic-icons` |
| Icon lookup and cache | `canopa-assets::icons` | `IconLookup` (one batch), `IconResolver` (cached), `IconSource` |
| SVG/raster loading by content | `canopa-assets` | `Formats::SniffedPngJpeg`, `RasterBatch::load_image_or_svg`, `svg::is_svg` |
| Iced handles | `canopa-ui::icons` (feature `icons`) | `handle`, `trimmed_handle` |

The lookup and detection are UI-free. Core and headless profiles are unchanged:
`check-core-dependencies.sh` and `check-provider-dependencies.sh` pass.

No crate was added. `freedesktop-icons` and `xdg-mime` were considered. The
existing lookup already had per-batch budgets, cancellation and safe index
parsing, which neither crate offers. Detection needs only the compiled
shared-mime-info files, and reading them with Canopa's bounded reads keeps
every input size-limited.

## Source of truth for the icon theme

GTK's `settings.ini`, key `gtk-icon-theme-name` in `[Settings]`: GTK 4 before
GTK 3, in `$XDG_CONFIG_HOME` before each `$XDG_CONFIG_DIRS` entry. Canopa's
shared configuration and its Settings portal don't own an icon theme, and GTK
applications on this session read the same file, so Canopa and GTK agree.
dconf (`org.gnome.desktop.interface icon-theme`) and KDE's `kdeglobals` are not
read. Without a selection the theme is `hicolor`. Notify's explicit
`icons theme=` still overrides; without it Notify follows the desktop.

## Icon lookup

- **Order:** the selected theme, then its inherited themes depth-first, then
  hicolor, then unthemed pixmaps, as the Icon Theme Specification orders a list
  of names. Every candidate name is tried in a theme before moving to the next
  theme. Parents load only after the selected theme misses.
- **Sizes and scales:** exact nominal size and scale first, then the nearest
  physical size, as before. The declared directories that exist are recorded
  once per theme, so a miss costs one check per existing directory instead of
  one per root.
- **Formats:** PNG, SVG and XPM paths (XPM is found but not decoded). Loaders
  detect SVG from content, never the name.
- **References:** `IconSource::parse` accepts absolute paths and local
  `file://` URLs, and drops a legacy `.png`/`.svg`/`.xpm` from names, as GIO
  does for desktop entries.
- **Generic fallbacks (opt-in):** after the given names, `a-b-symbolic` tries
  `a-symbolic`, `a-b` and `a`; `a-b-c` tries `a-b` and `a`. Notify uses this
  for sender icon names; launcher app icons and MIME icons don't.
- **Cache:** `IconResolver` keys results by theme, names, size, scale and
  options. Misses are cached too; cancelled lookups are not. At most every two
  seconds it checks the modification times of the settings files, each icon
  root, and each loaded theme directory. A change drops the cache and rereads
  the theme. `gtk-update-icon-cache` and package installs replace the theme
  directory's cache file, so they count as a change. Adding a file in an
  existing size directory without that step is seen only after another change.

Notify keeps uncached per-frame `IconLookup` batches. Its contract, covered by
tests, is that files are observed when a banner is prepared.

## MIME detection

Globs follow the specification: weights, the `cs` flag, `__NOGLOBS__`, and the
highest weight then the longest pattern. Literal and `*suffix` patterns use
hash lookups; others use fnmatch. A higher-precedence directory replaces a
type's globs and magic from lower ones. Results are unaliased.

Detection follows the recommended order:

1. A unique glob wins without reading content.
2. Otherwise magic (priorities, masks, word sizes, ranges, nested matchlets)
   runs on at most a 4 KiB prefix. A glob equal to or inheriting from the
   magic result wins. With no such glob, the first glob is used; with no glob
   at all, the magic result.
3. Otherwise `text/plain` or `application/octet-stream` from the first 128
   bytes, and `application/x-zerosize` for empty files.

Subclasses include the implicit `text/*` and `application/octet-stream`
parents. Icon names come from the `icon` element, then the type with `/`
replaced by `-`, then `generic-icon` or `media-x-generic`.

## Adoption

- **Launcher and Shell:** themed icons, absolute and `file://` `Icon=` paths,
  SVG or raster by content, and double-density sources for the 72 px rows. The
  tray and focused-window icons use the same resolver. The tray's SNI
  `IconThemePath` is still ignored.
- **Notify:** banners and the Center follow the desktop theme. `app_icon` and
  `image-path` files are admitted by content (PNG, JPEG or SVG) with the
  existing size and decoder limits, so Chromium and Electron's extensionless
  temporary images render. Sender icon names use generic fallbacks. Without a
  sender icon, the identity's installed desktop entry `Icon=` comes before the
  identity names (`canopa_apps::desktop_entry_icon`). Body `<img>` keeps its
  PNG/JPEG-with-extension rule.
- **Files:** each listed entry gets a type: a unique glob in the view, or
  content sniffing on a worker for missing or ambiguous names (1,024 files per
  request), keyed by path and modification time. The theme's type icon is
  prepared off-thread. The 8 built-in glyphs become the fallback for a type
  category when the theme has no icon. `FileVisual` is the seam for
  thumbnails: prepared pixels keyed by path and modification time would come
  before the type icon.
- **Portal:** AppChooser icons follow the desktop theme.

## Not done

- dconf and KDE icon-theme settings.
- Per-output scale for the Shell and Files: they use fixed source scales.
- The FileChooser portal's own `globs2` filter parsing: it could move to
  `canopa_file::mime`.
- Thumbnails: done in Files [ADR 0167](../files/adr/0167-sandboxed-thumbnails.md) over the shared cache of [ADR 0141](0141-mime-applications-and-thumbnail-cache.md).
