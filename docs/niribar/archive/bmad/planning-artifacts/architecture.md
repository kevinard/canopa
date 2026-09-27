---
stepsCompleted: [1, 2, 3, 4, 5, 6, 7, 8]
status: 'complete'
completedAt: '2026-03-28'
lastStep: 8
inputDocuments: ['_bmad-output/planning-artifacts/prd.md']
workflowType: 'architecture'
project_name: 'niribar'
user_name: 'Kevin'
date: '2026-03-28'
---

# Architecture Decision Document

_This document builds collaboratively through step-by-step discovery. Sections are appended as we work through each architectural decision together._

## Project Context Analysis

### Requirements Overview

**Functional Requirements:**

43 FRs across 7 capability areas: bar rendering and multi-output layout (FR1-5), expand/collapse interaction (FR6-10), trait-based module system (FR11-16), Niri compositor integration via IPC (FR17-21), system service modules via D-Bus (FR22-28), keyboard and input handling (FR29-31), KDL configuration system (FR32-38), and packaging/distribution artifacts (FR39-43).

Architecturally, the requirements decompose into four primary subsystems: the rendering surface layer, the async data pipeline, the module framework, and the configuration system.

**Non-Functional Requirements:**

- Performance (NFR1-6): Animation quality at GNOME Shell standard, resource parity with Waybar, one-frame Niri IPC latency, async-only external data sources — these collectively mandate a non-blocking async architecture with tight render loop isolation
- Security (NFR7-8): Local IPC only, no network, no privilege escalation — no security infrastructure required beyond what the OS provides
- Accessibility (NFR9-11): Full keyboard operability, WCAG AA contrast, HiDPI/scale factor compliance — these affect Iced surface configuration and input event handling

**Scale & Complexity:**

- Primary domain: Native Linux desktop application, systems-level Rust
- Complexity level: Medium-high — the Iced/wlr-layer-shell integration and async multi-source data pipeline are genuinely novel territory with limited prior art
- Estimated primary architectural components: 6 (surface manager, render loop, module registry, async data bus, config loader, Niri IPC client)

### Technical Constraints & Dependencies

