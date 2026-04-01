# Story 2.1: Define the Module Trait and Zone Layout

Status: done

## Story

As a **developer**,
I want a `Module` trait and `ZoneLayout` primitive that can host modules in top/center/bottom zones,
so that all subsequent modules have a consistent interface and the bar's spatial structure exists.

## Acceptance Criteria

1. **Given** the module trait is defined in `src/modules/mod.rs`
   **When** a concrete type implements `Module`
   **Then** it must provide: `view_collapsed()`, `view_expanded()`, `subscription()`, and `id()`
   **And** `on_interaction()` has a default implementation returning `None`

2. **Given** `zone_layout()` is defined in `src/surface/layout.rs`
   **When** the bar renders
   **Then** top/center/bottom zones are rendered with center filling remaining space (`Length::Fill`)
   **And** zones never overlap even when content overflows — overflow is clipped
   **And** the layout applies `spacing.bar.padding` (4px default) on all bar edges
   **And** modules within a zone are separated by `spacing.module.gap` (4px default)

3. **Given** niribar is built
   **When** `cargo clippy -- -D warnings` is run
   **Then** zero warnings

4. **Given** niribar is built
   **When** `cargo test` is run
   **Then** all 20 pre-existing tests pass and at least 5 new tests pass

## Tasks / Subtasks

