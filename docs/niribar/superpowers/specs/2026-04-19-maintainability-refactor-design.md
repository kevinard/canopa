# Maintainability Refactor — Design

**Date:** 2026-04-19
**Goal:** Eliminate remaining code duplication, shrink god-modules, and introduce compiler-enforced isolation between runtime concerns — without changing observable behavior, adding dependencies, or introducing macros beyond what already exists.

## Context

A prior refactor (see `2026-04-19-code-quality-refactor-summary.md`) extracted menu/tray reducers, added `ModuleKind` and `ModuleStore`, shared dynamic-module cleanup, centralized command execution, bounded tray pixmap/DBusMenu data, isolated the PulseAudio `unsafe` block, and reduced KDL parser duplication. Despite that work, several large files still mix concerns (`crates/niribar/src/app.rs` ≈3723 lines, `src/config/parse.rs` ≈1665, `src/dbus/sni_host.rs` ≈1006), and a thorough survey surfaced ~450–550 lines of remaining duplication.

This spec covers the follow-on restructure.

## Scope and constraints

- **Deep restructure** is in scope: duplication extractions, file splits, and an actor-style decomposition of `App`.
- **No macros** beyond trivial ones. Prefer trait defaults and helper functions.
- **No new runtime dependencies.**
- **Behavior parity, implementation freedom.** Externally observable behavior (config acceptance/rejection, widget output, D-Bus interactions, CLI output) is preserved. Internal test names, function signatures, and error-message wording may change where it improves clarity; any test asserting on specific wording is updated deliberately and called out in its PR.
- **iced's single-threaded update loop is preserved.** The actor split restructures ownership within that loop; it does not introduce async channels, threads, or locks.
- **Verification gate (every phase):** `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo build` must all pass. No `--release` gate; release builds belong to release work.

## Architecture

### Target ownership

```rust
pub struct App {
    config: AppConfig,
    shared: SharedContext,
    surfaces: SurfaceManager,   // all windows, including popups
    menu:     MenuSystem,       // generic, module-agnostic menu component
    modules:  ModuleRuntime,
    niri:     NiriState,
    tray:     TrayState,        // SNI/DBusMenu data source only
}
```

Each sub-struct owns its state and exposes a narrow API: `update`, `view` where applicable, `subscription` where applicable. `App::update` is a shallow router that forwards each domain message to its owner.

### Sub-struct responsibilities

- **`SharedContext`** — read-only during `update`: D-Bus connections (system + session), theme, clock source. Populated at startup; never mutated by any update handler. Passed by `&` to sub-struct updates that need it.
- **`SurfaceManager`** — every window. Bar surfaces and popup surfaces alike. Owns per-window state (`expand`, `hover`, sizes), animation ticks, and layer-shell operations. Exposes a popup lifecycle API (`open_popup`, `close_popup`, `close_popups_for`) so other sub-structs never create windows themselves.
- **`MenuSystem`** — generic menu component. Owns menu state (open/closed set, focused item, keyboard navigation) and rendering, but **not** the popup windows it renders into. Any module can supply a `MenuModel` and ask the menu system to display it; the menu system asks `SurfaceManager` for the popup.
- **`ModuleRuntime`** — `ModuleStore` plus per-module state, dynamic-module bookkeeping (tray icons, workspace items), and module subscription wiring.
- **`NiriState`** — workspace list, focused window, niri-ipc subscription, workspace rebuild logic.
- **`TrayState`** — SNI/DBusMenu data source only. Subscription handling, tray item list, pixmap validation, and translating DBusMenu trees into `MenuModel`. Opens menus by asking `MenuSystem`; owns no windows.

### Message routing

```rust
enum Message {
    Surface(SurfaceEvent),
    Menu(MenuEvent),
    Modules(ModuleEvent),
    Niri(NiriEvent),
    Tray(TrayEvent),
    // cross-cutting: Tick, ConfigReloaded, etc.
}
```