- **Rust** — language constraint; async runtime will be Tokio (Iced's scheduler dependency)
- **Iced** — UI toolkit; pre-1.0, API-unstable; provides the Model/View/Update architecture and GPU-accelerated rendering via wgpu
- **wlr-layer-shell** — Wayland protocol for surface placement; foundational constraint for bar rendering and positioning
- **zbus** — async-native Rust D-Bus library; chosen for D-Bus integration to avoid blocking the render loop
- **KDL** — config file format; chosen for consistency with Niri's own config format
- **Niri IPC** — Unix socket event stream; compositor-specific protocol for workspace and focus state

### Cross-Cutting Concerns Identified

- **Async runtime unification**: Tokio must be the single async runtime; Iced's subscription model, zbus async connections, and Niri IPC socket reads must all coexist on the same Tokio executor without contention
- **Module isolation and error resilience**: Each module is an independent async data source; a failing D-Bus connection (e.g., UPower unavailable on a desktop) must not crash or stall the bar — modules must degrade gracefully
- **Multi-output state**: Each physical output requires an independent Iced surface with its own expand/collapse state, but modules share underlying data sources — the data layer must be decoupled from the per-surface render state
- **Input region management**: The expand/collapse hover trigger requires precise Wayland input region configuration; keyboard focus on a layer-shell surface has compositor-specific behaviour that must be handled correctly for Niri

## Starter Template Evaluation

### Primary Technology Domain

Native Rust binary application targeting Linux/Wayland. No web framework starter applies — project is initialized with `cargo new` and a defined dependency set.

### Starter Options Considered

**Option A: `iced_layershell` (community crate)**
- `iced_layershell` 0.13.7 provides Iced + wlr-layer-shell integration as a crates.io dependency
- Clean, packageable dependency tree — no fork required
- Current version tracks Iced 0.13.x; Iced 0.14.0 compatibility to be confirmed

**Option B: pop-os/iced fork**
- Used by bar-rs (reference project: github.com/faervan/bar-rs) and COSMIC Desktop
- Git dependency (not crates.io) — adds packaging complexity
- Closely aligned with COSMIC's Iced branch

**Reference Implementation:** `bar-rs` (github.com/faervan/bar-rs) — a Rust status bar for Hyprland, Niri, and Wayfire built with iced-rs. Closest known prior art to niribar; worth studying architecture and module patterns.

### Selected Approach: `cargo new` + `iced_layershell` dependency set

**Rationale:** `iced_layershell` provides the wlr-layer-shell + Iced integration as a clean crates.io dependency. This preserves a standard `cargo build` pipeline (FR39) and avoids git fork dependencies that complicate packaging (FR41 Nix flake, AUR). Version alignment with Iced 0.14 should be confirmed before implementation begins; if iced_layershell has not yet updated, pin Iced to 0.13.x.

**Initialization Command:**
```bash
cargo new niribar --bin
```

**Core Dependency Set (`Cargo.toml`):**

| Crate | Version | Purpose |
|---|---|---|
| `iced_layershell` | 0.13.7 | Iced + wlr-layer-shell surface integration |
| `iced` | 0.13.x (pin to iced_layershell compatibility) | UI toolkit, rendering, MVU architecture |
| `zbus` | 5.12.0 | Async D-Bus (UPower, NetworkManager, PipeWire) |
| `kdl` | 6.5.0 | KDL config file parsing |
| `tokio` | 1.x (feature: full) | Async runtime |

**Architectural Decisions Established by This Foundation:**

- **Language & Runtime:** Rust stable, Tokio async runtime
- **UI Architecture:** Iced Model/View/Update (Elm-like); GPU-accelerated via wgpu
- **Wayland Surface:** wlr-layer-shell via iced_layershell; no X11/XWayland
- **D-Bus:** zbus async API; D-Bus connections managed as Iced subscriptions
- **Config Format:** KDL via `kdl` crate
- **Build:** Standard `cargo build`; no build scripts, no non-Rust tooling
- **Testing:** `cargo test` for unit tests; integration tests for D-Bus/IPC require environment mocking layer

**Note:** Project initialization (`cargo new niribar --bin`, Cargo.toml setup, workspace structure) should be the first implementation story.

## Core Architectural Decisions

### Decision Priority Analysis

**Critical Decisions (Block Implementation):**
- Application model: single Iced app, multiple layer-shell surfaces
- Module system: async stream producers feeding Iced subscriptions
- Niri IPC: `niri-ipc` sub-crate with event stream
- Config location: XDG convention

**Important Decisions (Shape Architecture):**
- Module registration: static compiled-in registry for MVP
- Project structure: single crate
- Error handling: per-module graceful degradation

**Deferred Decisions (Post-MVP):**
- Dynamic plugin loading (`dlopen`) — Phase 2 plugin API
- Cargo workspace split — revisit if codebase warrants it
- `aarch64` release target — add when CI infrastructure allows

### Application & Surface Architecture

**Decision:** Single Iced application instance managing multiple layer-shell surfaces.

One `main()` entry point, one Tokio runtime, one set of module data subscriptions. `iced_layershell` manages multiple Wayland surfaces from the same application context via its multi-window API. Per-output render state (expand/collapse width, layout zone content) is tracked per surface in the application's `Model`; module data (battery level, workspace list, etc.) is shared across all surfaces.

**Rationale:** Minimises resource usage (one D-Bus connection per service, one Niri IPC socket) and avoids cross-process coordination. Aligns with NFR2-3 (resource parity with Waybar).

**Cascading implication:** The `Model` must cleanly separate shared data (module state) from per-surface state (expand/collapse, focus). The `Message` enum must route surface-specific events by output ID.

### Module System Design

**Decision:** Modules are async stream producers; module registry is static (compiled-in) for MVP.

Each module implements a trait that exposes:
1. An async `Subscription` (wrapping an Iced `subscription::channel` or a `Stream`) that emits typed `ModuleMessage` variants into the application's update loop
2. A `view()` function returning Iced `Element`s for both collapsed and expanded states
3. An optional `on_interaction()` handler for click/scroll/keyboard events

Modules are registered in a static registry — the config parser maps a string identifier (e.g., `"clock"`, `"workspaces"`) to a module constructor. Unknown module names produce a config error (FR37).

**Rationale:** Event/subscription-driven model satisfies NFR6 (no render loop blocking). Static registration is sufficient for MVP and keeps the architecture simple; dynamic plugin loading is explicitly Phase 2.

**Cascading implication:** Each module's subscription must handle its data source being unavailable (e.g., UPower not running) and emit a degraded state rather than panic. Error states are a first-class `ModuleMessage` variant.

### Niri IPC Integration

**Decision:** Use the `niri-ipc` sub-crate (docs: niri-wm.github.io/niri/niri_ipc) via its event stream API.

The Niri IPC client connects to the Niri socket and subscribes to the event stream. Events are wrapped in an Iced `Subscription` that emits `NiriEvent` messages into the application update loop. The workspace module and focused window module consume these events.

**Rationale:** Official sub-crate from the Niri project — type-safe, maintained alongside the compositor. Event stream (push-based) satisfies NFR4 (one-frame latency for IPC state changes).

**Cascading implication:** Niri IPC must be treated as optional infrastructure — the subscription must handle socket unavailability (running on a non-Niri compositor) and disable Niri-specific modules gracefully (FR21).

### Configuration Architecture

**Decision:** `$XDG_CONFIG_HOME/niribar/config.kdl` (fallback: `~/.config/niribar/config.kdl`).

Config is parsed at startup using the `kdl` 6.5.0 crate into a typed `Config` struct. Validation errors produce actionable messages to stderr and exit (FR37). A bundled default config file is used when no user config is present (FR33) — this default is also the installed example config (FR42). Per-output config is expressed as named output blocks within the KDL document.

**Rationale:** XDG convention is standard for Linux desktop apps. KDL matches Niri's own config format (user familiarity). Fail-fast validation on startup prevents silent misconfiguration.

**Cascading implication:** The config struct must be fully parsed before any surfaces are created — no lazy config evaluation. Output names in config must be matched against Wayland output names at runtime (xdg-output protocol).

### Project Structure

**Decision:** Single Rust crate for MVP.

```
niribar/
├── Cargo.toml
├── src/
│   ├── main.rs          # Application entry point, Iced app setup
│   ├── config/          # KDL parsing, Config types
│   ├── modules/         # Module trait + all built-in modules
│   │   ├── mod.rs       # Module trait definition, registry
│   │   ├── workspaces.rs
│   │   ├── clock.rs
│   │   ├── battery.rs
│   │   └── ...
│   ├── niri/            # niri-ipc subscription wrapper
│   ├── dbus/            # zbus connection management, service wrappers
│   └── surface/         # Per-output surface state, expand/collapse logic
├── config/
│   └── default.kdl      # Bundled default config
└── assets/              # Icons, fonts (if any)
```

**Rationale:** Single crate avoids workspace overhead for MVP. Clear directory boundaries make a future workspace split mechanical if needed.

### CI/CD & Release Pipeline

**Decision:** GitHub Actions with standard Rust checks + tag-triggered binary releases.

- **PR checks:** `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`
- **Releases:** Tag-triggered workflow building `x86_64-unknown-linux-gnu` binary; `aarch64` added when infra allows
- **Artifacts:** Compiled binary + `crates/niribar/config/default.kdl` + `packaging/systemd/niribar.service`
- **Nix flake:** Maintained in-repo; CI validates `nix build` on pushes to main

### Decision Impact Analysis

**Implementation Sequence:**
1. Project scaffold + Cargo.toml with pinned dependencies
2. iced_layershell surface rendering (single output, static bar)
3. Expand/collapse animation and state
4. Config parsing (KDL → typed Config struct)
5. Module trait + subscription infrastructure
6. Niri IPC subscription wrapper
7. zbus D-Bus connection management
8. Built-in modules (one by one, starting with clock/workspaces)
9. Multi-output support
10. Keyboard input handling
11. Packaging artifacts (systemd unit, Nix flake, default config)

**Cross-Component Dependencies:**
- Surface manager depends on config (per-output layout) and module registry (which modules to render)
- All modules depend on the async subscription infrastructure
- Niri-specific modules depend on the Niri IPC wrapper
- D-Bus modules depend on the zbus connection manager
- Config system must be complete before any modules or surfaces initialize

## Implementation Patterns & Consistency Rules

### Potential Conflict Points Identified

**7 areas** where AI agents could make different incompatible choices without explicit rules.

### Naming Patterns

**Rust Standard Conventions (non-negotiable):**
- Functions, variables, fields, module file names: `snake_case`
- Types, structs, enums, traits: `PascalCase`
- Constants, statics: `SCREAMING_SNAKE_CASE`
- Crate name in code: `niribar` (matches binary name)

**Module System Naming:**
- Module trait: `Module` (no prefix — the `modules::` path provides context)
- Per-module state type: `{Name}State` (e.g., `BatteryState`, `ClockState`, `WorkspaceState`)
- Per-module event type: `{Name}Event` (e.g., `BatteryEvent`, `ClockEvent`, `WorkspaceEvent`)
- Module identifier in registry: string key matching config name, e.g., `"battery"`, `"clock"`, `"workspaces"`

**Iced Message Enum:**
- Top-level enum is named `Message` (Iced convention)
- Module-originated data: `Message::Module(ModuleId, ModuleEvent)` — all modules route through this variant
- Surface UI events: `Message::Surface(SurfaceId, SurfaceEvent)` — per-output events only
- Niri IPC events: `Message::Niri(NiriEvent)`
- Config events: `Message::ConfigLoaded(Config)` / `Message::ConfigError(String)`

**D-Bus Naming:**
- Proxy structs: `{Service}Proxy` (e.g., `UPowerProxy`, `NetworkManagerProxy`)
- One file per D-Bus service: `crates/niribar/src/dbus/upower.rs`, `crates/niribar/src/dbus/networkmanager.rs`, `crates/niribar/src/dbus/wireplumber.rs`

**Config Types:**
- Top-level parsed config: `Config` (in `src/config/mod.rs`)
- Per-section structs: `BarConfig`, `ModuleConfig`, `OutputConfig`, `AnimationConfig`
- No `Niribar` prefix on internal types — the crate name provides context

### Structure Patterns

**Test Organization:**
- All tests co-located: `#[cfg(test)] mod tests { ... }` at the bottom of each source file
- No separate `tests/` directory for unit tests
- Integration tests requiring live D-Bus: deferred, not in MVP

**Module File Organization:**
- One file per built-in module in `crates/niribar/src/modules/`
- Each module file is self-contained: defines its `{Name}State`, `{Name}Event`, and `impl Module`
- `src/modules/mod.rs` contains the `Module` trait definition and the static registry `HashMap`

**Subscription ID Pattern:**
- Every subscription uses `std::any::TypeId::of::<{Name}Subscription>()` for its unique ID
- No magic strings for subscription IDs

### State Management Patterns

**The critical boundary — all agents MUST respect this:**

```rust
struct AppState {
    shared: SharedState,                          // module data, shared across all outputs
    surfaces: HashMap<SurfaceId, SurfaceState>,   // per-output render state
}

struct SharedState {
    modules: HashMap<ModuleId, ModuleState>,
    niri: NiriState,
    config: Config,
}

struct SurfaceState {
    expand: ExpandState,                  // collapsed / animating / expanded
    focused_module: Option<ModuleId>,
    output_name: String,
}
```

**Rule:** Module data (battery %, workspace list, volume level) lives in `SharedState`. Expand/collapse width, keyboard focus, animation progress live in `SurfaceState`. Never cross this boundary.

### Communication Patterns

**Module Subscription Contract — every `Module` implementation must:**
1. Emit its first event within 1 second of subscription start, or emit `{Name}Event::Unavailable`
2. Never panic inside a subscription — wrap all fallible operations in `match`/`if let`
3. Treat D-Bus service disappearance as `{Name}Event::Unavailable`, not a fatal error
4. Support re-connection: if a D-Bus service restarts, the subscription must recover

**Message Routing Convention:**
```rust
// CORRECT — surface event routed by ID
Message::Surface(surface_id, SurfaceEvent::HoverEnter)

// CORRECT — module data broadcast to all surfaces
Message::Module(ModuleId::Battery, BatteryEvent::LevelChanged(85))

// WRONG — do not embed surface/output logic in module messages
Message::BatteryForOutput(output_name, 85)
```

### Process Patterns

**Module Initial State:**
- All modules start as `ModuleState::Loading` until the first event arrives
- `Loading` renders as an empty placeholder — no spinner, no text (the bar is persistent)
- `Unavailable` renders as nothing (module is hidden) — no error indicator in the bar UI

**Config Error Handling:**
- Invalid config → print actionable message to `stderr` → exit with code 1
- Message format: `error: <field path>: <what was wrong>. Expected: <what was expected>.`
- No partial config loading — config is fully valid or the binary exits

**Expand/Collapse State Machine:**
```
Collapsed → Animating(Expanding) → Expanded
Expanded  → Animating(Collapsing) → Collapsed
```
Animation progress is an `f32` in `[0.0, 1.0]` stored in `SurfaceState`, advanced by `Message::Tick`. No additional intermediate states are valid.

**Error Type Selection:**
- Config parsing and startup errors: `anyhow::Result<T>` — use anyhow for user-facing context chains
- Module subscriptions: never return `Result`; emit error as `{Name}Event::Error(String)` or `{Name}Event::Unavailable`
- Niri IPC unavailable: `NiriEvent::Unavailable` — not an error, a valid operational state

### Enforcement Guidelines

**All agents MUST:**
- Respect the `SharedState` / `SurfaceState` boundary without exception
- Emit `Unavailable` rather than propagating errors from module subscriptions
- Use `anyhow` for config/startup errors; typed event variants for runtime module state
- Name `Message` variants following the patterns above before adding new variants
- Co-locate tests with source files using `#[cfg(test)] mod tests`

**Pattern Verification:**
- `cargo clippy -- -D warnings` catches most naming violations
- Code review: verify new modules emit `Unavailable` on service absence, verify no module data in `SurfaceState`, verify `Message` variant naming convention

## Project Structure & Boundaries

### FR Categories to Architectural Components

| FR Category | Primary Location |
|---|---|
| FR1–5 Bar rendering/layout | `crates/niribar/src/surface/` + `crates/niribar/src/main.rs` |
| FR6–10 Expand/collapse | `src/surface/expand.rs` |
| FR11–16 Module system | `crates/niribar/src/modules/` |
| FR17–21 Niri integration | `crates/niribar/src/niri/` |
| FR22–28 System service modules | `crates/niribar/src/modules/{battery,network,volume,clock,script,button}.rs` |
| FR29–31 Keyboard/input | `crates/niribar/src/surface/input.rs` |
| FR32–38 Config system | `crates/niribar/src/config/` |
| FR39–43 Packaging/distribution | `contrib/`, `config/`, `.github/` |

### Complete Project Directory Structure

```
niribar/
├── Cargo.toml
├── Cargo.lock
├── .gitignore
├── .github/
│   └── workflows/
│       ├── ci.yml              # PR checks: fmt, clippy, test
│       └── release.yml         # Tag-triggered binary release
├── config/
│   └── default.kdl             # Bundled default config (FR33, FR42)
├── contrib/
│   └── systemd/
│       └── niribar.service     # systemd user service unit (FR40)
├── assets/                     # Icons, fonts (if needed)
└── src/
    ├── main.rs                 # Entry point: config load, Iced app setup, surface init
    ├── app.rs                  # Iced Application impl: Model, update(), view(), subscription()
    ├── config/
    │   ├── mod.rs              # Config, BarConfig, OutputConfig, ModuleConfig, AnimationConfig types
    │   ├── parse.rs            # KDL → Config parsing (anyhow errors)
    │   └── defaults.rs         # Default config values, bundled default.kdl embed
    ├── modules/
    │   ├── mod.rs              # Module trait, ModuleId enum, ModuleState enum, static registry
    │   ├── battery.rs          # BatteryState, BatteryEvent, impl Module (UPower D-Bus)
    │   ├── clock.rs            # ClockState, ClockEvent, impl Module (tokio interval)
    │   ├── network.rs          # NetworkState, NetworkEvent, impl Module (NetworkManager D-Bus)
    │   ├── volume.rs           # VolumeState, VolumeEvent, impl Module (WirePlumber D-Bus)
    │   ├── workspaces.rs       # WorkspaceState, WorkspaceEvent, impl Module (Niri IPC)
    │   ├── focused_window.rs   # FocusedWindowState, FocusedWindowEvent, impl Module (Niri IPC)
    │   ├── script.rs           # ScriptState, ScriptEvent, impl Module (tokio::process)
    │   └── button.rs           # ButtonState, ButtonEvent, impl Module (no data source)
    ├── niri/
    │   ├── mod.rs              # NiriState, NiriEvent types
    │   └── subscription.rs     # Iced Subscription wrapping niri-ipc event stream
    ├── dbus/
    │   ├── mod.rs              # zbus connection manager: session bus + system bus handles
    │   ├── upower.rs           # UPowerProxy, battery D-Bus subscription
    │   ├── networkmanager.rs   # NetworkManagerProxy, network D-Bus subscription (system bus)
    │   └── wireplumber.rs      # WirePlumberProxy, volume D-Bus subscription
    └── surface/
        ├── mod.rs              # SurfaceState, SurfaceId, SurfaceEvent types
        ├── expand.rs           # ExpandState machine, animation tick logic
        ├── input.rs            # Keyboard focus, input region management
        └── layout.rs           # Zone layout (top/center/bottom), module rendering dispatch
```

### Architectural Boundaries

**Module Boundary — what `src/modules/mod.rs` exports:**
```rust
pub trait Module: Send + Sync {
    fn id(&self) -> ModuleId;
    fn subscription(&self) -> iced::Subscription<Message>;
    fn view_collapsed(&self, state: &ModuleState) -> iced::Element<Message>;
    fn view_expanded(&self, state: &ModuleState) -> iced::Element<Message>;
    fn on_interaction(&self, event: InteractionEvent) -> Option<Message> { None }
}
```
Modules are owned by the registry. Surface code calls `view_*()` only — no module internals leak into `crates/niribar/src/surface/`.

**Surface Boundary:** `crates/niribar/src/surface/` is pure render/state logic. It receives `SurfaceState` and a slice of `(ModuleId, &ModuleState)` per zone. It has no knowledge of D-Bus, Niri IPC, or module internals.

**Config Boundary:** `Config` is fully parsed and validated before any other component initializes. `crates/niribar/src/config/` exports only the typed `Config` struct and `parse()` — no KDL types leak into the rest of the codebase.

**D-Bus Boundary:** `src/dbus/mod.rs` owns the zbus connections (session bus + system bus). All D-Bus module subscriptions borrow handles; they never create their own connections. This enforces the single-connection-per-bus constraint (NFR2/NFR3).

### Data Flow

```
Startup:
  config/parse.rs  →  Config (or exit with error)
  main.rs  →  iced_layershell::Application::run(initial state from Config)

Runtime (event-driven):
  niri/subscription.rs     →  Message::Niri(NiriEvent)
  dbus/*                   →  Message::Module(ModuleId, {Name}Event)
  surface/expand.rs tick   →  Message::Surface(SurfaceId, SurfaceEvent::Tick)
  pointer/keyboard input   →  Message::Surface(SurfaceId, SurfaceEvent::*)

  app.rs update() routes all Message variants:
    Niri events   →  SharedState.niri
    Module events →  SharedState.modules[id]
    Surface events → surfaces[id]

  app.rs view() per surface:
    surface/layout.rs renders zones using SharedState.modules
    surface/expand.rs applies width interpolation from SurfaceState.expand
```

### External Integration Points

| Integration | Entry Point | Bus/Protocol |
|---|---|---|
| Niri compositor | `crates/niribar/src/niri/subscription.rs` | Unix socket, `niri-ipc` event stream |
| UPower (battery) | `crates/niribar/src/dbus/upower.rs` | D-Bus session bus |
| NetworkManager (network) | `crates/niribar/src/dbus/networkmanager.rs` | D-Bus **system** bus |
| WirePlumber (audio) | `crates/niribar/src/dbus/wireplumber.rs` | D-Bus session bus |
| User scripts | `crates/niribar/src/modules/script.rs` | `tokio::process::Command` |
| User config | `src/config/parse.rs` | File read at startup only |

**Note:** NetworkManager uses the system bus. `src/dbus/mod.rs` must manage both a session bus handle and a system bus handle.

## Architecture Validation Results

### Coherence Validation

**Decision Compatibility — PASS**
- Single Tokio runtime shared by Iced (iced_layershell), zbus async, and niri-ipc — no runtime conflicts
- iced_layershell 0.13.7 pins Iced to 0.13.x — consistent throughout; no version conflicts
- `anyhow` for startup/config errors and typed `{Name}Event` variants for runtime state — complementary
- Single zbus connection per bus shared across all D-Bus modules — consistent with boundary rules

**Pre-build confirmation required:** verify iced_layershell 0.13.7 compiles against the target Iced 0.13.x patch; pin to exact patch version if needed.

**Pattern Consistency — PASS**
All patterns derive from and are consistent with the architectural decisions: `SharedState`/`SurfaceState` split maps to the single-app/multiple-surfaces decision; Module subscription contract maps to the async-only data sources decision; config fail-fast maps to config-first initialization.

**Structure Alignment — PASS**
Every architectural decision has a corresponding location in the directory tree. `niri/` and `dbus/` separation maps to IPC and D-Bus decisions. `config/` boundary isolates KDL parsing from the rest of the codebase.

### Requirements Coverage Validation

**FR Coverage — 43/43 COVERED**

| FR Range | Coverage |
|---|---|
| FR1–5 (bar rendering) | `crates/niribar/src/surface/`, `crates/niribar/src/main.rs`, iced_layershell multi-surface API |
| FR6–10 (expand/collapse) | `src/surface/expand.rs`, `SurfaceState.expand`, `Message::Surface` tick |
| FR11–16 (module system) | `src/modules/mod.rs` Module trait + registry; `script.rs` for FR16 |
| FR17–21 (Niri modules) | `crates/niribar/src/niri/`, `workspaces.rs`, `focused_window.rs`; `NiriEvent::Unavailable` for FR21 |
| FR22–28 (service modules) | `crates/niribar/src/modules/{battery,network,volume,clock,button}.rs`; `on_interaction()` for FR27 |
| FR29–31 (keyboard) | `crates/niribar/src/surface/input.rs`, iced_layershell keyboard focus API |
| FR32–38 (config) | `crates/niribar/src/config/`; per-output `OutputConfig`; stderr errors for FR37; `--version` in `main.rs` for FR38 |
| FR39–43 (packaging) | `cargo build`; `packaging/systemd/`; `flake.nix` at repo root; `crates/niribar/config/default.kdl` |

**NFR Coverage — 11/11 ADDRESSED**

| NFR | Coverage |
|---|---|
| NFR1 (animation quality) | `ExpandState` f32 progress + `Message::Tick`; easing curve defined at implementation time |
| NFR2–3 (resource parity) | Single Tokio runtime, single D-Bus connection per bus, GPU rendering via wgpu |
| NFR4 (one-frame IPC latency) | Push-based niri-ipc event stream; no polling |
| NFR5 (cold start) | Config parsed synchronously; surfaces initialize immediately after |
| NFR6 (async D-Bus) | All D-Bus via zbus async API as Iced subscriptions; render loop never blocked |
| NFR7 (no privilege escalation) | `tokio::process::Command` inherits user permissions; no setuid |
| NFR8 (no network) | No network crates in dependency set; local IPC only |
| NFR9 (keyboard operability) | `crates/niribar/src/surface/input.rs`; all module interactions via `Module::on_interaction()` |
| NFR10 (WCAG AA) | Enforced in default theme at implementation; single wgpu rendering path |
| NFR11 (HiDPI) | iced_layershell handles scale factor via wlr-layer-shell protocol |

### Gap Analysis Results

**Critical gaps: 0**

**Important gaps addressed:**
1. `flake.nix` and `flake.lock` added to repo root in directory structure (FR41)
2. Dual D-Bus bus handles (session + system) documented explicitly in boundary notes

### Architecture Completeness Checklist

- [x] Project context analyzed — 4 subsystems, 6 components, cross-cutting concerns mapped
- [x] Starter template selected — `cargo new` + iced_layershell, rationalized against alternatives
- [x] Core dependency set with pinned versions — iced_layershell 0.13.7, zbus 5.12.0, kdl 6.5.0, tokio 1.x
- [x] Critical architectural decisions documented — app model, module system, Niri IPC, config
- [x] Implementation patterns defined — naming, state boundary, message routing, error handling
- [x] Complete project structure — all files mapped to FRs
- [x] All 43 FRs architecturally covered
- [x] All 11 NFRs architecturally addressed
- [x] External integration points and bus types documented
- [x] CI/CD pipeline defined

### Architecture Readiness Assessment

**Overall Status: READY FOR IMPLEMENTATION**

**Confidence: High** — Technology choices are well-understood; bar-rs provides reference prior art; iced_layershell integration is the single highest-risk point and is explicitly called out for early prototyping before module work begins.

**Key strengths:**
- Single Tokio runtime + single D-Bus connection design directly serves NFR2/NFR3 resource goals
- Module subscription contract with mandatory `Unavailable` emission makes the bar robust by construction
- `SharedState`/`SurfaceState` boundary cleanly handles multi-output complexity without cross-process coordination
- Static module registry keeps MVP scope contained; dynamic plugin path identified as Phase 2

**Post-MVP enhancement areas:**
- Dynamic plugin loading via `dlopen` (Phase 2 plugin API — FR11 extension)
- Cargo workspace split if codebase warrants it
- `aarch64` release target when CI infrastructure allows
- Integration test suite with D-Bus session mocking

### Implementation Handoff

**First implementation story (Story 1):**
```bash
cargo new niribar --bin
```
Then populate `Cargo.toml` with the pinned dependency set from the Starter Template section. Verify iced_layershell 0.13.7 compiles cleanly before any other work begins.

**Implementation sequence (from Core Architectural Decisions section):**
1. Project scaffold + Cargo.toml
2. iced_layershell surface rendering (single output, static bar)
3. Expand/collapse animation and state
4. Config parsing (KDL → typed Config struct)
5. Module trait + subscription infrastructure
6. Niri IPC subscription wrapper
7. zbus D-Bus connection management
8. Built-in modules (clock → workspaces → battery → network → volume → script → button)
9. Multi-output support
10. Keyboard input handling
11. Packaging artifacts (systemd unit, Nix flake, default config)

**All agents must read this document before writing any code.**