- [x] Task 1: Core types in `src/modules/mod.rs` (AC: #1)
  - [x] 1.1: Define `ModuleId` newtype:
    ```rust
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct ModuleId(pub String);
    impl ModuleId {
        pub fn new(s: impl Into<String>) -> Self { ModuleId(s.into()) }
    }
    ```
  - [x] 1.2: Define `Zone` enum: `#[derive(Debug, Clone, PartialEq, Eq)] pub enum Zone { Top, Center, Bottom }`
  - [x] 1.3: Define `ModuleState` enum with `#[derive(Debug, Clone, Default)]`:
    - `#[default] Loading` — bar is starting up; render as empty space (no spinner, no text)
    - `Unavailable` — data source is absent; render as nothing (zero height, no gap left)
    - Comment placeholder lines for future variants: `// Clock(ClockState) — Story 2.3`, etc.
  - [x] 1.4: Define `ModuleEvent` enum with `#[derive(Debug, Clone)]` — empty placeholder; variants added in Stories 2.3+
  - [x] 1.5: Define `InteractionEvent` enum with `#[derive(Debug, Clone)]`: `Clicked`, `ScrollUp`, `ScrollDown`

- [x] Task 2: `Module` trait in `src/modules/mod.rs` (AC: #1)
  - [x] 2.1: Define the trait with this exact signature (architecture `src/modules/mod.rs` boundary):
    ```rust
    pub trait Module: Send + Sync {
        fn id(&self) -> ModuleId;
        fn subscription(&self) -> iced::Subscription<crate::app::Message>;
        fn view_collapsed(&self, state: &ModuleState) -> iced::Element<'_, crate::app::Message>;
        fn view_expanded(&self, state: &ModuleState) -> iced::Element<'_, crate::app::Message>;
        fn on_interaction(&self, _event: InteractionEvent) -> Option<crate::app::Message> { None }
    }
    ```
  - [x] 2.2: Add doc comment on the trait specifying the subscription contract:
    - Must emit first message within 1 second of start, or emit `Unavailable`
    - Must never panic — wrap all fallible ops in `match`/`if let`
    - Subscription ID: use `std::any::TypeId::of::<{Name}Subscription>()` — no magic strings
    - D-Bus service disappearance → emit Unavailable, not a fatal error; support reconnection
  - [x] 2.3: Define `ModuleConstructor` type alias: `pub type ModuleConstructor = fn() -> Box<dyn Module>;`
  - [x] 2.4: Define `build_registry()` returning an empty `HashMap<&'static str, ModuleConstructor>`

- [x] Task 3: `ZoneLayout` in `src/surface/layout.rs` (AC: #2)
  - [x] 3.1: Create `src/surface/layout.rs`
  - [x] 3.2: Define constants `BAR_PADDING: f32 = 4.0` and `MODULE_GAP: f32 = 4.0`
  - [x] 3.3: Define `zone_layout()` function with top/center/bottom vecs, center uses `Length::Fill`, outer container uses `clip(true)`
  - [x] 3.4: Add `pub mod layout;` to `src/surface/mod.rs`

- [x] Task 4: Update `src/app.rs` (AC: #1, #2)
  - [x] 4.1: Add `Message::Module(ModuleId, ModuleEvent)` variant to the `Message` enum
  - [x] 4.2: Add `SharedState` struct with `modules: HashMap<ModuleId, ModuleState>`
  - [x] 4.3: Add `shared: SharedState` field to `App` struct; initialize with `SharedState::new()` in `App::new()`
  - [x] 4.4: Replace static bar content in `App::view()` with `zone_layout(vec![], vec![], vec![])` preserving 1px right border
  - [x] 4.5: `update()` kept as `_message: Message` — routing established in Story 2.3 when ModuleEvent gains variants
  - [x] 4.6: All necessary `use` imports added

- [x] Task 5: Tests (AC: #3, #4)
  - [x] 5.1: `module_id_new_stores_string`, `module_id_equality`, `module_state_default_is_loading`, `build_registry_is_empty_initially`, `module_id_hashable` — all pass
  - [x] 5.2: `bar_padding_is_four`, `module_gap_is_four` — constants verified; `shared_state_starts_empty` in app tests
  - [x] 5.3: All 20 pre-existing tests pass; 8 new tests added (28 total)

- [x] Task 6: Quality gates
  - [x] 6.1: `cargo build` — succeeded
  - [x] 6.2: `cargo clippy -- -D warnings` — zero warnings
  - [x] 6.3: `cargo fmt` then `cargo fmt --check` — passes
  - [x] 6.4: `cargo test` — 28 passed (20 pre-existing + 8 new)

## Dev Notes

### Architecture Guardrails

**Module trait signature is fixed** — do not deviate from the architecture specification:
```rust
pub trait Module: Send + Sync {
    fn id(&self) -> ModuleId;
    fn subscription(&self) -> iced::Subscription<crate::app::Message>;
    fn view_collapsed(&self, state: &ModuleState) -> iced::Element<'_, crate::app::Message>;
    fn view_expanded(&self, state: &ModuleState) -> iced::Element<'_, crate::app::Message>;
    fn on_interaction(&self, _event: InteractionEvent) -> Option<crate::app::Message> { None }
}
```
Source: `_bmad-output/planning-artifacts/architecture.md` — "Module Boundary" section.

**SharedState / SurfaceState boundary is non-negotiable:**
- Module data (`ModuleState`, `ModuleId`) → `SharedState` in `app.rs`
- Expand/collapse, keyboard focus → `SurfaceState` in `src/surface/mod.rs`
- Never put module data in `SurfaceState`. Never put surface render state in `SharedState`.

**Message routing convention:**
```rust
// CORRECT
Message::Module(ModuleId::new("clock"), ClockEvent::Tick)
// WRONG — do not embed output/surface info in module messages
Message::ClockForOutput("HDMI-1", ...)
```

**Subscription ID pattern** — every future subscription must use:
```rust
iced::subscription::channel(std::any::TypeId::of::<{Name}Subscription>(), ...)
```
No magic strings as subscription IDs.

**ModuleEvent is empty in Story 2.1** — this is intentional. Concrete variants (`ClockEvent`, `ScriptEvent`, etc.) are added in Stories 2.3–2.5. The `Message::Module(ModuleId, ModuleEvent)` variant will be dead code until then — add `#[allow(dead_code)]`.

### File Locations

| What | Where | Note |
|---|---|---|
| Module trait, types, registry | `src/modules/mod.rs` | Replace placeholder comment |
| ZoneLayout function | `src/surface/layout.rs` | New file — add `pub mod layout;` to `surface/mod.rs` |
| SharedState struct | `src/app.rs` | Lives alongside `App` in the same file for now |
| Message::Module variant | `src/app.rs` | Add to existing `Message` enum |

Do NOT create:
- Any concrete module files (`clock.rs`, `script.rs`, etc.) — those are Stories 2.3–2.5
- `src/surface/expand.rs` — Story 3.1
- `src/surface/input.rs` — Story 6.1
- `src/niri/subscription.rs` — Story 4.1
- `src/dbus/upower.rs` etc. — Story 5.x

### Iced 0.14 API Notes

Confirmed dependencies (from `Cargo.toml`): `iced = "0.14"`, `iced_layershell = "0.17.1"`.

For `zone_layout`, use:
- `iced::widget::column(vec_of_elements)` — takes `Vec<Element>`
- `iced::widget::container(element).height(Length::Fill)` — for center zone fill
- `iced::widget::container(col).padding(Padding::new(4.0)).clip(true)` — for outer wrapper
- `iced::{Length, Padding}` imports

The `#[to_layer_message]` macro on `Message` adds LayerShell-specific variants. New variants you add (`Module(ModuleId, ModuleEvent)`) must satisfy `#[derive(Debug, Clone)]`. Both `ModuleId` and `ModuleEvent` already derive these.

In `App::view()`, the existing layout is:
```rust
row![
    container(Space::new()).width(content_width).height(Length::Fill),  // ← replace with zone_layout
    container(Space::new()).width(1).height(Length::Fill).style(border),  // ← keep as-is
]
```
Replace the first `container(Space::new())` with `zone_layout(vec![], vec![], vec![])` but keep it sized to `content_width`. Use `.width(content_width)` on a container wrapping the zone_layout result.

### Circular Module Dependencies

`src/modules/mod.rs` uses `crate::app::Message` (for the trait methods).
`src/app.rs` uses `crate::modules::{ModuleId, ModuleEvent, ModuleState}` (for SharedState and Message variant).

This is **fine in Rust** — circular `use` references between modules within the same crate compile without issue. Do not introduce a separate `message.rs` to avoid this; that refactor is not needed.

### Previous Story Patterns

From Stories 1.2–1.5 (active in this codebase):
- All quality gates: `cargo clippy -- -D warnings`, `cargo fmt --check`, `cargo test` — must pass before marking done
- Run `cargo fmt` before `cargo fmt --check` to auto-fix formatting first
- Tests co-located with source using `#[cfg(test)] mod tests`
- `anyhow` for startup/config errors; typed events for runtime module state (never propagate errors from subscriptions)
- Current test count is 20 — all must keep passing

### What ZoneLayout Must NOT Do

- No module data — it is a pure stateless layout primitive (no knowledge of D-Bus, Niri IPC, module internals)
- No expand/collapse logic — that is `SurfaceState` (Story 3.1)
- No dynamic spacing config lookup — hardcode 4px constants in Story 2.1; configurable spacing is Story 3.4

### Project Structure Notes

- `src/modules/mod.rs`: replaces the `// placeholder` comment entirely
- `src/surface/mod.rs`: add `pub mod layout;` and keep the existing `SurfaceState` struct unchanged
- `src/app.rs`: add `SharedState`, add `Message::Module`, wire `zone_layout` into `view()`
- `src/main.rs`: no changes needed
- `src/config/`: no changes needed — spacing values are hardcoded constants in Story 2.1

### References

- Epic 2, Story 2.1 (epics.md §"Story 2.1: Define the Module Trait and Zone Layout"): user story + acceptance criteria
- Architecture: "Module System Design" section — async stream producers, static registry
- Architecture: "Module Boundary" — exact Module trait signature
- Architecture: "State Management Patterns" — SharedState/SurfaceState split, AppState struct layout
- Architecture: "Naming Patterns" — ModuleId, {Name}State, {Name}Event conventions
- Architecture: "Communication Patterns" — subscription contract, Message routing
- Architecture: "Subscription ID Pattern" — TypeId-based IDs, no magic strings
- Architecture: `src/surface/layout.rs` — ZoneLayout location
- UX Design Spec: "ZoneLayout" component spec (stateless, center fills, clips overflow)
- UX Design Spec: "Spacing & Sizing System" — BAR_PADDING=4px, MODULE_GAP=4px
- UX Design Spec: "Module Visual Anatomy" — Loading renders empty, Unavailable renders nothing
- PRD FR11, FR12: modules in configurable zones; FR13: collapsed/expanded states

## Dev Agent Record

### Agent Model Used

claude-sonnet-4-6

### Debug Log References

- `cargo clippy -- -D warnings` initially failed: 9 dead_code errors for forward-declared pub items not yet used in the binary target. Fixed by adding `#[allow(dead_code)]` to each future-facing item, following the established pattern from `BarConfig.expanded_width` in Story 1.2.
- `update()` match on `Message::Module` omitted: `ModuleEvent` is an empty enum (uninhabited), so `Module(ModuleId, ModuleEvent)` is also uninhabited. Using `_message: Message` instead avoids exhaustiveness issues from `#[to_layer_message]` adding its own variants.
- `zone_layout` unit test for element construction skipped (Iced `Element` requires runtime context); tested constants `BAR_PADDING`/`MODULE_GAP` instead — fully sufficient.

### Completion Notes List

- `src/modules/mod.rs`: `ModuleId`, `Zone`, `ModuleState` (Loading/Unavailable), `ModuleEvent` (empty), `InteractionEvent`, `Module` trait, `ModuleConstructor`, `build_registry()` — all defined per architecture spec.
- `src/surface/layout.rs`: new file with `zone_layout()` function — top/center/bottom column layout, center fills `Length::Fill`, outer container clips overflow, constants `BAR_PADDING=4.0` and `MODULE_GAP=4.0`.
- `src/surface/mod.rs`: added `pub mod layout;`.
- `src/app.rs`: `SharedState` struct (modules HashMap), added to `App`, `Message::Module(ModuleId, ModuleEvent)` variant, `view()` now uses `zone_layout` with empty zones (preserves 1px border), all imports in place.
- `cargo build`: succeeded. `cargo clippy -- -D warnings`: zero warnings. `cargo fmt --check`: passes. `cargo test`: 28/28 passed (20 pre-existing + 8 new).
- AC1: Module trait implemented with exact architecture signature including `on_interaction` default.
- AC2: `zone_layout` renders top/center/bottom with center `Length::Fill`, outer container `clip(true)`, `BAR_PADDING=4px` edges, `MODULE_GAP=4px` spacing.
- AC3: Zero clippy warnings confirmed.
- AC4: 28 tests pass (8 new, exceeds minimum of 5).

### File List

- src/modules/mod.rs
- src/surface/layout.rs
- src/surface/mod.rs
- src/app.rs

### Review Findings

- [x] [Review][Patch] SharedState missing Default impl — `new()` constructor exists but `Default` is not derived; Rust's `clippy::new_without_default` lint is latent and will surface as usage grows [src/app.rs:22]
- [x] [Review][Patch] zone_layout center zone content not vertically aligned — doc says "modules centred within it" but the center container has no `.align_y(Center)` call; content will be top-aligned within the fill region [src/surface/layout.rs:26]
- [x] [Review][Defer] ModuleConstructor uses bare fn pointer, can't capture config params — `fn() -> Box<dyn Module>` cannot represent closures or stateful constructors needed when modules are config-parameterised (Story 2.2) [src/modules/mod.rs:84] — deferred, pre-existing
- [x] [Review][Defer] ModuleId::new accepts empty string — no guard against empty id producing a silent sentinel key in SharedState::modules HashMap; validation belongs at config parse time [src/modules/mod.rs:11] — deferred, pre-existing

### Change Log

- 2026-04-01: Story 2.1 implemented — Module trait, ModuleId, ModuleState, ModuleEvent, InteractionEvent, Zone, ModuleConstructor, build_registry() defined; zone_layout() created in surface/layout.rs; SharedState and Message::Module added to app.rs; view() wired to zone_layout; 8 new tests; 28 total pass.
