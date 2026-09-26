# Code Quality Refactors Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Improve niribar maintainability, extensibility, performance, and command-execution security without changing user-visible behavior.

**Architecture:** Consolidate repeated module metadata, rendering, parsing, subscription, and runtime-origin logic behind small internal APIs. Keep behavior-preserving refactors separate from new security/config features so regressions are easy to isolate.

**Tech Stack:** Rust 2024, iced 0.14, iced_layershell, tokio, zbus, kdl, niri-ipc.

---

## File Structure

- Modify `src/modules.rs`: extend module descriptors, remove duplicate config-name mapping where practical, make constructors fallible in a later task.
- Modify `src/app.rs`: delegate module construction to a focused builder.
- Create `src/app/module_graph.rs`: owns ordered module resolution, synthesized defaults, constructor dispatch, and action collection.
- Modify `src/app/subscriptions.rs`: replace hard-coded module-kind capability checks with descriptor/runtime capability queries.
- Modify `src/modules/view.rs`: add reusable template/view helpers for modules with placeholder values.
- Modify module files: `src/modules/{battery,volume,network,clock,script,button,focused_window}.rs` to use shared formatting helpers.
- Modify `src/config.rs`: introduce lower-allocation template value types.
- Modify `src/config/parse/modules/{mod,battery,volume,network,clock,script,button,focused_window,tray,workspaces}.rs`: move parser dispatch and common state/icons/format/tooltip patterns behind helpers.
- Modify `src/app/module_store.rs`, `src/app/module_runtime.rs`, `src/app/dynamic_modules.rs`, `src/app/tray.rs`, `src/app/niri.rs`: add typed runtime origins and batched mutations for dynamic modules.
- Modify `src/command.rs`, `src/config.rs`, `src/config/parse/actions.rs`, `src/config/parse/modules/{button,script}.rs`: add argv-capable command specs and command policy plumbing.

## Task 1: Descriptor Single Source Of Truth

**Files:**
- Modify: `src/modules.rs`
- Modify: `src/config.rs`
- Modify: `src/config/parse/mod.rs`
- Test: `src/modules.rs`
- Test: `src/config/parse/tests.rs`

- [ ] **Step 1: Write failing tests for descriptor-driven names**

Add tests in `src/modules.rs`:

```rust
#[test]
fn module_kind_names_are_descriptor_backed_for_config_modules() {
    for descriptor in MODULE_DESCRIPTORS {
        assert_eq!(descriptor.kind.config_name(), Some(descriptor.config_name));
        assert_eq!(ModuleKind::from_config_name(descriptor.config_name), Some(descriptor.kind));
    }
}

#[test]
fn runtime_only_kinds_are_not_config_names() {
    assert_eq!(ModuleKind::WorkspaceItem.config_name(), None);
    assert_eq!(ModuleKind::TrayIcon.config_name(), None);
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
rtk cargo test module_kind_names_are_descriptor_backed_for_config_modules runtime_only_kinds_are_not_config_names
```

Expected: fail because `ModuleKind::config_name` does not exist.

- [ ] **Step 3: Implement descriptor-backed lookup helpers**

In `src/modules.rs`, add:

```rust
impl ModuleKind {
    pub(crate) fn config_name(self) -> Option<&'static str> {
        MODULE_DESCRIPTORS
            .iter()
            .find(|descriptor| descriptor.kind == self)
            .map(|descriptor| descriptor.config_name)
    }

    pub(crate) fn from_config_name(s: &str) -> Option<Self> {
        MODULE_DESCRIPTORS
            .iter()
            .find(|descriptor| descriptor.config_name == s)
            .map(|descriptor| descriptor.kind)
    }
}
```

Then change existing `as_str` callers that only handle config-facing modules to use `config_name().unwrap_or_else(|| self.runtime_name())`. Keep a small `runtime_name()` match for `WorkspaceItem` and `TrayIcon`.

- [ ] **Step 4: Update known-module checks**

In `src/config.rs`, change `is_known_config_module_type` to:

```rust
fn is_known_config_module_type(module_type: &str) -> bool {
    crate::modules::ModuleKind::from_config_name(module_type).is_some()
}
```

- [ ] **Step 5: Run focused tests**

Run:

```bash
rtk cargo test modules::tests::module_kind_round_trips_known_config_names config::parse
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
rtk git add src/modules.rs src/config.rs src/config/parse/mod.rs
rtk git commit -m "refactor: make module descriptor names authoritative"
```

## Task 2: Extract Module Graph Builder

**Files:**
- Create: `src/app/module_graph.rs`
- Modify: `src/app.rs`
- Modify: `src/app/module_runtime.rs`
- Test: `src/app/module_graph.rs`

- [ ] **Step 1: Create failing tests for module graph behavior**

Create `src/app/module_graph.rs` with tests first. The first test should assert that undeclared built-ins are synthesized when referenced by a bar; the second should assert first zone wins when a module appears in multiple bars.

```rust
#[cfg(test)]
mod tests {
    use crate::config::{Config, Edge};
    use crate::modules::{ModuleKind, Zone};

    #[test]
    fn referenced_builtin_is_synthesized() {
        let config = Config::default();
        let graph = super::build_module_graph(&config).unwrap();
        assert!(graph.modules.iter().any(|(_, module, _)| module.kind() == ModuleKind::Clock));
    }

    #[test]
    fn duplicate_module_name_uses_first_zone() {
        let mut config = Config::default();
        config.bars[0].modules.top = vec!["clock".into()];
        config.bars[0].modules.bottom = vec!["clock".into()];
        config.bars[0].edge = Edge::Left;

        let graph = super::build_module_graph(&config).unwrap();
        let clock = graph.modules.iter().find(|(_, m, _)| m.kind() == ModuleKind::Clock).unwrap();
        assert_eq!(clock.0, Zone::Top);
    }
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
rtk cargo test app::module_graph
```

Expected: fail because `module_graph` is not wired and `build_module_graph` does not exist.

- [ ] **Step 3: Add module graph API**

In `src/app.rs`, add `mod module_graph;`.

In `src/app/module_graph.rs`, implement:

```rust
use std::collections::{HashMap, HashSet};

use anyhow::Context;

use crate::config::{Config, ModuleActionsConfig, ModuleConfig};
use crate::modules::{MODULE_DESCRIPTORS, Module, ModuleDescriptor, Zone, build_registry};

pub(super) struct ModuleGraph {
    pub(super) modules: Vec<(Zone, Box<dyn Module>, ModuleActionsConfig)>,
}

pub(super) fn build_module_graph(config: &Config) -> anyhow::Result<ModuleGraph> {
    let registry = build_registry();
    let descriptors: HashMap<&str, &ModuleDescriptor> =
        MODULE_DESCRIPTORS.iter().map(|d| (d.config_name, d)).collect();
    let module_map: HashMap<&str, &ModuleConfig> =
        config.modules.iter().map(|mc| (mc.name.as_str(), mc)).collect();

    let mut ordered = Vec::new();
    let mut seen_names = HashSet::new();
    for bar in &config.bars {
        for (zone, names) in [
            (Zone::Top, &bar.modules.top),
            (Zone::Center, &bar.modules.center),
            (Zone::Bottom, &bar.modules.bottom),
        ] {
            for name in names {
                if seen_names.insert(name.clone()) {
                    ordered.push((zone.clone(), name.clone()));
                }
            }
        }
    }

    let mut modules = Vec::new();
    for (zone, name) in &ordered {
        let synthesized;
        let (mc, descriptor, ctor) = if let Some(&mc) = module_map.get(name.as_str()) {
            let descriptor = descriptors
                .get(mc.module_type.as_str())
                .copied()
                .with_context(|| format!("unknown module type {:?}", mc.module_type))?;
            let ctor = registry
                .get(mc.module_type.as_str())
                .copied()
                .with_context(|| format!("missing constructor for {:?}", mc.module_type))?;
            (mc, descriptor, ctor)
        } else if let Some(descriptor) = descriptors.get(name.as_str()).copied() {
            let Some(params) = descriptor.synthesized_params() else {
                continue;
            };
            synthesized = ModuleConfig {
                name: name.clone(),
                module_type: descriptor.config_name.to_string(),
                params,
                actions: ModuleActionsConfig::default(),
            };
            (&synthesized, descriptor, descriptor.constructor)
        } else {
            continue;
        };

        let module = ctor(mc);
        debug_assert_eq!(module.kind(), descriptor.kind);
        modules.push((zone.clone(), module, mc.actions.clone()));
    }

    Ok(ModuleGraph { modules })
}
```

