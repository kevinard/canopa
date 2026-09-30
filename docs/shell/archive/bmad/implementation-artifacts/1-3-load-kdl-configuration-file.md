# Story 1.3: Load KDL Configuration File

Status: done

## Story

As a **user**,
I want niribar to load its configuration from a KDL file,
so that I can customize the bar's behavior without recompiling.

## Acceptance Criteria

1. **Given** a valid KDL config file exists at `$XDG_CONFIG_HOME/niribar/config.kdl`
   **When** niribar starts
   **Then** the config file is parsed fully before any surface is created

2. **Given** the config file is loaded
   **When** niribar starts
   **Then** the parsed `bar.edge` value is applied to the surface anchor (left/right/top/bottom edge)

3. **Given** the config file is loaded
   **When** niribar starts
   **Then** the parsed `bar.collapsed-width` and `bar.expanded-width` values are stored in `App` state and used for surface dimensions

4. **Given** no config file exists at the XDG path
   **When** niribar starts
   **Then** the embedded default config (`crates/niribar/config/default.kdl`) is used transparently — no error is shown

5. **Given** niribar starts with the default config
   **When** the binary runs
   **Then** the user sees no error and the bar renders identically to Story 1.2 (left edge, 48px width)

## Tasks / Subtasks

- [x] Task 1: Define config types and public API in `src/config/mod.rs` (AC: #1, #3)
  - [x] 1.1: Define `Config { bar: BarConfig }` — the full parsed config struct; `#[derive(Debug, Clone)]`
  - [x] 1.2: Define `BarConfig { edge: Edge, collapsed_width: u32, expanded_width: u32 }` — `#[derive(Debug, Clone)]`; implement `Default` (left edge, 48, 200)
  - [x] 1.3: Define `Edge` enum: `Left`, `Right`, `Top`, `Bottom`; `#[derive(Debug, Clone, Default)]` with `#[default]` on `Left`
  - [x] 1.4: Implement `Edge::from_str(s: &str) -> anyhow::Result<Edge>` — maps `"left"|"right"|"top"|"bottom"` to variants; returns `anyhow::bail!` for unknown values
  - [x] 1.5: Implement `Edge::anchor(&self) -> iced_layershell::reexport::Anchor` — maps edges to anchor bit combinations (see Dev Notes for exact mapping)
  - [x] 1.6: Export `pub fn load() -> anyhow::Result<Config>` from `src/config/mod.rs` — reads XDG path or falls back to embedded default (see Dev Notes)

- [x] Task 2: Create `crates/niribar/src/config/defaults.rs` (AC: #4, #5)
  - [x] 2.1: `pub const DEFAULT_CONFIG: &str = include_str!("../../config/default.kdl");`
  - [x] 2.2: Populate `crates/niribar/config/default.kdl` with minimal valid content (see Dev Notes for exact content)

- [x] Task 3: Create `src/config/parse.rs` (AC: #1, #2, #3)
  - [x] 3.1: `pub fn parse(input: &str) -> anyhow::Result<Config>` — parse KDL string into `Config`
  - [x] 3.2: Parse `bar { edge "left"; collapsed-width 48; expanded-width 200 }` structure from KDL document
  - [x] 3.3: Missing `bar` section → use `BarConfig::default()` (no error — Story 1.4 handles validation)
  - [x] 3.4: Unknown `edge` string → propagate error via anyhow (Story 1.4 will format this nicely)

- [x] Task 4: Update `crates/niribar/src/main.rs` to load config before surface creation (AC: #1, #2, #3, #5)
  - [x] 4.1: Change `fn main() -> iced_layershell::Result` to `fn main() -> anyhow::Result<()>`
  - [x] 4.2: Add `let config = config::load()?;` as the first statement in `main()`
  - [x] 4.3: Extract `anchor` and `collapsed_width` from config before the builder chain (see Dev Notes for move/clone pattern)
  - [x] 4.4: Replace the hardcoded `Anchor::Left | Anchor::Top | Anchor::Bottom` with `config.bar.edge.anchor()`
  - [x] 4.5: Replace the hardcoded `exclusive_zone: 48` with `exclusive_zone: config.bar.collapsed_width as i32`
  - [x] 4.6: Replace `size: Some((48, 0))` with `size: Some((config.bar.collapsed_width, 0))`
  - [x] 4.7: Change `application(App::new, ...)` to use a closure that passes config to `App` (see Dev Notes)
  - [x] 4.8: Convert `iced_layershell::Error` to `anyhow::Error` at the `.run()` call (see Dev Notes)

- [x] Task 5: Update `crates/niribar/src/app.rs` to store config (AC: #3)
  - [x] 5.1: Add `pub config: Config` field to `App` struct
  - [x] 5.2: Replace `App::new() -> App` with `pub fn new_with_config(config: Config) -> App`; keep `App::new()` as a test helper returning `App::new_with_config(Config::default())` (where `Config::default()` is derived)
  - [x] 5.3: Add `#[derive(Default)]` to `Config` and `BarConfig` (required for `App::new()` test helper and `Config::default()` in parse.rs missing-section fallback)
  - [x] 5.4: Update existing tests — `App::new()` helper must still work; `new_constructs_without_panic` and `namespace_returns_niribar` must still pass

- [x] Task 6: Quality gates
  - [x] 6.1: `cargo build` — must succeed
  - [x] 6.2: `cargo clippy -- -D warnings` — zero warnings
  - [x] 6.3: `cargo fmt` then `cargo fmt --check` — must pass
  - [x] 6.4: `cargo test` — all existing tests must pass; at least 2 new tests must pass (see Dev Notes)

### Review Follow-ups (AI)

- [x] [Review][Decision] **expanded_width AC3 interpretation** — resolved: AC3 satisfied; collapsed_width drives surface dimensions, expanded_width stored for Story 3.1.
- [x] [Review][Patch] **HOME unset → empty string produces nonsensical config path** [`src/config/mod.rs:config_file_path`] — fixed: `config_file_path()` now returns `anyhow::Result`; errors if both XDG_CONFIG_HOME and HOME are unset/empty.
- [x] [Review][Patch] **Negative integer silently wraps to u32 in parser** [`src/config/parse.rs:parse_bar_section`] — fixed: `u32::try_from(v).ok()` used; negative values fall back to defaults.
- [x] [Review][Patch] **u32 → i32 cast for exclusive_zone can overflow** [`crates/niribar/src/main.rs`] — fixed: `i32::try_from(collapsed_width).unwrap_or(48)` used.
- [x] [Review][Patch] **TOCTOU race: path.exists() then read_to_string** [`src/config/mod.rs:load`] — fixed: `match read_to_string` with `ErrorKind::NotFound` fallback.
- [x] [Review][Patch] **Default values 48/200 duplicated between BarConfig::default() and parse.rs** [`src/config/parse.rs:parse_bar_section`] — fixed: parser starts with `let defaults = BarConfig::default()` and uses its field values.
- [x] [Review][Patch] **Edge::from_str is pub — reduce to pub(crate)** [`src/config/mod.rs`] — fixed: changed to `pub(crate) fn from_str`.
- [x] [Review][Defer] **Edge::from_str should implement std::str::FromStr trait** [`src/config/mod.rs`] — deferred, pre-existing design; Story 1.4 or later refactor
- [x] [Review][Defer] **saturating_sub(1) masks collapsed_width=0 misconfiguration** [`crates/niribar/src/app.rs:view`] — deferred, Story 1.4 adds integer validation
- [x] [Review][Defer] **Test coverage of view() behavior** [`crates/niribar/src/app.rs`] — deferred, can't introspect Iced Element output without a live renderer
- [x] [Review][Defer] **Edge::from_str case-sensitivity undocumented** [`src/config/mod.rs`] — deferred, Story 1.4 error formatting will document expected values

## Dev Notes

### CRITICAL: iced_layershell 0.17.1 — Actual Versions in Use

Story 1.1 upgraded to `iced_layershell = "0.17.1"` / `iced = "0.14"` (not 0.13.7 as documented in the architecture doc). All patterns from Story 1.2 apply unchanged. See Story 1.2 dev notes for the definitive API reference.

### main() Return Type Change

From Story 1.3 onward, `main()` returns `anyhow::Result<()>` (as noted in Story 1.2 dev notes). The change lets config-loading errors propagate cleanly before iced takes over:

```rust
use anyhow::Context;

fn main() -> anyhow::Result<()> {
    let config = config::load()?;
    let anchor = config.bar.edge.anchor();
    let collapsed_width = config.bar.collapsed_width;
    // Extract before moving config into closure

    application(
        move || App::new_with_config(config),
        App::namespace,
        App::update,
        App::view,
    )
    .settings(Settings {
        layer_settings: LayerShellSettings {
            anchor,
            exclusive_zone: collapsed_width as i32,
            size: Some((collapsed_width, 0)),
            ..Default::default()
        },
        ..Default::default()
    })
    .style(App::style)
    .run()
    .map_err(anyhow::Error::from)
}
```

**Note on `.map_err(anyhow::Error::from)`:** `iced_layershell::Error` implements `std::error::Error`, so `anyhow::Error::from` converts it. If the compiler rejects this, use `.map_err(|e| anyhow::anyhow!("{e}"))` as a fallback.

**Note on `Fn` vs `FnOnce`:** If `application()` requires `Fn()` (callable multiple times), the closure must capture `Config` by clone rather than move. In that case:

```rust
let config_for_closure = config.clone();
application(move || App::new_with_config(config_for_closure.clone()), ...)
```

Check the `iced_layershell::build_pattern::application` signature to confirm whether it requires `Fn` or `FnOnce`.

### Edge → Anchor Mapping

```rust
use iced_layershell::reexport::Anchor;

impl Edge {
    pub fn anchor(&self) -> Anchor {
        match self {
            Edge::Left   => Anchor::Left  | Anchor::Top | Anchor::Bottom,
            Edge::Right  => Anchor::Right | Anchor::Top | Anchor::Bottom,
            Edge::Top    => Anchor::Top   | Anchor::Left | Anchor::Right,
            Edge::Bottom => Anchor::Bottom | Anchor::Left | Anchor::Right,
        }
    }
}
```

Default is `Edge::Left`, which produces the same anchor as the hardcoded value in Story 1.2.

### XDG Config Path Resolution

No new crate needed — use `std::env`:

```rust
fn config_file_path() -> std::path::PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_default();
            std::path::PathBuf::from(home).join(".config")
        });
    base.join("niribar").join("config.kdl")
}

pub fn load() -> anyhow::Result<Config> {
    let path = config_file_path();
    let content = if path.exists() {
        std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?
    } else {
        defaults::DEFAULT_CONFIG.to_string()
    };
    parse::parse(&content)
}
```

### kdl 6.5.0 API — Verify Against Actual Crate

The architecture doc was written targeting `kdl` 6.5.0 but does not specify its API. The `kdl` crate had breaking API changes between major versions. Verify using `cargo doc --open` or the crates.io documentation for 6.5.0 before implementing `parse.rs`.

**Expected 6.x API (verify before trusting):**

```rust
use kdl::{KdlDocument, KdlNode, KdlValue};

pub fn parse(input: &str) -> anyhow::Result<Config> {
    let doc: KdlDocument = input.parse()
        .map_err(|e: kdl::Error| anyhow::anyhow!("KDL parse error: {}", e))?;
    let bar = parse_bar_section(&doc)?;
    Ok(Config { bar })
}

fn parse_bar_section(doc: &KdlDocument) -> anyhow::Result<BarConfig> {
    let Some(bar_node) = doc.get("bar") else {
        return Ok(BarConfig::default());
    };
    let children = match bar_node.children() {
        Some(c) => c,
        None => return Ok(BarConfig::default()),
    };

    let edge = children.get("edge")
        .and_then(|n| n.get(0))           // first positional arg
        .and_then(|v| v.as_string())
        .map(Edge::from_str)
        .transpose()?
        .unwrap_or_default();

    let collapsed_width = children.get("collapsed-width")
        .and_then(|n| n.get(0))
        .and_then(|v| v.as_i64())
        .map(|v| v as u32)
        .unwrap_or(48);

    let expanded_width = children.get("expanded-width")
        .and_then(|n| n.get(0))
        .and_then(|v| v.as_i64())
        .map(|v| v as u32)
        .unwrap_or(200);

    Ok(BarConfig { edge, collapsed_width, expanded_width })
}
```

**Key kdl 6.x method names to verify:**
- `KdlDocument::get(name: &str) -> Option<&KdlNode>` — first node by name
- `KdlNode::get(idx: usize) -> Option<&KdlValue>` — positional argument
- `KdlNode::children() -> Option<&KdlDocument>` — child nodes
- `KdlValue::as_string() -> Option<&str>` — string value
- `KdlValue::as_i64() -> Option<i64>` — integer value

If these don't match the actual API, adjust accordingly. The kdl 6.x docs are at [docs.rs/kdl/6.5.0](https://docs.rs/kdl/6.5.0).

### crates/niribar/config/default.kdl Content

Populate the existing placeholder file (`crates/niribar/config/default.kdl`) with:

```kdl
// niribar default configuration
// Override by placing your config at $XDG_CONFIG_HOME/niribar/config.kdl

bar {
    edge "left"
    collapsed-width 48
    expanded-width 200
}
```

Story 1.4 adds validation so errors in this file surface cleanly. Story 2.x adds the `modules { }` block. Keep this minimal for Story 1.3.

**`include_str!` path** in `crates/niribar/src/config/defaults.rs` relative to the source file:
```rust
pub const DEFAULT_CONFIG: &str = include_str!("../../config/default.kdl");
```

This resolves to `{project-root}/config/default.kdl` — the file that already exists with a placeholder comment.

### App Struct Change — Preserve Existing Tests

```rust
// crates/niribar/src/app.rs
use crate::config::Config;

pub struct App {
    pub config: Config,
}

impl App {
    pub fn new_with_config(config: Config) -> Self {
        App { config }
    }

    // Keep for test use only — do not call from main()
    pub fn new() -> Self {
        App::new_with_config(Config::default())
    }
    // ...
}
```

`App::new()` must remain in scope so existing tests `namespace_returns_niribar` and `new_constructs_without_panic` compile unchanged. Do NOT rename the existing tests.

### config/mod.rs Module Declarations

`src/config/mod.rs` must declare its submodules:

```rust
mod defaults;
mod parse;

pub use parse::parse;  // expose if needed externally, or keep private

pub fn load() -> anyhow::Result<Config> { ... }
```

### Scope Boundary — What NOT to Do

- **Do not add `ModuleConfig`, `OutputConfig`, `AnimationConfig`** — those are for Story 2.x and 7.x.
- **Do not implement the formatted error message** (`error: <field.path>: ...`) — that is Story 1.4.
- **Do not add `colors`, `font`, `spacing` parsing** — those are later stories.
- **Do not modify `src/surface/mod.rs`** — `SurfaceState` still has its `#[allow(dead_code)]` stub from Story 1.2.
- **Do not add `niri-ipc`, `dirs`, or other new crates** — `Cargo.toml` is unchanged for this story.

### Required New Tests

Add to `src/config/mod.rs` (or `parse.rs`):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_default_config_succeeds() {
        let config = parse::parse(defaults::DEFAULT_CONFIG).unwrap();
        assert_eq!(config.bar.collapsed_width, 48);
        assert_eq!(config.bar.expanded_width, 200);
    }

    #[test]
    fn missing_bar_section_uses_defaults() {
        let config = parse::parse("").unwrap();
        assert_eq!(config.bar.collapsed_width, 48);
    }
}
```

### Project Structure Notes

- `src/config/mod.rs` — was a single-line placeholder; now owns `Config`, `BarConfig`, `Edge`, `load()`
- `src/config/parse.rs` — new file; owns `parse()` function
- `crates/niribar/src/config/defaults.rs` — new file; owns `DEFAULT_CONFIG` constant
- `crates/niribar/config/default.kdl` — was a placeholder comment; now has minimal valid KDL content
- `crates/niribar/src/main.rs` — return type changed, config loaded, closure replaces `App::new` ref, anchor/size from config
- `crates/niribar/src/app.rs` — `App` gains `config` field, `new_with_config()` constructor added
- All other stubs (`src/modules/mod.rs`, `src/niri/mod.rs`, `src/dbus/mod.rs`, `src/surface/mod.rs`) **untouched**

### References

- [Source: _bmad-output/planning-artifacts/architecture.md#Configuration Architecture] — XDG path, kdl crate, Config struct, anyhow for startup errors, AR7
- [Source: _bmad-output/planning-artifacts/architecture.md#Project Structure] — crates/niribar/src/config/ file breakdown (mod.rs, parse.rs, defaults.rs)
- [Source: _bmad-output/planning-artifacts/architecture.md#Error Type Selection] — anyhow::Result for config; no Result from subscriptions
- [Source: _bmad-output/planning-artifacts/architecture.md#Config Boundary] — Config fully parsed before any surface created; no KDL types leak out
- [Source: _bmad-output/planning-artifacts/epics.md#Story 1.3] — acceptance criteria
- [Source: _bmad-output/planning-artifacts/ux-design-specification.md#Component Strategy] — size tokens: collapsed 48px, expanded 200px
- [Source: _bmad-output/planning-artifacts/ux-design-specification.md#Configuration Error Message Format] — format owned by Story 1.4
- [Source: _bmad-output/implementation-artifacts/1-2-render-a-static-bar-surface-on-wayland.md#Dev Notes] — iced_layershell 0.17.1 API, anyhow deferred note, actual versions in use
- AR7: Config fully parsed and validated before any surfaces are created

## Dev Agent Record

### Agent Model Used

claude-sonnet-4-6

### Debug Log References

- **kdl 6.5.0 API diff from story notes:** `KdlValue::as_integer()` returns `Option<i128>` (not `i64`). `KdlDocument::get_arg(name)` is a shorthand that calls `doc.get(name).and_then(|n| n.get(0))` — very convenient for flat key-value style node children.
- **clippy `uninlined_format_args`:** `anyhow::bail!("... {other:?}")` must use inline format captures, not a trailing `other` argument.
- **`cargo fmt` style:** `input.parse().context(...)` must be on one line; multi-line chained calls on a single expression are reformatted by rustfmt.
- **`view()` content width must use config:** `content_width = collapsed_width.saturating_sub(1)` — hardcoded `47` replaced with dynamic value from `self.config.bar.collapsed_width`.
- **`Config::default()` derived from `BarConfig::default()`:** `BarConfig` uses a manual `impl Default` (not `#[derive(Default)]`) because the default values (48, 200) are non-zero, which `derive` would set to 0. `Config` uses `#[derive(Default)]` because it delegates to `BarConfig::default()`.

### Completion Notes List

- All 6 tasks / 25 subtasks completed.
- `cargo build`: succeeded.
- `cargo clippy -- -D warnings`: zero warnings (one `uninlined_format_args` fix required).
- `cargo fmt --check`: passes.
- `cargo test`: 15 tests passed (4 pre-existing + 11 new) — zero failures.
- `main()` return type changed from `iced_layershell::Result` to `anyhow::Result<()>`; config loaded before iced app starts (AR7 satisfied).
- Config closure `move || App::new_with_config(config.clone())` used because `application()` requires `Fn()` (not `FnOnce()`).
- `expanded_width` and `config` field marked `#[allow(dead_code)]` — used in Stories 3.1 and 2.x respectively.
- AC1–AC5 all satisfied: config parsed pre-surface, edge/width from config drive surface settings, default embedded config used as fallback, no errors on default config run.

### File List

- src/config/mod.rs
- crates/niribar/src/config/defaults.rs
- src/config/parse.rs
- crates/niribar/src/app.rs
- crates/niribar/src/main.rs
- crates/niribar/config/default.kdl
- _bmad-output/implementation-artifacts/1-3-load-kdl-configuration-file.md
- _bmad-output/implementation-artifacts/sprint-status.yaml

### Change Log

- 2026-03-31: Story 1.3 implemented — KDL config loading via `crates/niribar/src/config/` module (`Config`, `BarConfig`, `Edge` types; XDG path resolution; embedded default.kdl fallback); `main()` now loads config before iced app starts; `App` stores `Config`; 11 new tests added; all quality gates pass.
