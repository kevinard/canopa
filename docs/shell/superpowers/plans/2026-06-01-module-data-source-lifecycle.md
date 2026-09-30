# Module Data Source Lifecycle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Introduce a runtime-owned **Module Data Source** lifecycle for battery, network, and volume so source identity, availability, first emission, recovery, and fanout live behind one deep module.

**Architecture:** Add source-shaped **Module Data Source State** and explicit per-**Module Instance** outcomes. **Module Implementations** declare their **Module Data Source Identity** and map available source state into visible render state or **Empty Module**. A new app runtime module owns pending first emission, recovery, **Unavailable Module** transitions, and fanout to dependent **Module Instances**.

**Tech Stack:** Rust 2024, `iced` 0.14 subscriptions/tasks, existing Niribar `Module` trait, current `rtk cargo test` / `rtk cargo clippy --all-targets -- -D warnings` / `rtk cargo build` verification gate.

---

## Scope

This plan migrates battery, network, and volume first because they already share the global-source-fanout shape. It creates the seam for script and **Niri Integration**, but does not migrate script, focused-window, workspaces, tray, or clock in this slice.

Out of scope for this plan:

- Migrating script process execution to a per-command **Module Data Source Identity**.
- Migrating focused-window and workspace projection out of **Niri Integration**.
- Changing D-Bus retry/backoff behavior beyond preserving current recovery when a valid source event follows an unavailable event.

## File Structure

- Create: `crates/niribar/src/modules/data_source.rs` - domain/runtime types shared between **Module Implementations** and app lifecycle.
- Modify: `crates/niribar/src/modules.rs` - exports data-source types, makes **Empty Module** explicit, adds small mapping hooks to the `Module` trait.
- Modify: `crates/niribar/src/modules/battery.rs` - declares battery source identity and maps `BatteryReading`.
- Modify: `crates/niribar/src/modules/network.rs` - declares network source identity and maps `NetworkReading`.
- Modify: `crates/niribar/src/modules/volume.rs` - declares volume source identity and maps `VolumeReading`.
- Create: `crates/niribar/src/app/module_data_sources.rs` - runtime lifecycle state machine and fanout.
- Modify: `crates/niribar/src/app.rs` - wires `module_data_sources` into `App`.
- Modify: `crates/niribar/src/app/module_runtime.rs` - exposes source-dependent **Module Instances** and applies lifecycle outcomes.
- Modify: `crates/niribar/src/app/module_events.rs` - routes battery/network/volume events through the lifecycle module.
- Modify: `crates/niribar/src/app/module_state.rs` - keeps focused-window projection only; removes migrated global source fanout.
- Modify: `crates/niribar/src/app/subscriptions.rs` - detects battery/network/volume subscriptions by **Module Data Source Identity**.

---

## Task 1: Add Module Data Source Types And Empty Outcome

**Files:**
- Create: `crates/niribar/src/modules/data_source.rs`
- Modify: `crates/niribar/src/modules.rs`
- Modify tests in: `crates/niribar/src/modules.rs`, `crates/niribar/src/app/module_runtime.rs`

- [ ] **Step 1: Declare the new module and write the failing data-source type tests**

In `crates/niribar/src/modules.rs`, add the module declaration near the other module declarations:

```rust
pub mod data_source;
```

Add this test module to the bottom of the new `crates/niribar/src/modules/data_source.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_state_reports_its_identity() {
        assert_eq!(
            ModuleDataSourceState::Battery(BatteryReading {
                percentage: 42.0,
                is_charging: false,
            })
            .identity(),
            ModuleDataSourceIdentity::Battery
        );
        assert_eq!(
            ModuleDataSourceState::Network(NetworkReading {
                connectivity: crate::modules::network::NetworkConnectivity::Full,
                label: Some("wifi".into()),
            })
            .identity(),
            ModuleDataSourceIdentity::Network
        );
        assert_eq!(
            ModuleDataSourceState::Volume(VolumeReading {
                volume: 55.0,
                muted: false,
            })
            .identity(),
            ModuleDataSourceIdentity::Volume
        );
    }

    #[test]
    fn outcome_converts_to_existing_runtime_state() {
        assert_eq!(
            ModuleInstanceOutcome::Empty.into_module_state(),
            crate::modules::ModuleState::Empty
        );
        assert_eq!(
            ModuleInstanceOutcome::Unavailable.into_module_state(),
            crate::modules::ModuleState::Unavailable
        );
    }
}
```

- [ ] **Step 2: Run the focused failing test**

Run:

```bash
rtk cargo test modules::data_source
```

Expected: FAIL with unresolved type errors for `ModuleDataSourceState`, `BatteryReading`, `ModuleDataSourceIdentity`, `NetworkReading`, `VolumeReading`, and `ModuleInstanceOutcome`.