- [ ] **Step 4: Use builder from `App::boot`**

In `src/app.rs`, replace local module registry/module construction code with:

```rust
let module_graph = module_graph::build_module_graph(&config)
    .expect("validated config should build a module graph");
```

Then initialize runtime with:

```rust
modules: module_runtime::ModuleRuntime::from_modules_with_actions(module_graph.modules),
```

- [ ] **Step 5: Run focused tests**

Run:

```bash
rtk cargo test app::module_graph app::tests::boot_with_config_stores_config
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
rtk git add src/app.rs src/app/module_graph.rs src/app/module_runtime.rs
rtk git commit -m "refactor: extract module graph construction"
```

## Task 3: Shared Template Display Values

**Files:**
- Modify: `src/config.rs`
- Modify: `src/modules/view.rs`
- Modify: `src/modules/{battery,volume,network,clock,script,button,focused_window}.rs`
- Test: module tests in the same files

- [ ] **Step 1: Add tests for borrowed template values**

In `src/config.rs`, add:

```rust
#[test]
fn format_template_renders_borrowed_pairs() {
    let template = TextTemplate::parse("{icon} {value}", &["icon", "value"], "test.format").unwrap();
    let values = [
        ("icon", std::borrow::Cow::Borrowed("I")),
        ("value", std::borrow::Cow::Owned("42".to_string())),
    ];
    assert_eq!(template.render_cow_pairs(&values), "I 42");
}
```

- [ ] **Step 2: Run test and verify failure**

Run:

```bash
rtk cargo test format_template_renders_borrowed_pairs
```

Expected: fail because `render_cow_pairs` does not exist.

- [ ] **Step 3: Add borrowed value API**

In `src/config.rs`, add:

```rust
pub type FormatValue<'a> = (&'static str, std::borrow::Cow<'a, str>);
```

Add to `TextTemplate`:

```rust
pub fn render_cow_pairs(&self, values: &[FormatValue<'_>]) -> String {
    let mut rendered = String::new();
    for part in &self.parts {
        match part {
            TemplatePart::Literal(value) => rendered.push_str(value),
            TemplatePart::Placeholder(name) => {
                if let Some((_, value)) = values.iter().find(|(key, _)| key == &name.as_str()) {
                    rendered.push_str(value);
                }
            }
        }
    }
    rendered
}
```

Add to `FormatTemplate`:

```rust
pub fn render_styled_cow_segments_with_default(
    &self,
    values: &[FormatValue<'_>],
    state: Option<&str>,
    default_style: &Style,
) -> Vec<(String, ResolvedStyle)> {
    self.segments
        .iter()
        .map(|s| {
            (
                s.template.render_cow_pairs(values),
                s.styles.style_for(state).merge_over(*default_style).resolve(),
            )
        })
        .collect()
}
```

- [ ] **Step 4: Add shared helper in `src/modules/view.rs`**

Add:

```rust
pub(super) fn render_format_segments<'a>(
    format: &crate::config::FormatConfig,
    mode: crate::config::FormatMode,
    values: &[crate::config::FormatValue<'a>],
    state: Option<&str>,
    default_style: &crate::config::Style,
) -> Option<Vec<(String, crate::config::ResolvedStyle)>> {
    format
        .template_for(mode)
        .map(|template| template.render_styled_cow_segments_with_default(values, state, default_style))
}
```

- [ ] **Step 5: Convert one module at a time**

For each module listed in this task, replace local `template.render_styled_segments_with_default(&[(..., String)], ...)` calls with `render_format_segments(...)`. Start with `network`, then `volume`, then `battery`, then clock/script/button/focused-window.

- [ ] **Step 6: Run module tests after each conversion**

Run after each file:

```bash
rtk cargo test modules::network
rtk cargo test modules::volume
rtk cargo test modules::battery
rtk cargo test modules::clock modules::script modules::button modules::focused_window
```

Expected: pass after each conversion.

- [ ] **Step 7: Commit**

