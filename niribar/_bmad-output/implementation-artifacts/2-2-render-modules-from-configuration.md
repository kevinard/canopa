# Story 2.2: Render Modules from Configuration

Status: done

## Story

As a **user**,
I want the modules listed in my config to appear in the bar in the order and zones I specified,
so that I control what the bar shows and where.

## Acceptance Criteria

1. **Given** the config file specifies a `modules` list with zone placements
   **When** niribar starts
   **Then** exactly the configured modules are instantiated in config order, placed in their specified zones
   **And** modules not listed in config are not rendered

2. **Given** a module name in config is unrecognized
   **When** niribar starts
   **Then** an error matching FR37 format is printed to stderr and the process exits with code 1
   **Format:** `error: modules[N].type: unknown module "<name>". Expected one of: battery, clock, network, volume, workspaces, focused-window, script, button.`

3. **Given** a module is rendered
   **When** the pointer enters the module area
   **Then** a rounded-rect background fill (`#313244` / Catppuccin Surface0, 4px radius) appears behind the module content
   **And** the fill disappears when the pointer exits

4. **Given** niribar is built
   **When** `cargo clippy -- -D warnings` is run
   **Then** zero warnings

5. **Given** niribar is built
   **When** `cargo test` is run
   **Then** all 28 pre-existing tests pass and at least 5 new tests pass

## Tasks / Subtasks