- [ ] **Step 3: Create `crates/niribar/src/modules/data_source.rs`**

Create the file with this content above the tests:

```rust
use super::ModuleState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModuleDataSourceIdentity {
    Battery,
    Network,
    Volume,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BatteryReading {
    pub percentage: f64,
    pub is_charging: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NetworkReading {
    pub connectivity: crate::modules::network::NetworkConnectivity,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VolumeReading {
    pub volume: f64,
    pub muted: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModuleDataSourceState {
    Battery(BatteryReading),
    Network(NetworkReading),
    Volume(VolumeReading),
}

impl ModuleDataSourceState {
    pub fn identity(&self) -> ModuleDataSourceIdentity {
        match self {
            Self::Battery(_) => ModuleDataSourceIdentity::Battery,
            Self::Network(_) => ModuleDataSourceIdentity::Network,
            Self::Volume(_) => ModuleDataSourceIdentity::Volume,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModuleInstanceOutcome {
    Visible(ModuleState),
    Empty,
    Unavailable,
}

impl ModuleInstanceOutcome {
    pub fn into_module_state(self) -> ModuleState {
        match self {
            Self::Visible(state) => state,
            Self::Empty => ModuleState::Empty,
            Self::Unavailable => ModuleState::Unavailable,
        }
    }
}
```

- [ ] **Step 4: Make Empty explicit**

Change `ModuleState` so **Empty Module** is explicit and is the default. Remove the `Loading` variant:

```rust
#[derive(Debug, Clone, Default, PartialEq)]
pub enum ModuleState {
    #[default]
    Empty,
    Unavailable,
    Battery(battery::BatteryState),
    Network(network::NetworkState),
    Volume(volume::VolumeState),
    Clock(clock::ClockState),
    Script(script::ScriptState),
    Button(button::ButtonState),
    FocusedWindow(focused_window::FocusedWindowState),
    WorkspaceOutput(String),
}
```

- [ ] **Step 5: Update tests that still expect Loading**

In `crates/niribar/src/modules.rs`, replace the existing default-state assertion with:

```rust
#[test]
fn module_state_default_is_empty() {
    assert!(matches!(ModuleState::default(), ModuleState::Empty));
}
```

In `crates/niribar/src/app/module_runtime.rs`, replace the test setup that currently inserts `ModuleState::Loading` for removed dynamic tray icons with:

```rust
app.modules
    .set_state(runtime_id.clone(), ModuleState::Empty);
```

In `crates/niribar/src/modules.rs`, update the `module_id_hashable` test setup:

```rust
let mut map = HashMap::new();
map.insert(ModuleId::new("clock"), ModuleState::Empty);
assert!(map.contains_key(&ModuleId::new("clock")));
assert!(!map.contains_key(&ModuleId::new("battery")));
```

- [ ] **Step 6: Verify Task 1**

Run:

```bash
rtk cargo test modules::data_source
rtk cargo test module_state_default_is_empty
rtk cargo test removing_dynamic_tray_icon_prunes_cached_module_state
```

Expected: all listed tests PASS.

- [ ] **Step 7: Commit Task 1**

Run:

```bash
rtk git add crates/niribar/src/modules.rs crates/niribar/src/modules/data_source.rs crates/niribar/src/app/module_runtime.rs
rtk git commit -m "refactor(modules): add module data source state types"
```

---

## Task 2: Add Module Mapping Hooks For Battery Network Volume

**Files:**
- Modify: `crates/niribar/src/modules.rs`
- Modify: `crates/niribar/src/modules/battery.rs`
- Modify: `crates/niribar/src/modules/network.rs`
- Modify: `crates/niribar/src/modules/volume.rs`

- [ ] **Step 1: Write failing mapping tests**

Add this test to `crates/niribar/src/modules/battery.rs` inside the existing `tests` module:

```rust
#[test]
fn battery_maps_source_reading_to_visible_module_state() {
    let module = BatteryModule::new(
        crate::modules::ModuleId::new("battery"),
        15,
        crate::config::StatesConfig::battery_default(),
        crate::config::IconRulesConfig::default(),
        crate::config::FormatConfig::default(),
    );

    let outcome = module.map_data_source_state(
        &crate::modules::data_source::ModuleDataSourceState::Battery(
            crate::modules::data_source::BatteryReading {
                percentage: 66.0,
                is_charging: true,
            },
        ),
    );

    assert_eq!(
        outcome,
        crate::modules::data_source::ModuleInstanceOutcome::Visible(
            crate::modules::ModuleState::Battery(BatteryState {
                percentage: 66.0,
                is_charging: true,
            })
        )
    );
}
```

Add this test to `crates/niribar/src/modules/network.rs` inside the existing `tests` module:

```rust
#[test]
fn network_maps_source_reading_to_visible_module_state() {
    let module = NetworkModule::new(
        crate::modules::ModuleId::new("network"),
        crate::config::IconRulesConfig::default(),
        crate::config::FormatConfig::default(),
    );

    let outcome = module.map_data_source_state(
        &crate::modules::data_source::ModuleDataSourceState::Network(
            crate::modules::data_source::NetworkReading {
                connectivity: NetworkConnectivity::Limited,
                label: Some("captive".into()),
            },
        ),
    );

    assert_eq!(
        outcome,
        crate::modules::data_source::ModuleInstanceOutcome::Visible(
            crate::modules::ModuleState::Network(NetworkState {
                connectivity: NetworkConnectivity::Limited,
                label: Some("captive".into()),
            })
        )
    );
}
```

Add this test to `crates/niribar/src/modules/volume.rs` inside the existing `tests` module:

```rust
#[test]
fn volume_maps_source_reading_to_visible_module_state() {
    let module = VolumeModule::new(
        crate::modules::ModuleId::new("volume"),
        5,
        crate::config::StatesConfig::volume_default(),
        crate::config::IconRulesConfig::default(),
        crate::config::FormatConfig::default(),
    );

    let outcome = module.map_data_source_state(
        &crate::modules::data_source::ModuleDataSourceState::Volume(
            crate::modules::data_source::VolumeReading {
                volume: 22.0,
                muted: false,
            },
        ),
    );

    assert_eq!(
        outcome,
        crate::modules::data_source::ModuleInstanceOutcome::Visible(
            crate::modules::ModuleState::Volume(VolumeState {
                volume: 22.0,
                muted: false,
            })
        )
    );
}
```

- [ ] **Step 2: Run the failing tests**

Run:

```bash
rtk cargo test maps_source_reading_to_visible_module_state
```

Expected: FAIL because `map_data_source_state` is not part of the `Module` trait.

- [ ] **Step 3: Add default hooks to the Module trait**

In `crates/niribar/src/modules.rs`, add these methods near `subscription()` in the `Module` trait:

```rust
fn data_source_identity(&self) -> Option<data_source::ModuleDataSourceIdentity> {
    None
}

fn map_data_source_state(
    &self,
    _state: &data_source::ModuleDataSourceState,
) -> data_source::ModuleInstanceOutcome {
    data_source::ModuleInstanceOutcome::Empty
}
```

- [ ] **Step 4: Implement battery mapping**

In `impl crate::modules::Module for BatteryModule`, add:

```rust
fn data_source_identity(&self) -> Option<crate::modules::data_source::ModuleDataSourceIdentity> {
    Some(crate::modules::data_source::ModuleDataSourceIdentity::Battery)
}

fn map_data_source_state(
    &self,
    state: &crate::modules::data_source::ModuleDataSourceState,
) -> crate::modules::data_source::ModuleInstanceOutcome {
    let crate::modules::data_source::ModuleDataSourceState::Battery(reading) = state else {
        return crate::modules::data_source::ModuleInstanceOutcome::Unavailable;
    };

    crate::modules::data_source::ModuleInstanceOutcome::Visible(
        crate::modules::ModuleState::Battery(BatteryState {
            percentage: reading.percentage,
            is_charging: reading.is_charging,
        }),
    )
}
```

- [ ] **Step 5: Implement network mapping**

In `impl crate::modules::Module for NetworkModule`, add:

```rust
fn data_source_identity(&self) -> Option<crate::modules::data_source::ModuleDataSourceIdentity> {
    Some(crate::modules::data_source::ModuleDataSourceIdentity::Network)
}

fn map_data_source_state(
    &self,
    state: &crate::modules::data_source::ModuleDataSourceState,
) -> crate::modules::data_source::ModuleInstanceOutcome {
    let crate::modules::data_source::ModuleDataSourceState::Network(reading) = state else {
        return crate::modules::data_source::ModuleInstanceOutcome::Unavailable;
    };

    crate::modules::data_source::ModuleInstanceOutcome::Visible(
        crate::modules::ModuleState::Network(NetworkState {
            connectivity: reading.connectivity.clone(),
            label: reading.label.clone(),
        }),
    )
}
```

- [ ] **Step 6: Implement volume mapping**

In `impl crate::modules::Module for VolumeModule`, add:

```rust
fn data_source_identity(&self) -> Option<crate::modules::data_source::ModuleDataSourceIdentity> {
    Some(crate::modules::data_source::ModuleDataSourceIdentity::Volume)
}

fn map_data_source_state(
    &self,
    state: &crate::modules::data_source::ModuleDataSourceState,
) -> crate::modules::data_source::ModuleInstanceOutcome {
    let crate::modules::data_source::ModuleDataSourceState::Volume(reading) = state else {
        return crate::modules::data_source::ModuleInstanceOutcome::Unavailable;
    };

    crate::modules::data_source::ModuleInstanceOutcome::Visible(
        crate::modules::ModuleState::Volume(VolumeState {
            volume: reading.volume,
            muted: reading.muted,
        }),
    )
}
```