```bash
rtk git add src/config.rs src/modules/view.rs src/modules/battery.rs src/modules/volume.rs src/modules/network.rs src/modules/clock.rs src/modules/script.rs src/modules/button.rs src/modules/focused_window.rs
rtk git commit -m "refactor: share module template rendering"
```

## Task 4: Descriptor-Driven Parser Dispatch And Common Parse Helpers

**Files:**
- Modify: `src/modules.rs`
- Modify: `src/config/parse/modules/mod.rs`
- Modify: `src/config/parse/modules/{battery,volume,network,clock,focused_window,tray,workspaces}.rs`
- Test: `src/config/parse/tests.rs`

- [ ] **Step 1: Add test that every config descriptor has parser coverage**

In `src/config/parse/modules/mod.rs`, add:

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn every_config_descriptor_has_parser() {
        for descriptor in crate::modules::MODULE_DESCRIPTORS {
            assert!(
                super::parser_for(descriptor.kind).is_some(),
                "missing parser for {:?}",
                descriptor.kind
            );
        }
    }
}
```

- [ ] **Step 2: Run and verify failure**

Run:

```bash
rtk cargo test every_config_descriptor_has_parser
```

Expected: fail because `parser_for` does not exist.

- [ ] **Step 3: Add parser lookup**

In `src/config/parse/modules/mod.rs`, add:

```rust
type ModuleParamParser = fn(&KdlNode, usize, &ModuleActionsConfig) -> anyhow::Result<ModuleParams>;

pub(super) fn parser_for(kind: ModuleKind) -> Option<ModuleParamParser> {
    match kind {
        ModuleKind::Battery => Some(|node, idx, _| battery::parse(node, idx).map(ModuleParams::Battery)),
        ModuleKind::Clock => Some(|node, idx, _| clock::parse(node, idx).map(ModuleParams::Clock)),
        ModuleKind::Network => Some(|node, idx, _| network::parse(node, idx).map(ModuleParams::Network)),
        ModuleKind::Volume => Some(|node, idx, _| volume::parse(node, idx).map(ModuleParams::Volume)),
        ModuleKind::Script => Some(|node, idx, _| script::parse(node, idx).map(ModuleParams::Script)),
        ModuleKind::Button => Some(|node, idx, actions| button::parse(node, idx, actions).map(ModuleParams::Button)),
        ModuleKind::Workspaces => Some(|node, idx, _| workspaces::parse(node, idx).map(ModuleParams::Workspaces)),
        ModuleKind::FocusedWindow => Some(|node, idx, _| focused_window::parse(node, idx).map(ModuleParams::FocusedWindow)),
        ModuleKind::Tray => Some(|node, idx, _| tray::parse(node, idx).map(ModuleParams::Tray)),
        ModuleKind::WorkspaceItem | ModuleKind::TrayIcon => None,
    }
}
```

Change `dispatch` to:

```rust
let Some(parser) = parser_for(kind) else {
    anyhow::bail!(
        "{}: internal error: {:?} is runtime-only and cannot appear in config",
        error::module_path(idx, kind.runtime_name()),
        kind
    );
};
parser(node, idx, actions)
```

- [ ] **Step 4: Extract format/tooltip helper**

In `src/config/parse/modules/formats.rs`, add:

```rust
pub(super) fn parse_format_and_tooltip(
    children: &KdlDocument,
    module_path: &str,
    placeholders: &[&str],
    allowed_states: &[&str],
) -> anyhow::Result<(FormatConfig, TooltipSetting)> {
    Ok((
        parse_optional(children, module_path, placeholders, allowed_states)?,
        parse_tooltip_field(children, module_path, placeholders, allowed_states)?,
    ))
}
```

Use it in battery, volume, network, clock, focused-window, button, script, and workspaces.

- [ ] **Step 5: Run parser suite**

Run:

```bash
rtk cargo test config::parse
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
rtk git add src/modules.rs src/config/parse/modules
rtk git commit -m "refactor: centralize module parser dispatch"
```

## Task 5: Subscription Capabilities

**Files:**
- Modify: `src/modules.rs`
- Modify: `src/app/subscriptions.rs`
- Test: `src/app/subscriptions.rs`

- [ ] **Step 1: Add descriptor capability tests**

In `src/app/subscriptions.rs`, add:

```rust
#[test]
fn descriptor_capabilities_drive_niri_subscription() {
    let app = app_with_modules(
        vec![module("focused-window", ModuleParams::None)],
        vec!["focused-window"],
    );
    assert!(app.needs_niri_subscription());
}

