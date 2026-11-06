# Unified launcher validation — 2026-11-03

The subsequent [unit-conversion follow-up](UNIT-CONVERSIONS.md) records its focused
tests and expanded native acceptance checks separately.

The implementation uses the existing Panel Menu facilities and placement. No
installed configuration, live shell process, or compositor was replaced.

## Passing checks

- Final full shell binary regression suite: 1,284 tests passed, including the
  final selection refinement and all 14 focused launcher regressions.
- Existing integration targets `configuration`, `canopa_provider`, `diagnostics`,
  `menu_cli`, and `provider_protocol`: 20 tests passed.
- Final shell binary/test Clippy with warnings denied, formatting, and whitespace
  checks passed.
- `scripts/verify-shell-launcher.py` passed ten native journeys at scale 1.0 and
  1.25 in private Weston/Niri sessions: calculator clipboard after dismissal,
  Copy path, actual file/folder opening through GIO, application launching,
  executable-extension copying, superseded-query cancellation, provider-failure
  isolation, Escape clear/close, and closing during a query.
- `scripts/test-install-system.py`: ten tests passed. The staged documentation
  includes 55 files with 80 resolving offline links, including the launcher guide
  and Python extension example.
- Arch package template syntax, Python source compilation, and a disposable
  project-directory round trip through the example extension passed.

## Evidence and boundaries

Native screenshots, fixture configuration, compositor/shell logs, and result JSON
are under `.scratch/artifacts/unified-launcher/native-scale-1/` and
`.scratch/artifacts/unified-launcher/native-scale-125/`; the final built binary
was rechecked at `.scratch/artifacts/unified-launcher/native-final-125/`.
Native fixtures use private
XDG directories, session bus, desktop entries, MIME defaults, and temporary files.
Processes are retired by the fixture cleanup. Tests do not use the user's files.

Initial sandboxed D-Bus and systemd unit-verification attempts failed because
local socket operations were denied; the same tests passed in temporary isolated
sessions outside that sandbox. The native fixture initially needed its required
button icon and a screenshot method without an optional Pillow dependency.
Niri's screenshot action also replaced the clipboard; using `grim` resolved that
fixture interference. An initial calculator regression failed on the installed
qalc version's `-n` exit behavior; the corrected adapter accounts for it and isolates personal calculator state.

These are scoped shell/native checks, not full workspace validation, hardware or
multiple-Output certification, screen-reader acceptance, or a large-home-directory
performance benchmark. File-name coverage is eventually refreshed and bounded;
no third-party indexer performance comparison is claimed.

## Window search, ranking, file actions and calculator follow-ups

The [follow-up ledger](NEXT-FEATURES.md) records final validation. Full workspace
checks completed after registering the native scenario and regenerating the Cargo
lock hash in dependency attribution. Final native scale 1 acceptance includes the
Settings input page, and the earlier scale 1.25 run covers the other new journeys.
All requested features use existing Panel Menu facilities; Files owns Quick Look.

## Bundled extensions and searchable actions

See [the extension/action ledger](EXTENSIONS-AND-ACTIONS.md). The final workspace
continuation and workspace/all-target Clippy pass; Shell now has 1,307 passing unit
tests. New native journeys pass at scale 1 and 1.25, including the actual Settings
keyboard save/toggle flow and live launcher routing. Existing Settings display,
input, appearance, lockscreen, wallpaper-picker and unavailable power-backend
regressions pass. Evidence uses extensions-* under the common artifact directory.

## Clipboard history and saved snippets

Shell Clippy passed with warnings denied. The Shell suite passed 1,309 unit tests
and all Shell integration targets under a private D-Bus session. Clipboard frame
checks passed for multiline text, oversized input, invalid UTF-8, sensitive
offers, and empty selections. Acceptance-runner tests, package documentation,
and Arch recipe syntax checks passed.

The first native run stopped in date/time after a concurrent test build replaced
the executable. The launcher fixture now runs from a temporary executable copy.
The full rerun passed at scale 1.25, including external clipboard capture,
sensitive offer exclusion, clearing and stopping history, naming a recent copy,
saving directly while recording is off, and copying a snippet after restart.
Evidence is under `.scratch/artifacts/unified-launcher/clipboard-native-final-125/`
and `clipboard-shell-tests.log`. This change was validated within Shell and its
native launcher fixture.

## Clipboard images and Settings

The final Shell and Settings binaries built successfully. The launcher fixture
passed at scale 1 with the final binaries, and the earlier image implementation
passed at scale 1.25. The fixture uses actual Settings keyboard controls to
enable image capture and set a three-entry limit. It covers thumbnails,
byte-for-byte PNG recopy with the image MIME type, sensitive image exclusion,
image-only opt-out, and saved snippets after Shell restart. See the
`clipboard-images-native-final-1/` and `clipboard-images-native-125/` artifacts.

Clipboard frame checks passed for multiline text, image bytes, sensitive
offers, and the image byte limit. Workspace Clippy passed with warnings denied.
Formatting, dependency attribution, release documentation, and Settings portal
checks passed. The combined workspace command received SIGTERM during the
updater tests. All 21 updater tests passed separately. The systemd unit and
installation checks passed with access to their local sockets after sandboxed
attempts failed to set socket options.

The full workspace/all-target Rust test suite passed under a private D-Bus
session. The remaining workspace gate components also passed, including
configuration checks and vendor tests. Logs use the `clipboard-images-*`
prefix under `.scratch/artifacts/unified-launcher/`.

## Emoji and symbols

The Shell build, all-target Clippy with warnings denied, formatting, and release
documentation packaging passed. Regenerating the catalogs from the pinned
Unicode and CLDR inputs produced identical files. The initial Shell test run
failed in the existing Qalculate conversion test on `sin(pi/2)`. The full Shell
suite passed with four test threads, including 1,311 binary unit tests and all
integration targets. Logs use the `characters-shell-*` prefix in the launcher
artifact directory.

The first private Wayland run exposed overlapping labels for long emoji names.
The final run passed all 44 checks at scale 1.25. The fixture verified the
full-width character labels and exact clipboard output for joined emoji,
variation selectors, and the em dash. See `characters-native-final-125/`.
