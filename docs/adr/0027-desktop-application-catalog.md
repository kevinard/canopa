# 0027: Share desktop-entry metadata without sharing application lifetimes

Status: accepted; shared validation in progress, 2026-10-29.

The AppChooser portal needs localized application names and icons for the exact
application IDs supplied by the frontend. Shell already discovers desktop entries
for its application menu. Move that renderer-independent parser/discovery code to
`canopa-apps`, keeping its MIT licence and tests. Shell retains a re-export at its
existing module seam; launch behavior, observation, menus and caches stay owned by
Shell. The portal must not depend on the Shell application or copy its parser.

Keep the current launcher selection behavior. Add a separate metadata projection
for explicit chooser candidates: NoDisplay, Terminal and desktop-specific menu
visibility do not erase labels for a frontend-supplied handler. Hidden user
overrides continue to suppress lower-priority system entries. This metadata API
does not choose applications, authorize a launch, change MIME defaults or run an
Exec command. The chooser's request model owns its offered/selected IDs.

Desktop-entry reads use the shared bounded regular-file reader, with a 256 KiB
limit. Invalid UTF-8, oversized sources and special files are skipped without
blocking on a FIFO or logging source contents. Consumers keep filesystem work on
workers. Existing XDG precedence, locale fallback and desktop-file identity are
retained by the moved tests; chooser-specific metadata behavior gets its own
coverage.

This is shared infrastructure for the portal roadmap, not a merger of Shell and
portal lifetimes. See [ADR0026](0026-file-chooser-portal.md) for the dialog backend
and [the implementation map](../../.scratch/xdg-portals/ROADMAP.md) for the broader
Canopa portal goal. The workspace gate must be rerun after this shared phase is
integrated; the earlier FileChooser gate result remains recorded separately.