#[test]
fn descriptor_capabilities_drive_keyboard_focus_subscription() {
    let app = app_with_modules(
        vec![module("button", ModuleParams::Button(crate::config::ButtonConfig {
            label: "x".into(),
            icon: "x".into(),
            on_click: "true".into(),
            format: crate::config::FormatConfig::default(),
            tooltip: crate::config::TooltipSetting::Default,
        }))],
        vec!["button"],
    );
    assert!(app.needs_keyboard_focus_subscription());
}
```

- [ ] **Step 2: Add capability struct**

In `src/modules.rs`, add:

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ModuleCapabilities {
    pub needs_niri: bool,
    pub needs_keyboard_focus: bool,
    pub needs_system_dbus: bool,
    pub needs_session_dbus: bool,
    pub needs_wireplumber: bool,
}
```

Add `pub capabilities: ModuleCapabilities` to `ModuleDescriptor`.

- [ ] **Step 3: Populate capabilities in descriptor macro**

Extend descriptor entries with capabilities. Use:

```rust
ModuleCapabilities { needs_system_dbus: true, ..ModuleCapabilities::EMPTY }
```

or, if `const Default` is awkward, add associated const:

```rust
impl ModuleCapabilities {
    pub(crate) const NONE: Self = Self {
        needs_niri: false,
        needs_keyboard_focus: false,
        needs_system_dbus: false,
        needs_session_dbus: false,
        needs_wireplumber: false,
    };
}
```

Set battery/network to system D-Bus, volume to wireplumber and keyboard focus, tray to session D-Bus and keyboard focus, workspaces to niri and keyboard focus, focused-window to niri, button to keyboard focus.

- [ ] **Step 4: Replace hard-coded checks**

In `src/app/subscriptions.rs`, add:

```rust
fn has_capability(&self, predicate: impl Fn(crate::modules::ModuleCapabilities) -> bool) -> bool {
    self.modules
        .iter()
        .filter_map(|(_, module)| crate::modules::descriptor_for_kind(module.kind()))
        .any(|descriptor| predicate(descriptor.capabilities))
}
```

Then rewrite `needs_niri_subscription`, `needs_keyboard_focus_subscription`, and D-Bus booleans to use capabilities.

- [ ] **Step 5: Run subscription tests**

Run:

```bash
rtk cargo test app::subscriptions
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
rtk git add src/modules.rs src/app/subscriptions.rs
rtk git commit -m "refactor: drive subscriptions from module capabilities"
```

## Task 6: Typed Runtime Module Origins And Batched Store Mutations

**Files:**
- Modify: `src/app/module_store.rs`
- Modify: `src/app/module_runtime.rs`
- Modify: `src/app/dynamic_modules.rs`
- Modify: `src/app/tray.rs`
- Modify: `src/app/niri.rs`
- Test: `src/app/module_runtime.rs`

- [ ] **Step 1: Add origin type and tests**

