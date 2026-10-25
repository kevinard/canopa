# Live Files configuration and appearance reload

Status: accepted under delegated implementation authority, 2026-10-16.

Files observes saved shared configuration and its external theme dependency.
It retains the original selection: discovery is repeated, an explicit path stays
explicit, and `--defaults` never reads ambient configuration or starts a watcher.
Graphical saves preserve discovery rather than pinning the file found on startup.
This extends shared ADR 0002 and supersedes the deferred automatic-reload portions
of Files ADRs 0009 and 0011. Each application still installs independently.

The toolkit-independent configuration runtime polls on a worker after a 250 ms
interval. Two equal observations admit one bounded, complete candidate. Only one
preparation runs at a time. Observations include resolved selection, canonical
target, device/inode, size and modification/change times for the main source and
theme. Re-resolving paths handles atomic saves, parent-directory replacement,
symlink retargeting, initially absent files and creation of a missing dependency.
Canonical targets distinguish retargeting between existing hard links even when
the followed inode and all metadata remain identical. The shared reader retains
its descriptor checks and combined 1 MiB source/theme budget.

One captured main input now supplies Files settings, actions and shared appearance
together. A read, parse, Files-schema or appearance error retains the whole prior
candidate. A separate browser banner explains that the last valid settings remain
active; normal operation/navigation errors keep their own presentation. A stable
failed edit is reported once and is retried after its source or dependency changes.
Diagnostics never interpolate source values. A valid repair clears the banner.

Installation distinguishes source changes from effective settings and appearance
changes. Comment-only or unrelated-namespace edits update the source without
rebuilding browser views or disturbing in-progress location input. Files changes
refresh all windows/tabs and retain the established sparse folder overrides.
Appearance changes update one existing CSS provider and existing syntax-color
tags. They retain Quick Look's source, window, focus, PDF page, zoom and media
objects rather than reconstructing the preview. No periodic watcher holds the
application alive after its ordinary windows/jobs finish.

Open editors keep every field and draft through external updates, including text
not yet committed by a spin control. Their status explains the source changed and
offers explicit Reload. Existing source-byte and canonical-target guards refuse a
stale main-file save. Reload asks before discarding changes. The action editor now
supports Ctrl+R consistently with Preferences. An editor's successful save/reload
updates its own snapshot before publishing, avoiding a false self-stale warning.

The desktop increments an installation revision for graphical and automatic
installs. If a worker result predates a graphical install, it is discarded and the
runtime is invalidated. Its next settled valid source must be published even if
the worker already accepted that same candidate internally. This prevents a late
poll from overwriting a newer graphical result or leaving the source undisplayed.

Five public filesystem regressions cover complete installation, rejection and
repair; missing/changed/invalid themes; parent-directory replacement; symlink
retargeting to an existing hard link; stale-draft refusal; discovery after graphical
save; dangling-link repair; explicit missing sources; isolated defaults; and
in-flight invalidation. The hard-link and invalidation cases both reproduced
failures before their fixes. Native acceptance exercises actual repaint and input
through `verify-files.py --reload-only`; the maintained desktop inventory includes
dark and light/fractional scenarios. Exact completed runs and broader outstanding
work remain in the implementation ledger.

The complete core suite passes 97 tests; the final hard-link regression and its
four configuration companions also pass after the observation fix. The nine-check
native runs `native-reload-05` (dark) and `native-reload-06` (light, 1.25×) pass
with clean GTK logs. `native-reload-08` (dark) and `native-reload-09` (light, 1.25×)
pass the final ten checks, adding location input during an effective Files change.
Both final logs are clean. Rendered inspection confirms the held PDF page,
uncommitted preference value, unchanged action draft, explicit reloaded action
and separate invalid-candidate banner. `native-acceptance-26` passes all 27 existing
browsing, preview, clipboard, transfer, lifetime, Undo and Trash checks with a
clean GTK log. Build, all-target Clippy, formatting and dependency attribution
checks pass. Actual audio/video playback and removable-drive acceptance remain
separate work; retaining a media object through appearance reload does not by
itself prove playback behavior.