`App::update` matches on the outer variant and forwards to the owning sub-struct's `update`. Sub-structs return `Task<Message>`; cross-cutting effects are expressed as tasks that re-enter the router. **No sub-struct calls another sub-struct directly.**

### Cross-cutting example — tray icon click

1. `TrayEvent::IconClicked(id)` → `TrayState::update` translates the tray's menu tree into a `MenuModel`, returns `Task::done(Message::Menu(MenuEvent::Open { model, anchor, parent }))`.
2. `MenuSystem::update` records the menu and returns `Task::done(Message::Surface(SurfaceEvent::OpenPopup { … }))`.
3. `SurfaceManager::update` creates the popup window.

Three sub-structs, zero direct coupling, all effects expressed as tasks. The compiler enforces that none of them can reach into another's state.

## Work breakdown — risk-first sequencing

Six phases. Each phase is independently shippable (tests green, clippy clean, debug build OK). Highest-risk work (actor split) lands last, on top of already-tested extractions.

### Phase 1 — Pure extractions

- Generic D-Bus subscription stream builder in `crates/niribar/src/dbus/subscription.rs`, encapsulating the Init/Running/Done `stream::unfold` state machine repeated across `upower`, `networkmanager`, `wireplumber`. Callers supply only the task body.
- Generic multi-signal subscription helper (plain function, slice of named streams + reader closure) to collapse the `tokio::select!` loops in `upower` and `networkmanager`.
- `find_tray_slots()` helper on `App` (later `ModuleStore`) to replace the duplicated slot-finding block in `app/tray.rs`.
- Conditional D-Bus subscription helper in `app/subscriptions.rs`: `fn dbus_subscription(has_module, conn, build)`.
- Surface→bar lookup helper: `bar_for_window(Id) -> Option<&BarConfig>`.
- Error-path helper for config parsing: `fn cfg_err(idx, module, field) -> String`. Mechanical migration of all existing `anyhow::bail!` / `anyhow!` sites in `config/parse.rs`; wording preserved.

### Phase 2 — Module trait defaults

Extend the `Module` trait with default `view_collapsed`/`view_expanded`/`view_horizontal` implementations keyed off a small `IconLabel` helper trait (`fn icon`, `fn label`, `fn color`). Icon-label modules (battery, clock, network, volume, button) inherit all three views. Modules whose layout is genuinely custom (focused-window, tray, workspaces) keep their own implementations. No macros.

### Phase 3 — `config/parse.rs` split

```
config/parse/
  mod.rs
  kdl.rs
  error.rs
  bars.rs
  animation.rs
  modules/
    mod.rs        // dispatch: ModuleKind → parse_fn
    battery.rs
    clock.rs
    network.rs
    volume.rs
    script.rs
    button.rs
    workspaces.rs
    focused_window.rs
    tray.rs
  tests.rs        // cross-cutting tests
```

The stringly if-chain at `config/parse.rs:279` is replaced with a typed `match kind: ModuleKind { … }` dispatch. KDL extraction helpers move to `config/parse/kdl.rs`. Per-module parser tests move next to their parsers; multi-section tests stay in `config/parse/tests.rs`.

### Phase 4 — `dbus/sni_host.rs` split

```
dbus/sni/
  mod.rs
  host.rs         // SNI bus ownership, item lifecycle
  subscription.rs // uses dbus/subscription.rs from Phase 1
  pixmap.rs       // validation, bounds, ARGB→RGBA conversion
  menu.rs         // DBusMenu fetch, node bounds, label caps
  proxy.rs        // zbus proxy trait definitions
```

All untrusted-input validation concentrates in `pixmap.rs` and `menu.rs`. Both expose only validated types to the rest of the module; the host/subscription layers cannot bypass bounds. This becomes the single auditable security surface.

### Phase 5 — App actor split

Introduce the sub-structs one at a time; after each extraction, `App::update` is partly a router, partly still inline; all tests pass.