- [ ] **Step 7: Verify Task 2**

Run:

```bash
rtk cargo test maps_source_reading_to_visible_module_state
```

Expected: PASS for battery, network, and volume mapping tests.

- [ ] **Step 8: Commit Task 2**

Run:

```bash
rtk git add crates/niribar/src/modules.rs crates/niribar/src/modules/battery.rs crates/niribar/src/modules/network.rs crates/niribar/src/modules/volume.rs
rtk git commit -m "refactor(modules): map data source state in modules"
```

---

## Task 3: Add ModuleDataSourceRuntime

**Files:**
- Create: `crates/niribar/src/app/module_data_sources.rs`
- Modify: `crates/niribar/src/app.rs`
- Modify: `crates/niribar/src/app/module_runtime.rs`

- [ ] **Step 1: Write failing runtime fanout tests**

Create `crates/niribar/src/app/module_data_sources.rs` with this test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::config::{BatteryConfig, Config, ModuleConfig, ModuleParams};
    use crate::modules::data_source::{
        BatteryReading, ModuleDataSourceIdentity, ModuleDataSourceState, ModuleInstanceOutcome,
    };
    use crate::modules::{ModuleId, ModuleState};

    fn app_with_two_battery_modules() -> App {
        let mut config = Config {
            modules: vec![
                ModuleConfig {
                    name: "battery-main".into(),
                    module_type: "battery".into(),
                    params: ModuleParams::Battery(BatteryConfig::default()),
                    actions: crate::config::ModuleActionsConfig::default(),
                },
                ModuleConfig {
                    name: "battery-side".into(),
                    module_type: "battery".into(),
                    params: ModuleParams::Battery(BatteryConfig::default()),
                    actions: crate::config::ModuleActionsConfig::default(),
                },
            ],
            ..Config::default()
        };
        config.bars[0].modules.end = vec!["battery-main".into(), "battery-side".into()];
        App::boot(config, vec![]).0
    }

    #[test]
    fn runtime_initializes_pending_sources_from_modules() {
        let app = app_with_two_battery_modules();

        assert!(app.data_sources.is_pending(&ModuleDataSourceIdentity::Battery));
    }

    #[test]
    fn available_source_state_fans_out_to_dependent_modules() {
        let app = app_with_two_battery_modules();
        let mut runtime = ModuleDataSourceRuntime::from_modules(&app.modules);

        let updates = runtime.apply_source_state(
            ModuleDataSourceState::Battery(BatteryReading {
                percentage: 88.0,
                is_charging: false,
            }),
            &app.modules,
        );

        assert_eq!(updates.len(), 2);
        assert!(updates.iter().any(|(id, outcome)| {
            id == &ModuleId::new("battery-main")
                && matches!(
                    outcome,
                    ModuleInstanceOutcome::Visible(ModuleState::Battery(state))
                    if (state.percentage - 88.0).abs() < 0.001 && !state.is_charging
                )
        }));
        assert!(updates.iter().any(|(id, outcome)| {
            id == &ModuleId::new("battery-side")
                && matches!(
                    outcome,
                    ModuleInstanceOutcome::Visible(ModuleState::Battery(state))
                    if (state.percentage - 88.0).abs() < 0.001 && !state.is_charging
                )
        }));
    }

    #[test]
    fn unavailable_source_fans_out_unavailable_outcomes() {
        let app = app_with_two_battery_modules();
        let mut runtime = ModuleDataSourceRuntime::from_modules(&app.modules);

        let updates =
            runtime.mark_unavailable(ModuleDataSourceIdentity::Battery, &app.modules);

        assert_eq!(
            updates,
            vec![
                (ModuleId::new("battery-main"), ModuleInstanceOutcome::Unavailable),
                (ModuleId::new("battery-side"), ModuleInstanceOutcome::Unavailable),
            ]
        );
    }
}
```

- [ ] **Step 2: Run the failing runtime tests**

Run:

```bash
rtk cargo test app::module_data_sources
```

Expected: FAIL because `app::module_data_sources` is not declared.

- [ ] **Step 3: Declare the app module and App field**

In `crates/niribar/src/app.rs`, add this module declaration with the other `mod` lines:

```rust
mod module_data_sources;
```

Add this field to `App`:

```rust
data_sources: module_data_sources::ModuleDataSourceRuntime,
```

In `App::boot_with_eligible_outputs`, build `ModuleRuntime` before constructing `App`:

```rust
let modules = module_runtime::ModuleRuntime::from_modules_with_actions(module_graph.modules);
let data_sources = module_data_sources::ModuleDataSourceRuntime::from_modules(&modules);
```

Then set the fields in `App`:

```rust
modules,
data_sources,
```

Remove the old inline `modules: module_runtime::ModuleRuntime::from_modules_with_actions(...)` field initializer.

- [ ] **Step 4: Add dependent lookup to ModuleRuntime**

In `crates/niribar/src/app/module_runtime.rs`, add:

```rust
pub(super) fn data_source_dependents(
    &self,
    identity: crate::modules::data_source::ModuleDataSourceIdentity,
) -> Vec<crate::modules::ModuleId> {
    self.store
        .iter()
        .filter_map(|(_, module)| {
            (module.data_source_identity() == Some(identity)).then(|| module.id())
        })
        .collect()
}