In `src/app/module_store.rs`, add:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleOrigin {
    Configured,
    ExpandedFrom(crate::modules::ModuleId),
}
```

Add tests asserting tray icons/workspace items are removable by origin rather than string prefix.

- [ ] **Step 2: Store origin alongside modules**

Change:

```rust
ordered: Vec<(Zone, Box<dyn Module>)>
```

to:

```rust
ordered: Vec<ModuleStoreEntry>
```

with:

```rust
pub struct ModuleStoreEntry {
    pub zone: Zone,
    pub module: Box<dyn Module>,
    pub origin: ModuleOrigin,
}
```

Preserve existing iterators initially by adding compatibility methods returning `(&Zone, &Box<dyn Module>)` where needed, then migrate callers gradually.

- [ ] **Step 3: Add batch mutation API**

Add:

```rust
pub fn replace_dynamic_from(
    &mut self,
    parent: &crate::modules::ModuleId,
    entries: Vec<ModuleStoreEntry>,
) {
    self.ordered.retain(|entry| entry.origin != ModuleOrigin::ExpandedFrom(parent.clone()));
    self.ordered.extend(entries);
    self.rebuild_indexes();
}
```

- [ ] **Step 4: Update tray/workspace expansion**

Replace string-prefix cleanup and repeated insert/push calls with `replace_dynamic_from`. The parent origin should be the configured tray/workspaces module ID, not the textual prefix.

- [ ] **Step 5: Run dynamic module tests**

Run:

```bash
rtk cargo test app::module_runtime app::tray app::niri
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
rtk git add src/app/module_store.rs src/app/module_runtime.rs src/app/dynamic_modules.rs src/app/tray.rs src/app/niri.rs
rtk git commit -m "refactor: track dynamic module origins explicitly"
```

## Task 7: Safer Command Specs And Policy

**Files:**
- Modify: `src/command.rs`
- Modify: `src/config.rs`
- Modify: `src/config/parse/actions.rs`
- Modify: `src/config/parse/modules/script.rs`
- Modify: `src/config/parse/modules/button.rs`
- Modify: `src/modules/script.rs`
- Modify: `src/modules/button.rs`
- Test: `src/command.rs`, parser tests

- [x] **Step 1: Add command config enum tests**

In `src/command.rs`, add:

```rust
#[test]
fn argv_command_spec_keeps_argument_boundaries() {
    let args = ["hello world", "$(ignored)"];
    let spec = CommandSpec::Argv { program: "printf", args: &args };
    assert_eq!(format!("{spec:?}").contains("Argv"), true);
}
```

In parser tests, add KDL cases:

```kdl
module "script" {
    command {
        argv "printf" "hello"
    }
}
```

and:

```kdl
module "button" {
    label "Run"
    icon ">"
    on-click {
        argv "true"
    }
}
```

- [x] **Step 2: Add owned config command type**

In `src/config.rs`, add:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandConfig {
    Shell(String),
    Argv { program: String, args: Vec<String> },
}
```

Change script/button/action command fields from `String` to `CommandConfig` where appropriate. Keep string KDL syntax mapped to `Shell(String)` for compatibility.

- [x] **Step 3: Bridge config command into runtime command**

In `src/command.rs`, add:

```rust
impl<'a> From<&'a crate::config::CommandConfig> for CommandSpec<'a> {
    fn from(value: &'a crate::config::CommandConfig) -> Self {
        match value {
            crate::config::CommandConfig::Shell(command) => CommandSpec::Shell(command),
            crate::config::CommandConfig::Argv { program, args } => {
                let args: Vec<&str> = args.iter().map(String::as_str).collect();
                CommandSpec::Argv { program, args: Box::leak(args.into_boxed_slice()) }
            }
        }
    }
}
```

Do not keep the `Box::leak` version in final code. Instead implement call-site helpers that hold `Vec<&str>` locally while invoking the command. The temporary snippet is only a forcing function to reveal lifetime constraints during implementation.

- [x] **Step 4: Implement non-leaking helpers**

Add:

```rust
pub async fn run_config_command_capture_stdout(
    config: &crate::config::CommandConfig,
    timeout: std::time::Duration,
    max_output_bytes: usize,
) -> std::io::Result<CaptureResult> {
    match config {
        crate::config::CommandConfig::Shell(command) => {
            run_command_capture_stdout(CommandSpec::Shell(command), timeout, max_output_bytes).await
        }
        crate::config::CommandConfig::Argv { program, args } => {
            let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
            run_command_capture_stdout(
                CommandSpec::Argv { program, args: &borrowed },
                timeout,
                max_output_bytes,
            ).await
        }
    }
}
```

Add a matching `spawn_config_command_fire_and_forget`.

- [x] **Step 5: Add policy knobs**

Add to `Config`:

```rust
pub command_policy: CommandPolicy,

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandPolicy {
    pub allow_shell: bool,
    pub action_timeout_ms: u64,
}
```

Default:

```rust
CommandPolicy {
    allow_shell: true,
    action_timeout_ms: 3000,
}
```

Wire parsing so users can later disable shell commands globally. When disabled, reject `CommandConfig::Shell` during config parsing with an actionable error.

