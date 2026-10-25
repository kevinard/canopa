# XDG personal-folder locations

Status: accepted under delegated implementation authority, 2026-10-18.
Implementation and scoped public/native verification pass; full product gates
remain open.

Desktop, Documents, Downloads and Pictures use the user's XDG directory
definitions. Home and Filesystem retain their existing meanings; bookmarks
retain their explicit paths. The sidebar and narrow Locations popup consume the
same application snapshot. A place label never identifies its filesystem path.

Resolve the user configuration root with `canopa-config`'s existing absolute
XDG_CONFIG_HOME/HOME policy. Read `user-dirs.dirs` through `canopa-file`'s bounded
regular-file reader, with a 64 KiB limit, on a startup worker before opening the
first Browser. Views and event handlers only read the prepared snapshot. The
snapshot lasts for the application process; changed system directory definitions
take effect on its next launch.

Follow the documented quoted absolute-path or leading `$HOME/` format, retaining
Unix bytes and escaped path characters. Accept an explicit Home mapping, including
`$HOME` itself. Parse these assignments as data: do not source shell text, expand
other variables or execute substitutions. Ignore unknown names and malformed
assignments; the last valid assignment for a recognized place wins. Missing,
unreadable, non-regular or oversized sources use conventional Home subfolders.
A place without a valid definition retains its conventional fallback. Preserve
separators as decoded, including the trailing slash of an explicit `$HOME/`.

Configured paths may be outside Home, missing, or equal to Home. Preserve them
without creating directories or silently substituting an existing folder. Normal
navigation reports an unavailable destination. Users control which built-in
places appear through Files Preferences; loading the definitions does not alter
their ordered Locations settings.

Use a small application-owned byte parser for this documented format and the
existing shared read/configuration boundaries. This requires no GUI toolkit,
shell process, dependency fork or new package. Public tests use actual private
definition files; native checks must navigate the configured paths from the
sidebar and narrow menu, including a raw Unix path.

Reference: [freedesktop.org XDG user directories](https://wiki.freedesktop.org/www/Software/xdg-user-dirs/)
and the installed `user-dirs.dirs(5)` format documentation. Full preference
accessibility/source-authority, remaining Iced interfaces, durable operations and
all product/shared-stack/release gates remain required.

The final candidate passes 47 controller and 20 public tests (including three
personal-folder filesystem tests), build, Clippy lints and formatting on 485
unchanged inputs. A platform lookup comparison and exact-byte failing regression
caught normalization of `$HOME/`; its separator is now preserved. The paired final
native checks pass all four sidebar and four narrow-menu targets in both themes,
including actual GIO argv bytes. Two application logs are empty, with four selected
captures inspected. Executable:
`8b949f9d196d14b8cf325d8827594a0377cdcc15db58613edf1c47e8cd0ede06`.
See [the source-bound scoped evidence](../../../.scratch/file-manager/iced-locations-verification.json).

The preceding candidate passed eight Locations/Preferences/browsing/views native
workflows with 106 declared checks. That matrix predates the exact separator
follow-up; it is preserved separately rather than relabeled as current-candidate
execution. Full visual/accessibility/source-authority and remaining product gates
remain required. Native setup now establishes and checks its private HOME before
creating any test paths; the verification record includes an audited, cleaned-up
fixture setup mistake and the isolation fix.