pub(super) fn has_data_source(
    &self,
    identity: crate::modules::data_source::ModuleDataSourceIdentity,
) -> bool {
    self.store
        .iter()
        .any(|(_, module)| module.data_source_identity() == Some(identity))
}
```

- [ ] **Step 5: Implement ModuleDataSourceRuntime**

Put this implementation above the test module in `crates/niribar/src/app/module_data_sources.rs`:

```rust
use std::collections::HashMap;

use crate::modules::data_source::{
    ModuleDataSourceIdentity, ModuleDataSourceState, ModuleInstanceOutcome,
};
use crate::modules::ModuleId;

#[derive(Debug, Clone, PartialEq)]
enum ModuleDataSourceStatus {
    PendingFirstEmission,
    Available(ModuleDataSourceState),
    Unavailable,
}

#[derive(Debug, Default)]
pub(super) struct ModuleDataSourceRuntime {
    sources: HashMap<ModuleDataSourceIdentity, ModuleDataSourceStatus>,
}

impl ModuleDataSourceRuntime {
    pub(super) fn from_modules(modules: &super::module_runtime::ModuleRuntime) -> Self {
        let mut runtime = Self::default();
        for identity in [
            ModuleDataSourceIdentity::Battery,
            ModuleDataSourceIdentity::Network,
            ModuleDataSourceIdentity::Volume,
        ] {
            if modules.has_data_source(identity) {
                runtime
                    .sources
                    .insert(identity, ModuleDataSourceStatus::PendingFirstEmission);
            }
        }
        runtime
    }

    #[cfg(test)]
    pub(super) fn is_pending(&self, identity: &ModuleDataSourceIdentity) -> bool {
        matches!(
            self.sources.get(identity),
            Some(ModuleDataSourceStatus::PendingFirstEmission)
        )
    }

    pub(super) fn apply_source_state(
        &mut self,
        state: ModuleDataSourceState,
        modules: &super::module_runtime::ModuleRuntime,
    ) -> Vec<(ModuleId, ModuleInstanceOutcome)> {
        let identity = state.identity();
        self.sources
            .insert(identity, ModuleDataSourceStatus::Available(state.clone()));
        self.map_dependents(identity, &state, modules)
    }

    pub(super) fn mark_unavailable(
        &mut self,
        identity: ModuleDataSourceIdentity,
        modules: &super::module_runtime::ModuleRuntime,
    ) -> Vec<(ModuleId, ModuleInstanceOutcome)> {
        self.sources
            .insert(identity, ModuleDataSourceStatus::Unavailable);
        modules
            .data_source_dependents(identity)
            .into_iter()
            .map(|id| (id, ModuleInstanceOutcome::Unavailable))
            .collect()
    }

