# 0175 Search filters and content search

Status: accepted 2026-11-14. Extends the filename search of
[0058](0058-iced-filename-search.md); its tab-owned worker, budgets, Stop,
More results and stale-result rules are unchanged.

Search matched filenames only. GNOME Files filters by type and date, PCManFM by
type, size, date and contents. Finding "the PDF I changed this week" or "the note
that mentions the invoice number" meant leaving Files.

## Filters

A band under the toolbar holds the filters while a search is open:

- **Type**: Any type, Folders, Documents, Images, Audio, Video, Archives. A file's
  type is its name's unique shared-mime-info type (the same `canopa-file`
  database as the icons), without reading the file. Images, Audio and Video are
  the `image/`, `audio/` and `video/` types. Documents are `text/` types, PDF,
  PostScript, EPUB, RTF and the OpenDocument and Office formats. Archives are
  the names Files can extract ([0172](0172-compressed-archives.md)) plus zip,
  7z, rar, xz, zstd, bzip2 and disc images. Names without a unique type match
  only Any type.
- **Modified**: Any time, Today (since local midnight), Past week, Past month,
  Past year (rolling 7, 30 and 365 days).
- **Size**: Any size, Under 1 MiB, 1 to 100 MiB, Over 100 MiB. Folders never match
  a size.
- **Search contents**: also match the query inside text files.

Filters apply to the name match: every filter must pass, then the name or (with
Search contents) the contents must contain the query. A Type, Modified or Size
filter also searches with an empty query, so "every image changed today" needs
no text. Search contents alone does not, since there is nothing to look for.
Clearing every filter with an empty query returns to the folder, as emptying the
query did before.

Filters belong to the tab's search session and reset when the search closes.
Results record the filters they were computed for, and results for other filters
are refused, as results for another query or scope already are.

## Contents

Content matching reads regular files up to 16 MiB whole, skips files with a NUL
byte in their first 8 KiB (binary), and compares case-insensitively through
Unicode lowercasing. Filters run first, so a type or date filter spares most
reads. Reads run on the tab's search thread and stop with Stop or a new query
between files.

Desktop indexers (localsearch/Tracker) are not used: the Canopa session does not
run one, and a direct read gives the same answer in any folder, including network
mounts and removable drives. PDFs, office documents and compressed files are not
text, so their contents are not searched. An indexer-backed search would be a
separate decision.

## Verification

`search::tests` cover each type class against a fixture type database, date and
size filters including folders and a sparse 200 MiB file, case-insensitive and
accented content matches, binary and oversized files, and the empty-query rule.
`desktop::search_tests` covers a filter-only search starting, refusing results
for the previous filters, and closing when the filters are cleared.

Not verified: the filter band in a native session, including its wrapping in
narrow tiles and screen-reader announcements of the lists.
