# 0176 Recent files

Status: accepted 2026-11-14. Builds on the search session of
[0058](0058-iced-filename-search.md) and the filters of
[0175](0175-search-filters-and-contents.md).

GNOME Files, COSMIC Files and Thunar (through GTK) list recently used files.
Files had no way back to "the document I opened an hour ago" apart from
remembering its folder.

## Decision

- **Source.** Files reads the desktop's shared list,
  `$XDG_DATA_HOME/recently-used.xbel` (`~/.local/share` by default), which GTK,
  Qt/KDE and most applications record opened files in. It does not keep a
  separate history. Only local `file:` items are shown. An item's time is the
  latest of its `visited`, `modified` and `added` times. Items are newest first,
  each once, at most 500. A missing, unreadable, malformed or oversized list
  (over 16 MiB) shows nothing.
- **Presentation.** Recent is a built-in place, first in the default sidebar and
  offered in Preferences and the Locations menu. It is also the `show-recent`
  command, with no default shortcut. It opens in the current tab as a search
  whose scope is the recent list, so the existing result list, selection,
  Quick Look, Reveal in folder, Stop and stale-result rules apply unchanged. The
  query field reads "Search recent files". The query and the type, date and size
  filters narrow the list, and an empty query shows all of it. Include subfolders
  is hidden. The window title names Recent instead of the folder.
- **Order.** Recent results stay in recency order. Folder searches stay sorted
  by name.
- **Items that are gone** are left out silently rather than reported as
  unreadable locations: an old entry for a deleted file is not a problem the
  person can act on.
- **Writing.** Superseded by [0178](0178-recording-recent-files.md): Files
  records the documents it opens with an application. Removing or clearing
  items is not offered.

Existing configurations keep their place lists. Recent appears in new
configurations and can be added from Preferences.

## Verification

- `recent::tests` cover parsing, newest-first order, deduplication, raw path
  bytes, non-local items, undated items and malformed or missing lists.
- `search::tests::recent_lists_existing_items_newest_first_through_the_query_and_filters`
  covers recency order, left-out missing items, the query and a type filter.
- `desktop::search_tests::recent_files_open_as_a_query_free_search_without_the_subfolder_scope`
  covers the command, refusal of folder results, narrowing by query, and the
  window title.

Not verified: the place and result list in a native session.