    fn map_dependents(
        &self,
        identity: ModuleDataSourceIdentity,
        state: &ModuleDataSourceState,
        modules: &super::module_runtime::ModuleRuntime,
    ) -> Vec<(ModuleId, ModuleInstanceOutcome)> {
        modules
            .data_source_dependents(identity)
            .into_iter()
            .filter_map(|id| {
                modules
                    .get(&id)
                    .map(|module| (id, module.map_data_source_state(state)))
            })
            .collect()
    }
}
```

- [ ] **Step 6: Verify Task 3**

Run:

```bash
rtk cargo test app::module_data_sources
```

Expected: PASS.

- [ ] **Step 7: Commit Task 3**

Run:

```bash
rtk git add crates/niribar/src/app.rs crates/niribar/src/app/module_data_sources.rs crates/niribar/src/app/module_runtime.rs
rtk git commit -m "refactor(app): add module data source runtime"
```

---

## Task 4: Apply Lifecycle Outcomes Through ModuleRuntime

**Files:**
- Modify: `crates/niribar/src/app/module_runtime.rs`

- [ ] **Step 1: Write the failing apply-outcomes test**

Add this test to `crates/niribar/src/app/module_runtime.rs` inside its existing test module:

```rust
#[test]
fn module_runtime_applies_data_source_outcomes() {
    let mut app = make_battery_app();

    app.modules.apply_outcomes(vec![
        (
            ModuleId::new("battery"),
            crate::modules::data_source::ModuleInstanceOutcome::Visible(
                ModuleState::Battery(crate::modules::battery::BatteryState {
                    percentage: 91.0,
                    is_charging: false,
                }),
            ),
        ),
    ]);

    match app.modules.state(&ModuleId::new("battery")) {
        Some(ModuleState::Battery(state)) => {
            assert!((state.percentage - 91.0).abs() < 0.001);
            assert!(!state.is_charging);
        }
        other => panic!("expected battery state, got {other:?}"),
    }

    app.modules.apply_outcomes(vec![(
        ModuleId::new("battery"),
        crate::modules::data_source::ModuleInstanceOutcome::Unavailable,
    )]);

    assert!(matches!(
        app.modules.state(&ModuleId::new("battery")),
        Some(ModuleState::Unavailable)
    ));
}
```

- [ ] **Step 2: Run the failing test**

Run:

```bash
rtk cargo test module_runtime_applies_data_source_outcomes
```

Expected: FAIL because `ModuleRuntime::apply_outcomes` does not exist.

- [ ] **Step 3: Implement outcome application**

In `crates/niribar/src/app/module_runtime.rs`, add:

```rust
pub(super) fn apply_outcomes(
    &mut self,
    outcomes: Vec<(
        ModuleId,
        crate::modules::data_source::ModuleInstanceOutcome,
    )>,
) {
    for (id, outcome) in outcomes {
        self.set_state(id, outcome.into_module_state());
    }
}
```

- [ ] **Step 4: Verify Task 4**

Run:

```bash
rtk cargo test module_runtime_applies_data_source_outcomes
```

Expected: PASS.

- [ ] **Step 5: Commit Task 4**

Run:

```bash
rtk git add crates/niribar/src/app/module_runtime.rs
rtk git commit -m "refactor(app): apply module data source outcomes"
```

---

## Task 5: Route Battery Network Volume Events Through Lifecycle

**Files:**
- Modify: `crates/niribar/src/app/module_events.rs`
- Modify: `crates/niribar/src/app/module_state.rs`
- Modify tests in: `crates/niribar/src/app/module_runtime.rs`

- [ ] **Step 1: Add recovery test for runtime-owned lifecycle**

Add this test to `crates/niribar/src/app/module_runtime.rs` inside the existing test module:

```rust
#[test]
fn battery_source_recovers_after_unavailable_event() {
    let mut app = make_battery_app();

    let _ = app.update(Message::Modules(ModuleRuntimeEvent::Battery(
        BatteryEvent::Unavailable,
    )));
    assert!(matches!(
        app.modules.state(&ModuleId::new("battery")),
        Some(ModuleState::Unavailable)
    ));

    let _ = app.update(Message::Modules(ModuleRuntimeEvent::Battery(
        BatteryEvent::StateChanged {
            percentage: 74.0,
            is_charging: true,
        },
    )));

    match app.modules.state(&ModuleId::new("battery")) {
        Some(ModuleState::Battery(state)) => {
            assert!((state.percentage - 74.0).abs() < 0.001);
            assert!(state.is_charging);
        }
        other => panic!("expected recovered battery state, got {other:?}"),
    }
}
```

- [ ] **Step 2: Run the existing and new module event tests**

Run:

```bash
rtk cargo test battery_state_changed_updates_module
rtk cargo test battery_unavailable_marks_module_unavailable
rtk cargo test battery_source_recovers_after_unavailable_event
rtk cargo test network_state_changed_updates_module
rtk cargo test volume_state_changed_updates_module
```

Expected: existing tests PASS before implementation; the new recovery test may already pass by current direct state writes, so keep it as a regression test for the migration.

- [ ] **Step 3: Add App helpers for lifecycle routing**

In `crates/niribar/src/app/module_events.rs`, add these helpers inside the existing `impl App` block:

```rust
fn apply_module_data_source_state(
    &mut self,
    state: crate::modules::data_source::ModuleDataSourceState,
) {
    let outcomes = self
        .data_sources
        .apply_source_state(state, &self.modules);
    self.modules.apply_outcomes(outcomes);
}