1. Extract `SharedContext` (regroup fields; no behavior change).
2. Extract `SurfaceManager`, including popup-window lifecycle lifted out of the current menu/tray modules.
3. Extract `MenuSystem` as the generic menu component; migrate current tray-menu code to call it.
4. Extract `NiriState`.
5. Extract `TrayState` (now thin — data source only).
6. Extract `ModuleRuntime` last; its boundary is clearest once the other four exist.

When step 6 finishes, `App::update` is a pure router.

Tests migrate per the mixed strategy: tests that exercise one sub-struct move to `crates/niribar/src/app/<name>/tests.rs` or alongside; cross-cutting tests collect in `src/app/tests.rs`.

### Phase 6 — Cleanup and hardening

- Remove dead code that the split exposes.
- Tighten `pub` visibility to the minimum each sub-struct needs.
- Add any missing input-bounds unit tests for the pixmap/menu paths.

## Testing and verification

### Verification gate — every phase

- `cargo test`
- `cargo clippy --all-targets -- -D warnings`
- `cargo build`

### Manual Wayland smoke test — Phase 5

Required after the surface, menu, tray, and module extractions in Phase 5. Covers:

- Bar rendering on each configured output; hover/click expand toggle.
- Tray icon click opens a popup menu; keyboard navigation; click-outside closes.
- Tray icons appear/disappear as apps start/stop; right-click menu items execute.
- Workspace switching updates focus highlight; focused-window title updates.
- Battery, network, volume values update live; icons reflect state.

### Behavior-parity checks

- Config acceptance/rejection set unchanged: every KDL input accepted before is still accepted; every one rejected before is still rejected.
- Widget output for each module is equivalent on identical state.
- D-Bus object paths, interfaces, and signal subscriptions are unchanged.
- Error-message wording preserved unless deliberately clarified; any changed test assertion is called out in its PR.

### Security checks — Phase 4

- Grep verifies that no pixmap byte-length or menu-size check exists outside `dbus/sni/pixmap.rs` and `dbus/sni/menu.rs`.
- Unit tests enforce the bounds: oversize pixmap rejected, wrong `w*h*4` length rejected, excess menu items truncated, label length capped.
- Command-execution boundary (`crates/niribar/src/command.rs`) remains the single shell-out path.

### Phase 4 audit

Audit ran the requested boundary checks with `rtk grep -Rn -E "(width|height)\s*\*\s*(height|width)\s*\*\s*4" src/ | rtk grep -v dbus/sni/pixmap.rs` and `rtk grep -Rn "max_menu_items\|max_label_len\|MAX_PIXMAP" src/ | rtk grep -v "dbus/sni/\(pixmap\|menu\).rs"`. Both source-tree boundary checks returned no matches outside the allowed SNI boundary files. Any `rtk grep -v` output that mentioned spec/planning docs was a wrapper artifact, not part of the audit result.

## Non-goals

- No new runtime dependencies.
- No macros beyond the trivial ones already in the tree.
- No async channels, threads, or locks beyond what already exists. iced's single-threaded update loop stays intact.
- No change to the config file format or user-facing CLI.
- No introduction of structured / argv commands in config; deferred.
- No broader rework of module discovery or dynamic module plugins.
- No iced widget-tree rewrite; the view layer is touched only where duplication collapses (Phase 2).
- No performance regression tolerated; no explicit performance target beyond "equal or better."

## Expected outcome

- `crates/niribar/src/app.rs` reduced from ~3723 lines to a thin router, estimated under 400 lines.
- `src/config/parse.rs` replaced by a directory; each file under ~300 lines.
- `src/dbus/sni_host.rs` replaced by a directory; each file under ~400 lines.
- ~450–550 lines of duplication eliminated in Phases 1 and 2.
- Compiler-enforced isolation between surfaces, menu, modules, niri, and tray — a class of cross-domain bugs made unrepresentable.
