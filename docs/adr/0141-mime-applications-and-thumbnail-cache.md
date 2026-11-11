# 0141 MIME applications and the thumbnail cache

Status: accepted.

Files needed to know which applications open a type, to change the default, and
to read and write the freedesktop thumbnail cache. The launcher and the portal
need the first; the FileChooser portal could use the second. Both now live in
shared crates beside the lookups of [ADR 0138](0138-shared-icon-theme-and-mime-lookup.md),
without GIO and without a new crate dependency.

## Applications for a type: `canopa_apps::mime_apps`

The XDG MIME Applications Associations specification over the same desktop-entry
discovery as `canopa_apps` (ADR 0027):

- **Lists**, highest precedence first: `$XDG_CONFIG_HOME`, each
  `$XDG_CONFIG_DIRS`, `$XDG_DATA_HOME/applications`, each
  `$XDG_DATA_DIRS/applications`. In each directory every
  `<desktop>-mimeapps.list` for `XDG_CURRENT_DESKTOP` comes before
  `mimeapps.list`. Lists are bounded (1 MiB); a repeated key keeps its first value.
- **Associations** for a type: per directory, its lists' `[Added Associations]`,
  then the entries that directory defines (after ID shadowing) whose `MimeType=`
  lists the type. A `[Removed Associations]` entry hides the application in the
  same and lower directories, unless a higher list adds it again. Entries with
  `NoDisplay=true` are offered only when a list names them; a missing `TryExec`
  program hides an entry.
- **Default**: an installed `[Default Applications]` choice for the type, then
  for each ancestor, then the most preferred association of the type, then of each
  ancestor: GLib's `g_app_info_get_default_for_type` order.
- **Ancestors** (`type_chain`): the unaliased type, then its shared-mime-info
  parents breadth-first, with `text/plain` for `text/*`. `application/octet-stream`
  is left out unless it is the type, or hex editors would open everything.
- **Writing** (`set_default`): GLib's record of a user choice. In
  `$XDG_CONFIG_HOME/mimeapps.list`, `[Default Applications]` gets `type=id;`,
  `[Added Associations]` gets the ID first, and the ID leaves the type's
  `[Removed Associations]`. Every other line, comment and group is kept byte for
  byte; missing groups are appended. A desktop-specific user list that already
  names a default for the type is updated too, because it would win. The file is
  replaced atomically with its mode kept, and a symlinked list (dotfile
  managers) is written through to its target.

`MimeApps` reads everything once per call; callers load it on a worker. Files uses
it for Open, Open With and its chooser (Files ADR 0166). The launcher's Open With
and the portal's AppChooser keep their current lists; both can switch to
`applications_for` and `set_default` without another parser.

## Thumbnail cache: `canopa_assets::thumbnails`

The Thumbnail Managing Standard's storage, UI- and process-free:

- **Names**: `MD5(uri).png`, with the URI escaped exactly like GLib's
  `g_filename_to_uri` (only alphanumerics and `!$&'()*+,-./:=@_~` stay), so other
  desktops' thumbnails are found and Files' are reused by them. MD5 and the PNG
  CRC are implemented in the module (about 60 lines) rather than adding crates.
- **Sizes**: `normal` 128, `large` 256, `x-large` 512, `xx-large` 1024; a lookup
  accepts the requested size or larger.
- **Metadata**: `Thumb::URI`, `Thumb::MTime`, `Thumb::Size`, `Thumb::Mimetype`
  and `Software` as `tEXt` chunks inserted after `IHDR` of an `image`-encoded
  PNG; readers also accept uncompressed `iTXt`. A thumbnail is valid when URI and
  whole-second modification time match, and its size if recorded. Invalid
  entries are skipped, never deleted.
- **Writes**: directories `0700`, files `0600`, a temporary name in the same
  directory then a rename. Failures are 1×1 PNGs in `fail/<app>-<version>/`.
- **Thumbnailers**: `thumbnailers/*.thumbnailer` in the XDG data directories; a
  file name in a higher directory hides the same name below; for one type the
  first entry wins; `TryExec` and the program must exist. `Exec` is split like a
  shell word list without expansion, and `%i %u %o %s %%` are expanded per
  argument.

Running thumbnailers is the consumer's job; Files runs them in a Bubblewrap
sandbox (Files ADR 0167).

## Consequences

- Core and headless profiles are unchanged: `canopa-apps` gains no dependency;
  `canopa-assets` uses its existing `image` dependency.
- Files now depends on `canopa-apps`.
- ADR 0138's "Not done: Thumbnails" is resolved by Files ADR 0167.