fn apply_module_data_source_unavailable(
    &mut self,
    identity: crate::modules::data_source::ModuleDataSourceIdentity,
) {
    let outcomes = self
        .data_sources
        .mark_unavailable(identity, &self.modules);
    self.modules.apply_outcomes(outcomes);
}
```

- [ ] **Step 4: Replace battery event handling**

Replace `handle_battery_event` in `crates/niribar/src/app/module_events.rs` with:

```rust
pub(super) fn handle_battery_event(&mut self, event: BatteryEvent) -> Task<Message> {
    match event {
        BatteryEvent::StateChanged {
            percentage,
            is_charging,
        } => self.apply_module_data_source_state(
            crate::modules::data_source::ModuleDataSourceState::Battery(
                crate::modules::data_source::BatteryReading {
                    percentage,
                    is_charging,
                },
            ),
        ),
        BatteryEvent::Unavailable => self.apply_module_data_source_unavailable(
            crate::modules::data_source::ModuleDataSourceIdentity::Battery,
        ),
    }
    Task::none()
}
```

- [ ] **Step 5: Replace network event handling**

Replace `handle_network_event` with:

```rust
pub(super) fn handle_network_event(&mut self, event: NetworkEvent) -> Task<Message> {
    match event {
        NetworkEvent::StateChanged {
            connectivity,
            label,
        } => self.apply_module_data_source_state(
            crate::modules::data_source::ModuleDataSourceState::Network(
                crate::modules::data_source::NetworkReading {
                    connectivity,
                    label,
                },
            ),
        ),
        NetworkEvent::Unavailable => self.apply_module_data_source_unavailable(
            crate::modules::data_source::ModuleDataSourceIdentity::Network,
        ),
    }
    Task::none()
}
```

- [ ] **Step 6: Replace volume event handling**

Replace `handle_volume_event` with:

```rust
pub(super) fn handle_volume_event(&mut self, event: VolumeEvent) -> Task<Message> {
    match event {
        VolumeEvent::StateChanged { volume, muted } => self.apply_module_data_source_state(
            crate::modules::data_source::ModuleDataSourceState::Volume(
                crate::modules::data_source::VolumeReading { volume, muted },
            ),
        ),
        VolumeEvent::Unavailable => self.apply_module_data_source_unavailable(
            crate::modules::data_source::ModuleDataSourceIdentity::Volume,
        ),
    }
    Task::none()
}
```

- [ ] **Step 7: Remove migrated fanout helpers from module_state.rs**

In `crates/niribar/src/app/module_state.rs`, remove these methods:

```rust
pub(super) fn push_battery_state(&mut self, percentage: f64, is_charging: bool) { ... }
pub(super) fn push_network_state(
    &mut self,
    connectivity: NetworkConnectivity,
    label: Option<String>,
) { ... }
pub(super) fn push_volume_state(&mut self, volume: f64, muted: bool) { ... }
```

Keep `push_focused_window_state` and its helper because **Niri Integration** is out of scope for this slice. Remove imports that become unused:

```rust
use crate::modules::battery::BatteryState;
use crate::modules::network::{NetworkConnectivity, NetworkState};
use crate::modules::volume::VolumeState;
```

- [ ] **Step 8: Remove obsolete unavailable helper if unused**

In `crates/niribar/src/app/module_events.rs`, remove this helper if `rtk rg "mark_module_type_unavailable" crates/niribar/src/app` shows no callers:

```rust
fn mark_module_type_unavailable(&mut self, module_type: ModuleKind) { ... }
```

If removing it makes `ModuleKind` unused in `module_events.rs`, remove `ModuleKind` from the `use crate::modules::{...};` list.

- [ ] **Step 9: Verify Task 5**

Run:

```bash
rtk cargo test battery_state_changed_updates_module
rtk cargo test battery_unavailable_marks_module_unavailable
rtk cargo test battery_source_recovers_after_unavailable_event
rtk cargo test network_state_changed_updates_module
rtk cargo test network_unavailable_marks_module_unavailable
rtk cargo test volume_state_changed_updates_module
rtk cargo test volume_unavailable_marks_module_unavailable
```

Expected: all listed tests PASS.

- [ ] **Step 10: Commit Task 5**

Run:

```bash
rtk git add crates/niribar/src/app/module_events.rs crates/niribar/src/app/module_state.rs crates/niribar/src/app/module_runtime.rs
rtk git commit -m "refactor(app): route global module sources through lifecycle"
```

---

## Task 6: Use Data Source Identity For Source Subscriptions

**Files:**
- Modify: `crates/niribar/src/app/subscriptions.rs`

- [ ] **Step 1: Write the failing subscription identity tests**

In `crates/niribar/src/app/subscriptions.rs`, add this test inside the existing `tests` module:

```rust
#[test]
fn data_source_identity_detects_global_source_modules() {
    let app = app_with_modules(
        vec![module(
            "battery",
            ModuleParams::Battery(crate::config::BatteryConfig::default()),
        )],
        vec!["battery"],
    );

    assert!(app
        .modules
        .has_data_source(crate::modules::data_source::ModuleDataSourceIdentity::Battery));
    assert!(!app
        .modules
        .has_data_source(crate::modules::data_source::ModuleDataSourceIdentity::Network));
}
```

- [ ] **Step 2: Run the test**

Run:

```bash
rtk cargo test data_source_identity_detects_global_source_modules
```

Expected: PASS if Task 3's `has_data_source` is already available. If it fails due to method visibility, make `has_data_source` `pub(super)` in `crates/niribar/src/app/module_runtime.rs`.

- [ ] **Step 3: Replace kind checks for battery network volume**

In `crates/niribar/src/app/subscriptions.rs`, replace:

```rust
let has_battery = self.has_module_kind(ModuleKind::Battery);
let has_network = self.has_module_kind(ModuleKind::Network);
let has_volume = self.has_module_kind(ModuleKind::Volume);
```

with:

```rust
let has_battery = self
    .modules
    .has_data_source(crate::modules::data_source::ModuleDataSourceIdentity::Battery);