- [x] Task 1: `ModuleConfig` in `src/config/mod.rs` (AC: #1, #2)
  - [x] 1.1: Add `ModuleConfig` struct:
    ```rust
    pub struct ModuleConfig {
        pub name: String,
        pub zone: crate::modules::Zone,
    }
    ```
  - [x] 1.2: Add `modules: Vec<ModuleConfig>` field to `Config`; update `Config::default()` to `modules: vec![]`
  - [x] 1.3: Add validation constant (not pub — internal to config):
    ```rust
    const KNOWN_MODULE_NAMES: &[&str] = &[
        "battery", "clock", "network", "volume",
        "workspaces", "focused-window", "script", "button",
    ];
    ```
  - [x] 1.4: Update `validate()` to check each `ModuleConfig::name` against `KNOWN_MODULE_NAMES`:
    - Error format (N = 0-indexed position): `modules[{N}].type: unknown module "{name}". Expected one of: battery, clock, network, volume, workspaces, focused-window, script, button.`
    - Use `anyhow::bail!` — consistent with existing validate() error style

- [x] Task 2: Parse modules from KDL in `src/config/parse.rs` (AC: #1)
  - [x] 2.1: Add `parse_modules_section(doc: &KdlDocument) -> anyhow::Result<Vec<ModuleConfig>>`:
    - Look for `modules` node in top-level doc; if absent → return `vec![]`
    - Iterate children: each child node named `module`
    - First positional arg is the module name string
    - `zone` property: `"top"` → `Zone::Top`, `"center"` → `Zone::Center`, `"bottom"` → `Zone::Bottom`; unknown zone → error `modules[N].zone: unknown value "{v}". Expected: one of: top, center, bottom.`
    - Field path uses 0-indexed position: `modules[0].zone`, `modules[1].type`, etc.
  - [x] 2.2: Call `parse_modules_section(&doc)?` inside `parse()` and populate `Config { bar, modules }`
  - [x] 2.3: Update `config/default.kdl` to add modules section with clock in bottom zone:
    ```kdl
    modules {
        module "clock" zone="bottom"
    }
    ```
    (clock passes validation as a known name; instantiation is a no-op until Story 2.3 adds it to the registry)

- [x] Task 3: Implement `ModuleFrame` in `src/surface/frame.rs` (AC: #3)
  - [x] 3.1: Create `src/surface/frame.rs`
  - [x] 3.2: Implement `module_frame()`:
    ```rust
    use iced::widget::container;
    use iced::{Color, Element};

    pub fn module_frame<'a, Message: 'a>(
        content: Element<'a, Message>,
        hovered: bool,
    ) -> Element<'a, Message> {
        container(content)
            .style(move |_| {
                if hovered {
                    container::Style {
                        background: Some(iced::Background::Color(Color::from_rgb8(0x31, 0x32, 0x44))),
                        border: iced::Border {
                            radius: 4.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                } else {
                    container::Style::default()
                }
            })
            .into()
    }
    ```
  - [x] 3.3: Add `pub mod frame;` to `src/surface/mod.rs`

- [x] Task 4: Update `src/app.rs` — module instantiation, hover state, wired view (AC: #1, #3)
  - [x] 4.1: Add hover message variants to `Message` enum:
    ```rust
    pub enum Message {
        Module(ModuleId, ModuleEvent),
        ModuleHoverEnter(ModuleId),
        ModuleHoverExit(ModuleId),
    }
    ```
  - [x] 4.2: Add fields to `App`:
    ```rust
    pub struct App {
        pub config: Config,
        pub shared: SharedState,
        pub modules: Vec<(Zone, Box<dyn Module>)>,
        pub hovered_module: Option<ModuleId>,
    }
    ```
  - [x] 4.3: Update `App::new()` to instantiate modules from registry:
    ```rust
    pub fn new(config: Config) -> Self {
        let registry = build_registry();
        let modules: Vec<(Zone, Box<dyn Module>)> = config.modules.iter()
            .filter_map(|mc| {
                registry.get(mc.name.as_str()).map(|ctor| (mc.zone.clone(), ctor()))
            })
            .collect();
        App { config, shared: SharedState::new(), modules, hovered_module: None }
    }
    ```
    (Registry is empty in Story 2.2 — `modules` will be empty until Story 2.3 populates the registry)
  - [x] 4.4: Update `App::update()` to handle hover messages:
    ```rust
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ModuleHoverEnter(id) => {
                self.hovered_module = Some(id);
                Task::none()
            }
            Message::ModuleHoverExit(id) => {
                if self.hovered_module.as_ref() == Some(&id) {
                    self.hovered_module = None;
                }
                Task::none()
            }
            _ => Task::none(), // Module event routing established in Story 2.3
        }
    }
    ```
  - [x] 4.5: Update `App::view()` to partition modules by zone and wrap each in `ModuleFrame`:
    ```rust
    pub fn view(&self) -> Element<'_, Message> {
        use crate::modules::Zone;
        use crate::surface::frame::module_frame;
        use iced::widget::mouse_area;

        let content_width = self.config.bar.collapsed_width.saturating_sub(1);
        let mut top: Vec<Element<'_, Message>> = vec![];
        let mut center: Vec<Element<'_, Message>> = vec![];
        let mut bottom: Vec<Element<'_, Message>> = vec![];

        for (zone, module) in &self.modules {
            let state = self.shared.modules.get(&module.id())
                .cloned()
                .unwrap_or_default();
            let id = module.id();
            let hovered = self.hovered_module.as_ref() == Some(&id);
            let view: Element<'_, Message> = mouse_area(module_frame(
                module.view_collapsed(&state),
                hovered,
            ))
            .on_enter(Message::ModuleHoverEnter(id.clone()))
            .on_exit(Message::ModuleHoverExit(id))
            .into();

            match zone {
                Zone::Top => top.push(view),
                Zone::Center => center.push(view),
                Zone::Bottom => bottom.push(view),
            }
        }

        row![
            container(zone_layout(top, center, bottom))
                .width(content_width)
                .height(Length::Fill),
            // 1px right border strip — keep as-is from Story 1.2
            container(Space::new())
                .width(1)
                .height(Length::Fill)
                .style(|_theme| container::Style {
                    background: Some(iced::Background::Color(Color::from_rgb8(0x45, 0x47, 0x5a))),
                    ..Default::default()
                }),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
    ```
  - [x] 4.6: Add `App::subscription()`:
    ```rust
    pub fn subscription(&self) -> iced::Subscription<Message> {
        iced::Subscription::batch(self.modules.iter().map(|(_, m)| m.subscription()))
    }
    ```
  - [x] 4.7: Update all necessary `use` imports in `app.rs`

- [x] Task 5: Wire subscription in `src/main.rs` (AC: #1)
  - [x] 5.1: Add `.subscription(App::subscription)` to the `application(...)` builder chain, before `.run()`

- [x] Task 6: Tests (AC: #4, #5)
  - [x] 6.1: In `src/config/parse.rs` tests:
    - `parse_modules_section_absent` — no modules block in KDL → `config.modules.is_empty()`
    - `parse_single_module_bottom_zone` — `modules { module "clock" zone="bottom" }` → name="clock", zone=Zone::Bottom
    - `parse_multiple_modules_order_preserved` — two modules → order preserved (clock first, then battery)
    - `parse_unknown_zone_errors` — `module "clock" zone="diagonal"` → error contains `modules[0].zone:`
  - [x] 6.2: In `src/config/mod.rs` tests:
    - `validate_unknown_module_name_errors` — name="volumee" → error starts with `modules[0].type:`
    - `validate_known_module_name_passes` — name="clock" → `validate()` returns Ok
    - `validate_error_includes_expected_list` — error message contains "Expected one of:" and "clock"
  - [x] 6.3: In `src/app.rs` tests:
    - `hover_enter_sets_hovered_module` — `App::update(Message::ModuleHoverEnter(id))` → `app.hovered_module == Some(id)`
    - `hover_exit_clears_hovered_module` — enter then exit same id → `hovered_module == None`
    - `hover_exit_different_id_does_not_clear` — enter id_a, exit id_b → `hovered_module == Some(id_a)`
  - [x] 6.4: All 28 pre-existing tests still pass

- [x] Task 7: Quality gates
  - [x] 7.1: `cargo build` — succeeded
  - [x] 7.2: `cargo clippy -- -D warnings` — zero warnings
  - [x] 7.3: `cargo fmt` then `cargo fmt --check` — passes
  - [x] 7.4: `cargo test` — 14 new tests + all 28 pre-existing pass (42 total)

### Review Findings

- [x] [Review][Patch] `parse_modules_section` accepts non-`module` child nodes — any KDL child inside `modules {}` with a positional string arg is silently parsed as a module regardless of its node name [src/config/parse.rs:25]
- [x] [Review][Defer] Duplicate ModuleId not validated — two config entries with the same module name produce identical `ModuleId` instances; hover highlights both simultaneously and `shared.modules` HashMap clobbers the second's state [src/config/mod.rs, src/app.rs] — deferred, pre-existing design gap; relevant only when real module instances exist (Story 2.3+)
- [x] [Review][Defer] Hover stuck if surface loses focus without MouseExit — `hovered_module` stays `Some(id)` with no clearing mechanism if compositor hides the surface [src/app.rs:79-83] — deferred, resolves in Story 7.1 when `hovered_module` moves to `SurfaceState`

## Dev Notes

### Architecture Guardrails

**Module trait and types are fixed** — do not deviate:
```rust
pub trait Module: Send + Sync {
    fn id(&self) -> ModuleId;
    fn subscription(&self) -> iced::Subscription<crate::app::Message>;
    fn view_collapsed(&self, state: &ModuleState) -> iced::Element<'_, crate::app::Message>;
    fn view_expanded(&self, state: &ModuleState) -> iced::Element<'_, crate::app::Message>;
    fn on_interaction(&self, _event: InteractionEvent) -> Option<crate::app::Message> { None }
}
```

**SharedState / SurfaceState boundary** — never cross it:
- Module data (`ModuleState`, `ModuleId`) → `SharedState.modules` in `app.rs`
- Hover state (`hovered_module: Option<ModuleId>`) → lives in `App` for now; moves to `SurfaceState` in Story 7.1 (multi-output)
- Expand/collapse, keyboard focus → `SurfaceState` in `src/surface/mod.rs` (Stories 3.1, 6.1)

**Message routing convention:**
```rust
// CORRECT — hover routed by ModuleId
Message::ModuleHoverEnter(ModuleId::new("clock"))
// CORRECT — module data broadcast
Message::Module(ModuleId::new("clock"), ClockEvent::Tick)
```

**`#[to_layer_message]` macro caveat** — the macro adds LayerShell-specific variants to `Message`. When writing `match message { ... }`, always include a `_ => Task::none()` catch-all to handle macro-injected variants. Do NOT try to exhaustively match all variants.

**ModuleConstructor still `fn() -> Box<dyn Module>`** — module-specific config (clock format string, script command) is deferred to Stories 2.3–2.5 when each module needs it. Do not change the constructor signature in Story 2.2.

**Empty registry is correct** — `build_registry()` returns an empty `HashMap` in Story 2.2. `App::new()` will produce `self.modules = vec![]`. All zones render empty. This is correct behavior — registry is populated in Stories 2.3–2.5.

**FR37 validation uses hardcoded list** — the set of known module names is a hardcoded `KNOWN_MODULE_NAMES` constant in `src/config/mod.rs`. It is not derived from `build_registry()` keys (which are empty). The list is intentionally future-complete so that `module "clock"` in config passes validation in Story 2.2 even before clock is implemented.

**Default config must not break startup** — after Task 2.3, `config/default.kdl` will have `module "clock" zone="bottom"`. This passes validation (clock is in `KNOWN_MODULE_NAMES`). At instantiation, registry is empty, so it renders as nothing. When Story 2.3 adds clock to the registry, it will automatically start rendering.

### File Locations

| What | Where | Note |
|---|---|---|
| `ModuleConfig`, `KNOWN_MODULE_NAMES` | `src/config/mod.rs` | Add to existing file |
| Modules KDL parsing | `src/config/parse.rs` | Add `parse_modules_section()` |
| `module_frame()` | `src/surface/frame.rs` | New file |
| `App` changes + hover logic | `src/app.rs` | Extend existing |
| Subscription wiring | `src/main.rs` | Add `.subscription(App::subscription)` |
| Default config modules | `config/default.kdl` | Add modules section |

Do NOT create:
- `src/modules/clock.rs` — Story 2.3
- `src/modules/script.rs` — Story 2.4
- `src/modules/button.rs` — Story 2.5
- `src/modules/battery.rs`, `src/modules/network.rs`, `src/modules/volume.rs` — Stories 5.x
- `src/niri/subscription.rs` — Story 4.1
- `src/surface/expand.rs` — Story 3.1
- `src/surface/input.rs` — Story 6.1

### KDL Config Format

Module section in KDL:
```kdl
modules {
    module "clock" zone="bottom"     // type positional arg, zone property
    module "battery" zone="top"
    module "workspaces" zone="center"
}
```

Parse rules:
- `modules` node is optional at top level; absent → `config.modules = vec![]`
- Each child of `modules` block must be named `module`
- First positional arg (index 0) → module type name (string)
- `zone` property → zone enum: `"top"` / `"center"` / `"bottom"`
- Missing `zone` property → error: `modules[N].zone: missing required property. Expected: one of: top, center, bottom.`
- Unknown `zone` value → error: `modules[N].zone: unknown value "{v}". Expected: one of: top, center, bottom.`

Error field path construction: `N` is the 0-indexed position of the module in the `modules` block.

### Iced 0.14 API Notes

Confirmed: `iced = "0.14"`, `iced_layershell = "0.17.1"`.

**`mouse_area`** — hover detection widget:
```rust
use iced::widget::mouse_area;
mouse_area(content)
    .on_enter(Message::ModuleHoverEnter(id.clone()))
    .on_exit(Message::ModuleHoverExit(id))
```
Note: `mouse_area` wraps any `Element` and delivers `on_enter`/`on_exit` messages.

**`container` style with border radius:**
```rust
container::Style {
    background: Some(iced::Background::Color(Color::from_rgb8(0x31, 0x32, 0x44))),
    border: iced::Border { radius: 4.0.into(), ..Default::default() },
    ..Default::default()
}
```

**`iced::Subscription::batch(iter)`** — merges multiple subscriptions:
```rust
iced::Subscription::batch(self.modules.iter().map(|(_, m)| m.subscription()))
// With empty self.modules, this produces iced::Subscription::none()
```

**iced_layershell subscription wiring** — add to application builder:
```rust
application(factory, namespace, update, view)
    .settings(...)
    .style(App::style)
    .subscription(App::subscription)  // add this line
    .run()
```
If `iced_layershell` 0.17.1's builder does not have `.subscription()`, check the docs for the equivalent. The existing codebase uses `.style(App::style)` as a reference for the pattern.

### From Story 2.1 — Patterns to Follow

- `#[allow(dead_code)]` on all pub items not yet used in the binary (same pattern as Story 2.1 forward-declarations)
- All quality gates: `cargo clippy -- -D warnings`, `cargo fmt --check`, `cargo test` — must pass before marking done
- Run `cargo fmt` before `cargo fmt --check` to auto-fix formatting first
- Tests co-located in `#[cfg(test)] mod tests` at the bottom of each file
- `anyhow` for config/startup errors; typed event variants for runtime module state
- `#[derive(Default)]` on new structs that have a trivial all-default constructor

### Deferred Items Resolved in This Story

From Story 2.1 deferred work:
- ~~ModuleConstructor bare fn pointer~~ — still deferred; constructor signature stays `fn() -> Box<dyn Module>` until Story 2.3 needs module-specific config
- ~~ModuleId::new accepts empty string~~ — still deferred; validation at config parse time is the right location; `modules[N].type` validation in `validate()` already catches empty module names (empty string is not in `KNOWN_MODULE_NAMES`)

### State After Story 2.2

When story 2.2 is complete:
- `config.modules` is populated from KDL; defaults to `[clock@bottom]` from default.kdl
- `app.modules` is **empty** (registry empty until 2.3) — bar renders empty zones as before
- `ModuleFrame` hover fill is wired and working (visible once modules render in 2.3)
- `App::subscription()` is live (returns empty batch until 2.3)
- The full module rendering pipeline is in place; 2.3 just adds clock to the registry

### References

- Epic 2, Story 2.2 (epics.md §"Story 2.2"): user story + acceptance criteria
- Architecture: "Module System Design" — static registry, async stream producers
- Architecture: "Module Boundary" — exact Module trait signature
- Architecture: "State Management Patterns" — SharedState/SurfaceState split
- Architecture: "Naming Patterns" — ModuleId, {Name}State, {Name}Event conventions
- Architecture: "Communication Patterns" — Message routing convention
- Architecture: "Config Boundary" — Config parsed before surface creation
- Architecture: "Process Patterns" — Config error format (FR37)
- UX Design: "ModuleFrame" component spec — hover fill `color.surface` 4px radius
- UX Design: "Loading & Empty State Patterns" — Loading renders empty space, no spinner
- UX Design: "Interaction Feedback Patterns" — hover fill is the sole interactive indicator
- UX Design: "Spacing & Sizing System" — ModuleFrame 4px border radius
- PRD FR11 (configurable module zones), FR12 (module rendering), FR37 (unknown module error)
- Deferred work: `_bmad-output/implementation-artifacts/deferred-work.md` §"2-1"

## Dev Agent Record

### Agent Model Used

claude-sonnet-4-6

### Debug Log References

None — implementation proceeded without blocking issues. Two review patches from story 2-1 code-review were pre-applied before implementation began (SharedState Default derive, zone_layout center alignment).

### Completion Notes List

- `build_registry()` returns empty HashMap in story 2.2; `App::new()` uses `filter_map` so `app.modules` is empty. Full pipeline wired; registry populated in stories 2.3+.
- FR37 validation uses hardcoded `KNOWN_MODULE_NAMES` rather than registry keys so known module names pass validation before their implementations exist.
- `hovered_module: Option<ModuleId>` lives in `App` for now; moves to `SurfaceState` in story 7.1 per architecture docs.
- `#[to_layer_message]` macro injects LayerShell variants into `Message`; catch-all `_ => Task::none()` in `update()` required.
- `cargo fmt` run before `cargo fmt --check` to auto-fix long lines in filter_map and assert! calls.
- 14 new tests added across config/parse.rs (5), config/mod.rs (5), app.rs (4) for a total of 42 passing tests.

### File List

- src/config/mod.rs
- src/config/parse.rs
- config/default.kdl
- src/surface/frame.rs
- src/surface/mod.rs
- src/app.rs
- src/main.rs

### Change Log

- 2026-04-01: Story 2.2 created — module config parsing, ModuleFrame hover, module instantiation pipeline, subscription wiring.
- 2026-04-01: Story 2.2 implemented — all 7 tasks complete, 42 tests passing, zero clippy warnings.