- [x] **Step 6: Run command and parser tests**

Run:

```bash
rtk cargo test command config::parse::modules::script config::parse::modules::button config::parse::actions
```

Expected: pass.

- [x] **Step 7: Commit**

```bash
rtk git add src/command.rs src/config.rs src/config/parse/actions.rs src/config/parse/modules/script.rs src/config/parse/modules/button.rs src/modules/script.rs src/modules/button.rs
rtk git commit -m "feat: support argv command configuration"
```

## Task 8: Fallible Constructors

**Files:**
- Modify: `src/modules.rs`
- Modify: `src/app/module_graph.rs`
- Test: `src/modules.rs`, `src/app/module_graph.rs`

- [x] **Step 1: Add constructor mismatch test**

In `src/modules.rs`, add:

```rust
#[test]
fn script_constructor_reports_wrong_params() {
    let mc = crate::config::ModuleConfig {
        name: "bad".into(),
        module_type: "script".into(),
        params: crate::config::ModuleParams::None,
        actions: crate::config::ModuleActionsConfig::default(),
    };
    let err = script_ctor(&mc).unwrap_err().to_string();
    assert!(err.contains("script"));
}
```

- [x] **Step 2: Change constructor type**

Change:

```rust
pub(crate) type ModuleConstructor = fn(&crate::config::ModuleConfig) -> Box<dyn Module>;
```

to:

```rust
pub(crate) type ModuleConstructor =
    fn(&crate::config::ModuleConfig) -> anyhow::Result<Box<dyn Module>>;
```

Update every constructor to return `Ok(Box::new(...))`.

- [x] **Step 3: Replace constructor panics**

Change script/button mismatches from `panic!` to:

```rust
_ => anyhow::bail!("script constructor called with non-script params for {:?}", mc.name),
```

and similarly for button.

- [x] **Step 4: Propagate errors from module graph**

In `src/app/module_graph.rs`, change:

```rust
let module = ctor(mc);
```

to:

```rust
let module = ctor(mc)
    .with_context(|| format!("failed to construct module {:?}", mc.name))?;
```

- [x] **Step 5: Run constructor tests**

Run:

```bash
rtk cargo test script_constructor_reports_wrong_params app::module_graph
```

Expected: pass.

- [x] **Step 6: Commit**

```bash
rtk git add src/modules.rs src/app/module_graph.rs
rtk git commit -m "refactor: make module constructors fallible"
```

## Task 9: Full Verification And Cleanup

**Files:**
- Modify as needed based on compiler/test findings.

- [x] **Step 1: Format**

Run:

```bash
rtk cargo fmt
```

Expected: no errors.

- [x] **Step 2: Run full test suite**

Run:

```bash
rtk cargo test
```

Expected: all tests pass.

- [x] **Step 3: Run clippy if available**

Run:

```bash
rtk cargo clippy --all-targets -- -D warnings
```

Expected: pass. If clippy is not installed, record that in the final handoff.

- [x] **Step 4: Inspect remaining duplication**

Run:

```bash
rtk rg -n "from_config_name|styled_segments_with_default|CommandSpec::Shell|starts_with\\(" src
```

Expected:
- `from_config_name` appears only in descriptor-backed lookup/tests.
- `styled_segments_with_default` appears only where module-specific logic is still justified.
- `CommandSpec::Shell` appears only in command helpers/tests and compatibility paths.
- `starts_with(` is not used to identify dynamic module ownership.

- [x] **Step 5: Final commit**

```bash
rtk git status --short
rtk git add src docs/superpowers/plans/2026-04-29-code-quality-refactors.md
rtk git commit -m "test: verify code quality refactors"
```

## Self-Review

- Spec coverage: The plan maps to all review findings: descriptor duplication, module graph extraction, shared template rendering, parser dispatch, subscription capabilities, dynamic origins, command execution policy, fallible constructors, and verification.
- Placeholder scan: No task uses TBD/TODO/fill-in placeholders. Where an implementation must be adapted to compile, the plan gives the exact target API and verification command.
- Type consistency: The plan uses `ModuleCapabilities`, `ModuleGraph`, `ModuleOrigin`, `CommandConfig`, and `CommandPolicy` consistently across the tasks that introduce and consume them.