let has_network = self
    .modules
    .has_data_source(crate::modules::data_source::ModuleDataSourceIdentity::Network);
let has_volume = self
    .modules
    .has_data_source(crate::modules::data_source::ModuleDataSourceIdentity::Volume);
```

Keep `has_module_kind` for tray unless it becomes unused. If it is only used for tray, leave it as is:

```rust
let has_tray = self.has_module_kind(ModuleKind::Tray);
```

- [ ] **Step 4: Verify Task 6**

Run:

```bash
rtk cargo test app::subscriptions
```

Expected: PASS.

- [ ] **Step 5: Commit Task 6**

Run:

```bash
rtk git add crates/niribar/src/app/subscriptions.rs crates/niribar/src/app/module_runtime.rs
rtk git commit -m "refactor(app): subscribe by module data source identity"
```

---

## Task 7: Verify Loading Is Gone From Runtime Vocabulary

**Files:**
- Modify: `crates/niribar/src/modules.rs`
- Modify tests anywhere that still mention `ModuleState::Loading`

- [ ] **Step 1: Scan for remaining Loading references**

Run:

```bash
rtk rg "ModuleState::Loading|Loading" src
```

Expected before cleanup: no production references to `ModuleState::Loading`. If matches remain from comments or tests, update them in the next steps.

- [ ] **Step 2: Replace leftover Loading comments with Empty or pending lifecycle wording**

If `crates/niribar/src/modules.rs` still has a comment describing a startup render state, replace it with:

```rust
/// Module has no visible content. Used for available-but-empty Module outcomes
/// and as the no-state render fallback while first emission is pending inside
/// the Module Data Source runtime lifecycle.
```

- [ ] **Step 3: Verify no Loading vocabulary remains in code**

Run:

```bash
rtk rg "ModuleState::Loading|Loading" src
```

Expected: no matches in `src/`.

- [ ] **Step 4: Run the module tests**

Run:

```bash
rtk cargo test modules
```

Expected: PASS.

- [ ] **Step 5: Commit Task 7 if cleanup changed files**

If Task 7 changed files, run:

```bash
rtk git add crates/niribar/src/modules.rs crates/niribar/src/app/module_runtime.rs
rtk git commit -m "refactor(modules): remove loading render state"
```

If Task 7 changed no files, do not create an empty commit.

---

## Task 8: Full Verification And Plan Closure

**Files:**
- Modify: none unless verification finds a compile or lint issue in files changed by earlier tasks.

- [ ] **Step 1: Run focused lifecycle tests**

Run:

```bash
rtk cargo test app::module_data_sources
rtk cargo test maps_source_reading_to_visible_module_state
rtk cargo test battery_source_recovers_after_unavailable_event
```

Expected: all PASS.

- [ ] **Step 2: Run full verification gate**

Run:

```bash
rtk cargo test
rtk cargo clippy --all-targets -- -D warnings
rtk cargo build
```

Expected: all PASS.

- [ ] **Step 3: Check that migrated paths no longer bypass lifecycle**

Run:

```bash
rtk rg "push_battery_state|push_network_state|push_volume_state|mark_module_type_unavailable" crates/niribar/src/app
```

Expected: no matches.

Run:

```bash
rtk rg "ModuleState::Loading|Loading" src
```

Expected: no matches.

- [ ] **Step 4: Commit final verification fixes if any**

If Task 8 required small compile/lint fixes, commit them:

```bash
rtk git add src
rtk git commit -m "chore: finish module data source lifecycle migration"
```

If Task 8 required no file changes, do not create an empty commit.

---

## Follow-Up Plans

After this plan lands, write separate plans for these migrations:

1. Script **Module Data Source Identity** keyed by configured command identity.
2. **Niri Integration** projection for focused-window and workspace-backed **Module Instances**.
3. First-emission timeout policy if the current source subscriptions do not already emit unavailable within the runtime contract.
