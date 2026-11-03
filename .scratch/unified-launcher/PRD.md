Status: ready-for-human

# PRD: Unified Shell Launcher

Canopa Shell provides one keyboard-first launcher for applications, local file
and folder names, arithmetic, and features supplied by executable extensions.
It is a `launcher` Panel Menu Source using the existing panel facilities.

The user confirmed file/folder names first on 2026-11-03 and explicitly required
the existing application launcher's panel placement. Document contents remain out
of scope. [ADR 0023](../../docs/shell/adr/0023-unified-launcher-providers.md) records
the implementation decisions; the [launcher guide](../../docs/shell/launcher.md)
defines supported configuration and the extension contract.

## User experience

The bundled Applications menu opts into the unified source, retaining the spine
action and `canopa-shell --open-menu Applications` / `--toggle-menu Applications`
entry points. User configurations opt in with `source "launcher"`; the existing
`installed-apps` source remains available. Panel placement, geometry, theme,
focus, scrolling, dismissal, and Foreground replacement keep their existing owners.

| Interaction | Behavior |
| --- | --- |
| `firefox` | Search applications and file/folder names. |
| `apps firefox` | Search only applications. |
| `files budget` | Search file/folder names in configured local roots. |
| `12 * (8 + 3)` or `= 12 * (8 + 3)` | Show `132`; activation copies the value. |
| `10 cm to m`, `32 F to C`, or `1 GiB to MiB` | Convert compatible units; activation copies the value and unit. |
| A configured extension keyword followed by text | Route that query to the extension. |
| Up/Down, Enter | Select and activate without leaving the query field. |
| Ctrl+Enter | Run the selected result's second action. |
| Ctrl+1 through Ctrl+4 | Run the corresponding named action. |
| Escape | Clear a nonempty query, then close. |

Results show an icon, label, and application/file/provider context. Additional
named actions appear beneath the list. Files and folders use the default desktop
handler; text paths offer Copy path. Non-UTF-8 targets preserve exact bytes for
opening and omit Copy path. Blank queries show applications in stable order.

## Implementation boundaries

- Each Launcher Session has a unique identity, query generation, provider batches,
  and displayed revision. Late results are checked even when cancellation fails.
- Same-query updates retain selection by provider/result identity; removed results
  choose a deterministic neighbor. New queries start new selection.
- Pointer activation captures the displayed result/action revision. Actions cannot
  execute twice while their completion is pending. Success closes only the same
  session/query; failures keep it open.
- Application discovery, desktop-ID precedence, metadata, visibility filtering,
  fuzzy matching, and session-owned launch remain in their existing boundaries.
- A native memory-only filename cache is built off the UI thread and reused across
  opens of the same configuration. Scans refresh every minute while open, with
  path-count, depth, elapsed-time, exclusion, and cancellation limits.
- Qalculate supplies arithmetic and unit conversions. A complete numeric grammar,
  a supported unit catalog with dimension checks, output validation,
  process limits, and isolated calculator state keep queries side-effect-free.
- Providers return bounded ordered groups. The host never compares arbitrary
  provider scores. Recognized calculations, applications, files, then extensions
  have deterministic group order and up to 20 results per source.
- Configured executable extensions receive one debounced JSONL query, return
  results with host-executed Open/Copy/Launch actions, then exit. Cancellation
  retires their process group. Whole-panel providers retain their separate protocols.
- Changed configuration and logical dismissal retire interaction. Retiring visual
  content does not keep provider subscriptions alive or accept late activations.

## Delivery slices

1. [Session and provider contract](issues/01-launcher-session.md).
2. [Existing panel integration with applications](issues/02-launcher-panel.md).
3. [File and folder name provider](issues/03-file-name-search.md).
4. [Calculator provider](issues/04-calculator.md).
5. [Executable extensions](issues/05-extensions.md).

Implementation and focused validation are complete; see [evidence](VALIDATION.md).
This status requests human acceptance review, not a claim of a full workspace,
hardware, screen-reader, or multiple-Output acceptance matrix.

## Deferred scope

Document contents, open-window results, previews, usage-history ranking, clipboard
history, web/currency data, arbitrary extension widgets, a marketplace, persistent
extension sessions, and activation callbacks remain later features. A shared
indexing service requires a root integration ADR. The current cache is eventually
refreshed; it does not promise immediate discovery of every filesystem change.

## Comments

- 2026-11-03: User requested the unified launcher and confirmed filename/folder-name
  scope. The implementation request explicitly retained existing panel placement.
- 2026-11-03: A native path cache and Qalculate were selected for this slice.
  Extensions use a bounded per-query executable contract. These replace the
  initial draft's centered placement and persistent extension-session proposals.
- 2026-11-03: Added unit conversions at the user's request, including absolute
  temperatures, information units, common physical units, powers, and ratios.

## Follow-up implementation

The user also requested unit conversions and the recommended window search,
selection ranking, richer file actions, desktop shortcuts and math functions.
See [next features](NEXT-FEATURES.md) and [unit conversions](UNIT-CONVERSIONS.md).
The existing Panel Menu remains the launcher owner. Files owns Quick Look through
its activation/presentation boundary; Open with remains in the launcher. Root ADR
0037 and Files ADR 0158 document the cross-application contract.

Bundled web/date extensions, pins/aliases, searchable actions and Settings extension
management are implemented and verified. See [the extension/action ledger](EXTENSIONS-AND-ACTIONS.md).
