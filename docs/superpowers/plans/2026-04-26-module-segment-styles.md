# Module Segment Styles Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Per-segment style overrides (color, size, weight) on module formats, with state-keyed style maps so users can drive coloring from module state (battery critical, network limited, volume muted, workspace focused, etc.).

**Architecture:** Extend the existing `FormatTemplate` to hold `StyledSegment` instead of bare `TextTemplate`. Each segment carries an optional `SegmentStyles` enum (`None`, `Static`, `Stateful`). Parser accepts both bare positional-string form and a child-node form (`segment "..." color="..."` or `segment "..." { style ... }`). Renderer resolves each segment's style against the module's current state at render time and returns `Vec<(String, ResolvedStyle)>`. Each module passes its `StateKey` (or `None`) into the render call.

**Tech Stack:** Rust, KDL config, iced UI. Existing `theme.rs` palette constants. Existing format-templates infrastructure in `src/config.rs` and `src/config/parse/modules/formats.rs`.

**User note:** This codebase's owner has indicated no git commits without explicit request. Treat each `git commit` step as a checkpoint — pause and confirm with the user before running it.

---

## File Structure

**New files:**
- `src/config/style.rs` — `Style`, `ColorValue`, `ThemeToken`, `Weight`, `ResolvedStyle`, `SegmentStyles`, color/weight string parsers and tests.
- `src/config/parse/modules/segment_styles.rs` — KDL parser helper for child-node `segment` / `style` syntax.

**Modified files:**
- `src/config.rs` — re-export style types; change `FormatTemplate::segments` from `Vec<TextTemplate>` to `Vec<StyledSegment>`; add `render_styled_segments` and `render_styled_joined`. (lines around 122–286)
- `src/config/parse/modules/formats.rs` — extend `parse_field` to read either positional strings or child `segment` nodes; thread per-module state-selector list. (full file)
- `src/config/parse/modules/{battery,volume,network,workspaces,clock,script,button,focused_window}.rs` — supply each module's allowed `state` selector list to the format parser.
- `src/modules/{battery,volume,network,workspaces,clock,script,button,focused_window}.rs` — emit a `StateKey` and switch render call sites to `render_styled_segments`.
- `src/modules/view.rs` — accept `Vec<(String, ResolvedStyle)>` from formatted modules and apply the style to the iced text widget.
- `src/theme.rs` — add `ThemeToken::resolve()` impl that maps tokens to existing constants (or place it on `ColorValue` in the new module).

---

## Task 1: Style primitives — `Style`, `ColorValue`, `ThemeToken`, `Weight`

**Files:**
- Create: `src/config/style.rs`
- Modify: `src/config.rs:1-10` (add `pub mod style;` and re-exports)
- Test: `src/config/style.rs` (inline `#[cfg(test)]` module)

- [ ] **Step 1: Write the failing tests**

```rust
// src/config/style.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_theme_token_color() {
        assert_eq!(
            ColorValue::parse("accent").unwrap(),
            ColorValue::Token(ThemeToken::Accent)
        );
        assert_eq!(
            ColorValue::parse("CRITICAL").unwrap(),
            ColorValue::Token(ThemeToken::Critical)
        );
    }

    #[test]
    fn parse_hex_rgb_color() {
        let c = ColorValue::parse("#ff8800").unwrap();
        match c {
            ColorValue::Rgba(rgba) => {
                assert!((rgba.r - 1.0).abs() < 1e-3);
                assert!((rgba.g - 0.533).abs() < 1e-2);
                assert!((rgba.b - 0.0).abs() < 1e-3);
                assert!((rgba.a - 1.0).abs() < 1e-3);
            }
            _ => panic!("expected hex"),
        }
    }

    #[test]
    fn parse_hex_rgba_color() {
        let c = ColorValue::parse("#ff880080").unwrap();
        match c {
            ColorValue::Rgba(rgba) => assert!((rgba.a - 0.502).abs() < 1e-2),
            _ => panic!("expected hex"),
        }
    }

    #[test]
    fn parse_invalid_color_string() {
        assert!(ColorValue::parse("magenta").is_err());
        assert!(ColorValue::parse("#ff").is_err());
        assert!(ColorValue::parse("#gggggg").is_err());
        assert!(ColorValue::parse("").is_err());
    }

    #[test]
    fn parse_weight() {
        assert_eq!(Weight::parse("light").unwrap(), Weight::Light);
        assert_eq!(Weight::parse("normal").unwrap(), Weight::Normal);
        assert_eq!(Weight::parse("bold").unwrap(), Weight::Bold);
        assert!(Weight::parse("extrabold").is_err());
        assert!(Weight::parse("").is_err());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib config::style`
Expected: FAIL — `style` module not found.

- [ ] **Step 3: Create `src/config/style.rs` with the types**

```rust
use anyhow::{anyhow, bail};
use iced::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeToken {
    Accent, Text, Subtext, Base,
    Surface0, Surface1, Border, Overlay0,
    Critical, Warning, Success,
}

impl ThemeToken {
    pub fn resolve(self) -> Color {
        use crate::theme;
        match self {
            ThemeToken::Accent => theme::ACCENT,
            ThemeToken::Text => theme::TEXT,
            ThemeToken::Subtext => theme::SUBTEXT,
            ThemeToken::Base => theme::BASE,
            ThemeToken::Surface0 => theme::SURFACE0,
            ThemeToken::Surface1 => theme::SURFACE1,
            ThemeToken::Border => theme::BORDER,
            ThemeToken::Overlay0 => theme::OVERLAY0,
            ThemeToken::Critical => theme::CRITICAL,
            ThemeToken::Warning => theme::WARNING,
            ThemeToken::Success => theme::SUCCESS,
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        Some(match name.to_ascii_lowercase().as_str() {
            "accent" => ThemeToken::Accent,
            "text" => ThemeToken::Text,
            "subtext" => ThemeToken::Subtext,
            "base" => ThemeToken::Base,
            "surface0" => ThemeToken::Surface0,
            "surface1" => ThemeToken::Surface1,
            "border" => ThemeToken::Border,
            "overlay0" => ThemeToken::Overlay0,
            "critical" => ThemeToken::Critical,
            "warning" => ThemeToken::Warning,
            "success" => ThemeToken::Success,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorValue {
    Token(ThemeToken),
    Rgba(Color),
}

impl ColorValue {
    pub fn resolve(self) -> Color {
        match self {
            ColorValue::Token(t) => t.resolve(),
            ColorValue::Rgba(c) => c,
        }
    }

    pub fn parse(raw: &str) -> anyhow::Result<Self> {
        if raw.is_empty() {
            bail!("color value must not be empty.");
        }
        if let Some(token) = ThemeToken::from_name(raw) {
            return Ok(ColorValue::Token(token));
        }
        if let Some(hex) = raw.strip_prefix('#') {
            return parse_hex(hex)
                .map(ColorValue::Rgba)
                .ok_or_else(|| anyhow!("invalid hex color {raw:?}."));
        }
        Err(anyhow!("unknown color {raw:?}; expected theme token or #rrggbb[aa]."))
    }
}

fn parse_hex(hex: &str) -> Option<Color> {
    let bytes = match hex.len() {
        6 | 8 => hex.as_bytes(),
        _ => return None,
    };
    let mut out = [0u8; 4];
    out[3] = 0xff;
    for (i, chunk) in bytes.chunks(2).enumerate() {
        let s = std::str::from_utf8(chunk).ok()?;
        out[i] = u8::from_str_radix(s, 16).ok()?;
    }
    Some(Color::from_rgba8(out[0], out[1], out[2], out[3] as f32 / 255.0))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    Light, Normal, Bold,
}

impl Weight {
    pub fn parse(raw: &str) -> anyhow::Result<Self> {
        Ok(match raw {
            "light" => Weight::Light,
            "normal" => Weight::Normal,
            "bold" => Weight::Bold,
            _ => bail!("unknown weight {raw:?}; expected light|normal|bold."),
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Style {
    pub color: Option<ColorValue>,
    pub size: Option<f32>,
    pub weight: Option<Weight>,
}

impl Style {
    pub fn is_empty(&self) -> bool {
        self.color.is_none() && self.size.is_none() && self.weight.is_none()
    }

    pub fn merge_over(self, base: Style) -> Style {
        Style {
            color: self.color.or(base.color),
            size: self.size.or(base.size),
            weight: self.weight.or(base.weight),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ResolvedStyle {
    pub color: Option<Color>,
    pub size: Option<f32>,
    pub weight: Option<Weight>,
}

impl Style {
    pub fn resolve(&self) -> ResolvedStyle {
        ResolvedStyle {
            color: self.color.map(ColorValue::resolve),
            size: self.size,
            weight: self.weight,
        }
    }
}
```

Add to `src/config.rs` near the existing module declarations (around line 1):

```rust
pub mod style;

pub use style::{ColorValue, ResolvedStyle, Style, ThemeToken, Weight};
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib config::style`
Expected: PASS for all 5 tests.

- [ ] **Step 5: Commit (after user confirmation)**

```bash
git add src/config.rs src/config/style.rs
git commit -m "feat: add style primitives for module segments"
```

---

## Task 2: `SegmentStyles` and state-key matching

**Files:**
- Modify: `src/config/style.rs` (append)
- Test: `src/config/style.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn segment_styles_none_resolves_empty() {
    let s = SegmentStyles::None;
    assert_eq!(s.resolve_for(None), ResolvedStyle::default());
    assert_eq!(s.resolve_for(Some("critical")), ResolvedStyle::default());
}

#[test]
fn segment_styles_static_always_returns_same() {
    let style = Style { color: Some(ColorValue::Token(ThemeToken::Accent)), ..Default::default() };
    let s = SegmentStyles::Static(style);
    let expected = style.resolve();
    assert_eq!(s.resolve_for(None), expected);
    assert_eq!(s.resolve_for(Some("critical")), expected);
}

#[test]
fn segment_styles_stateful_picks_matching_state() {
    let default = Style { color: Some(ColorValue::Token(ThemeToken::Accent)), ..Default::default() };
    let crit = Style { color: Some(ColorValue::Token(ThemeToken::Critical)), ..Default::default() };
    let s = SegmentStyles::Stateful {
        default: Some(default),
        by_state: vec![("critical".into(), crit)],
    };
    assert_eq!(s.resolve_for(Some("critical")), crit.resolve());
    assert_eq!(s.resolve_for(Some("normal")), default.resolve());
    assert_eq!(s.resolve_for(None), default.resolve());
}

#[test]
fn segment_styles_stateful_no_default_no_match_is_empty() {
    let s = SegmentStyles::Stateful {
        default: None,
        by_state: vec![("critical".into(), Style {
            color: Some(ColorValue::Token(ThemeToken::Critical)),
            ..Default::default()
        })],
    };
    assert_eq!(s.resolve_for(Some("normal")), ResolvedStyle::default());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib config::style`
Expected: FAIL — `SegmentStyles` not found.

- [ ] **Step 3: Append `SegmentStyles` to `src/config/style.rs`**

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum SegmentStyles {
    None,
    Static(Style),
    Stateful {
        default: Option<Style>,
        by_state: Vec<(String, Style)>,
    },
}

impl Default for SegmentStyles {
    fn default() -> Self {
        SegmentStyles::None
    }
}

impl SegmentStyles {
    pub fn resolve_for(&self, state: Option<&str>) -> ResolvedStyle {
        match self {
            SegmentStyles::None => ResolvedStyle::default(),
            SegmentStyles::Static(s) => s.resolve(),
            SegmentStyles::Stateful { default, by_state } => {
                if let Some(state) = state {
                    if let Some((_, style)) = by_state.iter().find(|(k, _)| k == state) {
                        return style.resolve();
                    }
                }
                default.map(|s| s.resolve()).unwrap_or_default()
            }
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib config::style`
Expected: PASS.

- [ ] **Step 5: Commit (after user confirmation)**

```bash
git add src/config/style.rs
git commit -m "feat: add SegmentStyles state-key matching"
```

---

## Task 3: `StyledSegment` and updated `FormatTemplate`

**Files:**
- Modify: `src/config.rs:149-176` (replace `FormatTemplate`)
- Modify: `src/config.rs:617-630` (existing tests still need to pass)

- [ ] **Step 1: Write the failing tests**

Add to the existing `mod tests` block in `src/config.rs` (near line 615):

```rust
#[test]
fn render_styled_segments_returns_resolved_style_per_segment() {
    use crate::config::style::{ColorValue, SegmentStyles, Style, ThemeToken};

    let mut tmpl = FormatTemplate::parse(&["{icon}", "{value}%"], &["icon", "value"], "test.format").unwrap();
    tmpl.set_segment_styles(0, SegmentStyles::Static(Style {
        color: Some(ColorValue::Token(ThemeToken::Accent)),
        ..Default::default()
    }));
    let rendered = tmpl.render_styled_segments(
        &[("icon", "▓".into()), ("value", "42".into())],
        None,
    );
    assert_eq!(rendered.len(), 2);
    assert_eq!(rendered[0].0, "▓");
    assert_eq!(rendered[0].1.color, Some(ThemeToken::Accent.resolve()));
    assert_eq!(rendered[1].0, "42%");
    assert_eq!(rendered[1].1, ResolvedStyle::default());
}

#[test]
fn render_styled_segments_picks_state_keyed_style() {
    use crate::config::style::{ColorValue, SegmentStyles, Style, ThemeToken};

    let mut tmpl = FormatTemplate::parse(&["{icon}"], &["icon"], "test.format").unwrap();
    tmpl.set_segment_styles(0, SegmentStyles::Stateful {
        default: Some(Style { color: Some(ColorValue::Token(ThemeToken::Accent)), ..Default::default() }),
        by_state: vec![
            ("critical".into(), Style { color: Some(ColorValue::Token(ThemeToken::Critical)), ..Default::default() }),
        ],
    });
    let crit = tmpl.render_styled_segments(&[("icon", "x".into())], Some("critical"));
    assert_eq!(crit[0].1.color, Some(ThemeToken::Critical.resolve()));
    let norm = tmpl.render_styled_segments(&[("icon", "x".into())], Some("normal"));
    assert_eq!(norm[0].1.color, Some(ThemeToken::Accent.resolve()));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib`
Expected: FAIL — `set_segment_styles`, `render_styled_segments` do not exist.

- [ ] **Step 3: Replace `FormatTemplate` in `src/config.rs`**

Replace lines 149–176 with:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct FormatTemplate {
    segments: Vec<StyledSegment>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StyledSegment {
    template: TextTemplate,
    styles: SegmentStyles,
}

impl StyledSegment {
    pub fn new(template: TextTemplate) -> Self {
        StyledSegment { template, styles: SegmentStyles::None }
    }

    pub fn template(&self) -> &TextTemplate { &self.template }
    pub fn styles(&self) -> &SegmentStyles { &self.styles }
    pub fn set_styles(&mut self, styles: SegmentStyles) { self.styles = styles; }
}

impl FormatTemplate {
    pub fn parse(raw_segments: &[&str], allowed: &[&str], path: &str) -> anyhow::Result<Self> {
        if raw_segments.is_empty() {
            anyhow::bail!("{path}: expected at least one string value.");
        }
        let mut segments = Vec::with_capacity(raw_segments.len());
        for raw in raw_segments {
            segments.push(StyledSegment::new(TextTemplate::parse(raw, allowed, path)?));
        }
        Ok(FormatTemplate { segments })
    }

    pub fn from_styled(segments: Vec<StyledSegment>) -> Self {
        FormatTemplate { segments }
    }

    pub fn segments(&self) -> &[StyledSegment] { &self.segments }

    pub fn set_segment_styles(&mut self, index: usize, styles: SegmentStyles) {
        if let Some(s) = self.segments.get_mut(index) {
            s.set_styles(styles);
        }
    }

    pub fn render_segments(&self, values: &[(&str, String)]) -> Vec<String> {
        self.segments
            .iter()
            .map(|s| s.template.render_pairs(values))
            .collect()
    }

    pub fn render_joined(&self, values: &[(&str, String)]) -> String {
        self.render_segments(values).join(" ")
    }

    pub fn render_styled_segments(
        &self,
        values: &[(&str, String)],
        state: Option<&str>,
    ) -> Vec<(String, ResolvedStyle)> {
        self.segments
            .iter()
            .map(|s| (s.template.render_pairs(values), s.styles.resolve_for(state)))
            .collect()
    }
}
```

Also add at the top of `src/config.rs` (just after the existing `pub use style::...` re-export):

```rust
pub use style::SegmentStyles;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib`
Expected: PASS for new tests; existing `render_segments` / `render_joined` tests still pass.

- [ ] **Step 5: Commit (after user confirmation)**

```bash
git add src/config.rs
git commit -m "feat: add StyledSegment and styled rendering"
```

---

## Task 4: Per-module state vocabularies (constants)

**Files:**
- Modify: `src/config.rs` (append a small module of state-list constants)

- [ ] **Step 1: Write the failing test**

Add to the existing `mod tests` block in `src/config.rs`:

```rust
#[test]
fn state_selectors_per_module() {
    use crate::config::states;
    assert_eq!(states::BATTERY, &["charging", "critical", "low", "normal"]);
    assert_eq!(states::VOLUME, &["muted", "unmuted"]);
    assert_eq!(states::NETWORK, &["full", "limited", "none"]);
    assert_eq!(states::WORKSPACES, &["focused", "active", "inactive"]);
    assert!(states::EMPTY.is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib state_selectors_per_module`
Expected: FAIL — `states` module not found.

- [ ] **Step 3: Add the module**

Append to `src/config.rs`:

```rust
pub mod states {
    pub const BATTERY: &[&str] = &["charging", "critical", "low", "normal"];
    pub const VOLUME: &[&str] = &["muted", "unmuted"];
    pub const NETWORK: &[&str] = &["full", "limited", "none"];
    pub const WORKSPACES: &[&str] = &["focused", "active", "inactive"];
    pub const EMPTY: &[&str] = &[];
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib state_selectors_per_module`
Expected: PASS.

- [ ] **Step 5: Commit (after user confirmation)**

```bash
git add src/config.rs
git commit -m "feat: declare module state selector vocabularies"
```

---

## Task 5: KDL parser — child-node `segment` form (no styles yet)

**Files:**
- Create: `src/config/parse/modules/segment_styles.rs`
- Modify: `src/config/parse/modules/formats.rs` (rewrite `parse_field`)
- Modify: `src/config/parse/modules/mod.rs` (add `mod segment_styles;`)
- Test: `src/config/parse/tests.rs` (add a battery test using child-node form without styles)

- [ ] **Step 1: Write the failing tests**

Add to `src/config/parse/tests.rs`:

```rust
#[test]
fn battery_format_accepts_child_node_form() {
    let cfg = parse_module_for_test(r#"
        module "battery" {
            format {
                segment "{icon}"
                segment "{value}%"
            }
        }
    "#).unwrap();
    let battery = cfg.battery();
    let segs = battery.format.collapsed.as_ref().unwrap()
        .render_segments(&[("icon", "B".into()), ("value", "42".into())]);
    assert_eq!(segs, vec!["B".to_string(), "42%".to_string()]);
}

#[test]
fn battery_format_mixed_positional_and_segment_children() {
    let cfg = parse_module_for_test(r#"
        module "battery" {
            format "{icon}" {
                segment "{value}%"
            }
        }
    "#).unwrap();
    let battery = cfg.battery();
    let segs = battery.format.collapsed.as_ref().unwrap()
        .render_segments(&[("icon", "B".into()), ("value", "42".into())]);
    assert_eq!(segs, vec!["B".to_string(), "42%".to_string()]);
}
```

If `parse_module_for_test` and `cfg.battery()` are not present in `tests.rs`, use the existing test helpers in that file as a model — read `src/config/parse/tests.rs` first and adapt the test to whatever fixture/helper conventions are already used there. The behavior asserted (two segments, rendered in order) is what matters.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib battery_format`
Expected: FAIL — child-node `segment` not supported.

- [ ] **Step 3: Create `src/config/parse/modules/segment_styles.rs`**

```rust
use ::kdl::{KdlDocument, KdlEntry, KdlNode};

use crate::config::{StyledSegment, TextTemplate};

/// Parse a `format`-family node, supporting both bare positional strings and
/// child `segment` nodes. Style props are NOT parsed yet; that is added in a
/// follow-up task.
pub(super) fn parse_segments(
    node: &KdlNode,
    allowed_placeholders: &[&str],
    _allowed_states: &[&str],
    path: &str,
) -> anyhow::Result<Vec<StyledSegment>> {
    let mut out = Vec::new();

    // Positional string args become unstyled segments.
    for entry in node.entries() {
        if entry.name().is_some() {
            anyhow::bail!("{path}: unexpected named property on format node.");
        }
        let raw = entry.value().as_string()
            .ok_or_else(|| anyhow::anyhow!("{path}: expected string value."))?;
        out.push(StyledSegment::new(TextTemplate::parse(raw, allowed_placeholders, path)?));
    }

    // Child `segment` nodes are appended in order.
    if let Some(children) = node.children() {
        for (i, child) in children.nodes().iter().enumerate() {
            if child.name().value() != "segment" {
                anyhow::bail!("{path}: unexpected child node {:?}; expected `segment`.", child.name().value());
            }
            let seg_path = format!("{path}.segment[{i}]");
            let raw = child.entries().iter()
                .find(|e| e.name().is_none())
                .and_then(|e| e.value().as_string())
                .ok_or_else(|| anyhow::anyhow!("{seg_path}: expected one positional string."))?;
            out.push(StyledSegment::new(TextTemplate::parse(raw, allowed_placeholders, &seg_path)?));
            // Reject any other entries on `segment` until Task 6 adds style props.
            for e in child.entries() {
                if e.name().is_some() {
                    anyhow::bail!("{seg_path}: unknown property {:?} (styling not yet supported).", e.name().unwrap().value());
                }
            }
            if child.children().is_some() {
                anyhow::bail!("{seg_path}: child nodes not yet supported.");
            }
        }
    }

    if out.is_empty() {
        anyhow::bail!("{path}: expected at least one segment.");
    }
    Ok(out)
}

// Suppress the unused-import warning while `KdlEntry` is used implicitly.
#[allow(dead_code)]
fn _ensure_imports(_: &KdlEntry) {}
```

In `src/config/parse/modules/mod.rs`, add (next to the existing `mod formats;`):

```rust
mod segment_styles;
```

Replace `parse_field` in `src/config/parse/modules/formats.rs`:

```rust
fn parse_field(
    children: &KdlDocument,
    field: &str,
    path: &str,
    allowed: &[&str],
    allowed_states: &[&str],
) -> anyhow::Result<Option<FormatTemplate>> {
    let Some(node) = children.get(field) else {
        return Ok(None);
    };
    let segments = super::segment_styles::parse_segments(node, allowed, allowed_states, path)?;
    Ok(Some(FormatTemplate::from_styled(segments)))
}
```

Update the call sites of `parse_field` (still in `formats.rs`) and the public `parse_optional` to thread `allowed_states`:

```rust
pub(super) fn parse_optional(
    children: &KdlDocument,
    path: &str,
    allowed: &[&str],
    allowed_states: &[&str],
) -> anyhow::Result<FormatConfig> {
    Ok(FormatConfig {
        collapsed: parse_field(children, "format",
            &format!("{path}.format"), allowed, allowed_states)?,
        expanded: parse_field(children, "expanded-format",
            &format!("{path}.expanded-format"), allowed, allowed_states)?,
        horizontal: parse_field(children, "horizontal-format",
            &format!("{path}.horizontal-format"), allowed, allowed_states)?,
    })
}
```

Find every call to `formats::parse_optional(...)` (one per module — see `src/config/parse/modules/{battery,volume,network,clock,script,button,focused_window,workspaces}.rs`) and append the matching `crate::config::states::*` constant for each module. For example, in `battery.rs` change:

```rust
let format = formats::parse_optional(children, path, &ALLOWED_PLACEHOLDERS)?;
```

to:

```rust
let format = formats::parse_optional(
    children, path, &ALLOWED_PLACEHOLDERS, crate::config::states::BATTERY,
)?;
```

Use `crate::config::states::BATTERY` for battery, `VOLUME` for volume, `NETWORK` for network, `WORKSPACES` for workspaces, and `EMPTY` for clock, script, button, focused_window. Verify with `grep parse_optional src/config/parse/modules/` after edits — every call must pass five arguments.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib`
Expected: PASS for new tests; all existing format-template tests still pass.

- [ ] **Step 5: Commit (after user confirmation)**

```bash
git add src/config.rs src/config/parse/modules/
git commit -m "feat: parse segment child-node form for module formats"
```

---

## Task 6: KDL parser — segment style props (inline) and `style` children

**Files:**
- Modify: `src/config/parse/modules/segment_styles.rs`
- Test: `src/config/parse/tests.rs`

- [ ] **Step 1: Write the failing tests**

Add to `src/config/parse/tests.rs`:

```rust
#[test]
fn segment_inline_style_props() {
    let cfg = parse_module_for_test(r#"
        module "battery" {
            format {
                segment "{icon}" color="accent" size=18 weight="bold"
                segment "{value}%"
            }
        }
    "#).unwrap();
    let battery = cfg.battery();
    let segs = battery.format.collapsed.as_ref().unwrap().segments();
    use crate::config::style::{ColorValue, SegmentStyles, ThemeToken, Weight};
    match segs[0].styles() {
        SegmentStyles::Static(s) => {
            assert_eq!(s.color, Some(ColorValue::Token(ThemeToken::Accent)));
            assert_eq!(s.size, Some(18.0));
            assert_eq!(s.weight, Some(Weight::Bold));
        }
        other => panic!("expected Static, got {other:?}"),
    }
    matches!(segs[1].styles(), SegmentStyles::None);
}

#[test]
fn segment_stateful_style_children() {
    let cfg = parse_module_for_test(r#"
        module "battery" {
            format {
                segment "{icon}" {
                    style color="accent"
                    style state="critical" color="critical"
                    style state="charging" color="success"
                }
            }
        }
    "#).unwrap();
    let battery = cfg.battery();
    let segs = battery.format.collapsed.as_ref().unwrap().segments();
    use crate::config::style::SegmentStyles;
    match segs[0].styles() {
        SegmentStyles::Stateful { default, by_state } => {
            assert!(default.is_some());
            assert_eq!(by_state.len(), 2);
            assert_eq!(by_state[0].0, "critical");
            assert_eq!(by_state[1].0, "charging");
        }
        other => panic!("expected Stateful, got {other:?}"),
    }
}

#[test]
fn segment_inline_props_and_style_children_is_error() {
    let err = parse_module_for_test(r#"
        module "battery" {
            format {
                segment "{icon}" color="accent" {
                    style state="critical" color="critical"
                }
            }
        }
    "#).unwrap_err();
    let msg = format!("{err:#}");
    assert!(msg.contains("inline") || msg.contains("style children"), "got: {msg}");
}

#[test]
fn segment_two_default_styles_is_error() {
    let err = parse_module_for_test(r#"
        module "battery" {
            format {
                segment "{icon}" {
                    style color="accent"
                    style color="text"
                }
            }
        }
    "#).unwrap_err();
    assert!(format!("{err:#}").contains("default"));
}

#[test]
fn segment_duplicate_state_is_error() {
    let err = parse_module_for_test(r#"
        module "battery" {
            format {
                segment "{icon}" {
                    style state="critical" color="critical"
                    style state="critical" color="warning"
                }
            }
        }
    "#).unwrap_err();
    assert!(format!("{err:#}").contains("critical"));
}

#[test]
fn segment_unknown_state_is_error() {
    let err = parse_module_for_test(r#"
        module "battery" {
            format {
                segment "{icon}" {
                    style state="bogus" color="critical"
                }
            }
        }
    "#).unwrap_err();
    assert!(format!("{err:#}").contains("bogus"));
}

#[test]
fn segment_state_on_stateless_module_is_error() {
    let err = parse_module_for_test(r#"
        module "clock" {
            format {
                segment "{time}" {
                    style state="critical" color="critical"
                }
            }
        }
    "#).unwrap_err();
    assert!(format!("{err:#}").contains("state"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib segment_`
Expected: FAIL — style props not parsed.

- [ ] **Step 3: Replace the body of `segment_styles.rs`**

```rust
use ::kdl::{KdlDocument, KdlNode};

use crate::config::{StyledSegment, TextTemplate};
use crate::config::style::{ColorValue, SegmentStyles, Style, Weight};

pub(super) fn parse_segments(
    node: &KdlNode,
    allowed_placeholders: &[&str],
    allowed_states: &[&str],
    path: &str,
) -> anyhow::Result<Vec<StyledSegment>> {
    let mut out = Vec::new();

    for entry in node.entries() {
        if entry.name().is_some() {
            anyhow::bail!("{path}: unexpected named property on format node.");
        }
        let raw = entry.value().as_string()
            .ok_or_else(|| anyhow::anyhow!("{path}: expected string value."))?;
        out.push(StyledSegment::new(TextTemplate::parse(raw, allowed_placeholders, path)?));
    }

    if let Some(children) = node.children() {
        for (i, child) in children.nodes().iter().enumerate() {
            if child.name().value() != "segment" {
                anyhow::bail!("{path}: unexpected child node {:?}; expected `segment`.",
                    child.name().value());
            }
            let seg_path = format!("{path}.segment[{i}]");
            let mut seg = parse_segment(child, allowed_placeholders, allowed_states, &seg_path)?;
            out.push(std::mem::take(&mut seg));
        }
    }

    if out.is_empty() {
        anyhow::bail!("{path}: expected at least one segment.");
    }
    Ok(out)
}

fn parse_segment(
    node: &KdlNode,
    allowed_placeholders: &[&str],
    allowed_states: &[&str],
    path: &str,
) -> anyhow::Result<StyledSegment> {
    // Positional string is the template.
    let raw = node.entries().iter()
        .find(|e| e.name().is_none())
        .and_then(|e| e.value().as_string())
        .ok_or_else(|| anyhow::anyhow!("{path}: expected one positional string."))?;
    let template = TextTemplate::parse(raw, allowed_placeholders, path)?;

    // Inline style props (excluding the positional template arg).
    let inline = parse_style_props(node, path, /*allow_state=*/ false, allowed_states)?;

    let has_inline = !inline.is_empty();
    let has_children = node.children().is_some();

    if has_inline && has_children {
        anyhow::bail!("{path}: inline style props and `style` children are mutually exclusive.");
    }

    let mut segment = StyledSegment::new(template);
    if has_inline {
        segment.set_styles(SegmentStyles::Static(inline));
    } else if has_children {
        segment.set_styles(parse_style_children(node, allowed_states, path)?);
    }
    Ok(segment)
}

fn parse_style_props(
    node: &KdlNode,
    path: &str,
    allow_state: bool,
    allowed_states: &[&str],
) -> anyhow::Result<Style> {
    let mut style = Style::default();
    let mut state_seen = false;
    for entry in node.entries() {
        let Some(name) = entry.name() else { continue };
        match name.value() {
            "color" => {
                let raw = entry.value().as_string()
                    .ok_or_else(|| anyhow::anyhow!("{path}.color: expected string."))?;
                style.color = Some(ColorValue::parse(raw)
                    .map_err(|e| anyhow::anyhow!("{path}.color: {e}"))?);
            }
            "size" => {
                let f = entry.value().as_float().or_else(||
                    entry.value().as_integer().map(|i| i as f64));
                let f = f.ok_or_else(|| anyhow::anyhow!("{path}.size: expected number."))?;
                if !(f > 0.0) {
                    anyhow::bail!("{path}.size: must be positive, got {f}.");
                }
                style.size = Some(f as f32);
            }
            "weight" => {
                let raw = entry.value().as_string()
                    .ok_or_else(|| anyhow::anyhow!("{path}.weight: expected string."))?;
                style.weight = Some(Weight::parse(raw)
                    .map_err(|e| anyhow::anyhow!("{path}.weight: {e}"))?);
            }
            "state" if allow_state => {
                state_seen = true;
                // handled by caller
            }
            other => {
                anyhow::bail!("{path}: unknown property {other:?}.");
            }
        }
    }
    let _ = (state_seen, allowed_states); // silence unused when allow_state=false
    Ok(style)
}

fn parse_style_children(
    parent: &KdlNode,
    allowed_states: &[&str],
    path: &str,
) -> anyhow::Result<SegmentStyles> {
    let children = parent.children().expect("checked by caller");
    let mut default: Option<Style> = None;
    let mut by_state: Vec<(String, Style)> = Vec::new();

    for (i, child) in children.nodes().iter().enumerate() {
        if child.name().value() != "style" {
            anyhow::bail!("{path}: unexpected child {:?}; expected `style`.",
                child.name().value());
        }
        let style_path = format!("{path}.style[{i}]");
        let style = parse_style_props(child, &style_path, /*allow_state=*/ true, allowed_states)?;

        let state = child.entries().iter()
            .find(|e| e.name().map(|n| n.value()) == Some("state"))
            .map(|e| e.value().as_string()
                .ok_or_else(|| anyhow::anyhow!("{style_path}.state: expected string.")))
            .transpose()?
            .map(|s| s.to_string());

        match state {
            None => {
                if default.is_some() {
                    anyhow::bail!("{style_path}: duplicate default style for segment.");
                }
                default = Some(style);
            }
            Some(state) => {
                if allowed_states.is_empty() {
                    anyhow::bail!("{style_path}: this module does not support `state` selectors.");
                }
                if !allowed_states.contains(&state.as_str()) {
                    anyhow::bail!("{style_path}: unknown state {state:?}; expected one of {:?}.",
                        allowed_states);
                }
                if by_state.iter().any(|(k, _)| k == &state) {
                    anyhow::bail!("{style_path}: duplicate state {state:?} for segment.");
                }
                by_state.push((state, style));
            }
        }
    }

    Ok(SegmentStyles::Stateful { default, by_state })
}

impl Default for StyledSegment {
    fn default() -> Self {
        StyledSegment::new(TextTemplate::default())
    }
}
```

Add a `Default` derive (or impl) for `TextTemplate` if missing — check `src/config.rs:178-187` and add `Default` to its derive list if not already present:

```rust
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TextTemplate {
    parts: Vec<TemplatePart>,
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib`
Expected: PASS for all new and existing tests.

- [ ] **Step 5: Commit (after user confirmation)**

```bash
git add src/config/parse/modules/segment_styles.rs src/config.rs
git commit -m "feat: parse inline and stateful segment styles"
```

---

## Task 7: Battery — widen `{state}` vocabulary and emit state key

**Files:**
- Modify: `src/modules/battery.rs:95-124`
- Modify: any battery test that asserts the old `discharging` text (search with `grep -n discharging src/`)

- [ ] **Step 1: Write the failing test**

Add to `src/modules/battery.rs` `#[cfg(test)] mod tests` block (or wherever battery format tests live — search for `template.render_segments` in battery tests):

```rust
#[test]
fn battery_state_placeholder_uses_widened_vocabulary() {
    let module = test_battery_with_critical_threshold(15);
    let snap = BatterySnapshot { percentage: 5.0, is_charging: false };
    let segs = module.format_for_test(&snap, FormatMode::Collapsed,
        &[("state", "%state%".into())]);
    // segs[…] containing "{state}" rendering should now read "critical", not "discharging".
    assert!(segs.iter().any(|s| s.contains("critical")));
}
```

If this helper structure does not match the existing test code, read `src/modules/battery.rs` first and adapt — the assertion to add is: when a segment renders `{state}` against a 5% discharging battery, the substituted text is `critical`, not `discharging`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib battery_state`
Expected: FAIL — current code emits `"discharging"`.

- [ ] **Step 3: Replace the state computation in `src/modules/battery.rs`**

In `src/modules/battery.rs`, replace lines 111–115:

```rust
let state = if battery.is_charging {
    "charging".to_string()
} else {
    "discharging".to_string()
};
```

with:

```rust
let state = battery_state_key(battery, self.critical_threshold).to_string();
```

And add at the bottom of the file (or near `tier_icon`):

```rust
pub(crate) fn battery_state_key(b: &BatterySnapshot, critical_threshold: u8) -> &'static str {
    if b.is_charging {
        "charging"
    } else if b.percentage <= critical_threshold as f32 {
        "critical"
    } else if b.percentage <= (critical_threshold as f32) * 2.0 {
        "low"
    } else {
        "normal"
    }
}
```

(Use whatever the existing low/critical thresholds are — `low` is conventionally `2 × critical` in the prior styling code; verify by searching for `tier_icon` / `charge_tier` and use the same buckets if they already differ.)

- [ ] **Step 4: Run battery tests**

Run: `cargo test --lib battery`
Expected: PASS for the new test; existing battery tests pass (or are updated if any literally asserted `"discharging"` — update them).

- [ ] **Step 5: Commit (after user confirmation)**

```bash
git add src/modules/battery.rs
git commit -m "feat(battery): widen state vocabulary to charging/critical/low/normal"
```

---

## Task 8: Battery — render styled segments end-to-end

**Files:**
- Modify: `src/modules/battery.rs:90-125`
- Modify: `src/modules/view.rs` (whatever path renders battery's `Vec<String>` into widgets — find with `grep -n format_segments src/`)

- [ ] **Step 1: Write the failing test**

Add a battery test asserting that with a config like `format { segment "{icon}" { style color="accent"; style state="critical" color="critical" } }`, a 5% (critical) discharging battery yields a segment whose `ResolvedStyle.color == Some(theme::CRITICAL)`. Use whatever module-test scaffolding the prior format-templates tests use (read `src/modules/battery.rs` test module first).

- [ ] **Step 2: Run test to verify it fails**

Expected: FAIL — battery does not yet expose styled segments to its renderer.

- [ ] **Step 3: Extract icon computation into a helper and add a styled-render method**

In `src/modules/battery.rs`, extract the icon-resolution logic currently inlined in `format_segments_for_layout` (lines ~97–109) into a helper:

```rust
fn current_icon(&self, battery: &BatterySnapshot) -> String {
    if battery.is_charging {
        self.icons
            .icon_for_state_or_value(Some("charging"), Some(battery.percentage))
            .unwrap_or("⚡")
            .to_owned()
    } else {
        self.icons
            .icon_for_value(battery.percentage)
            .unwrap_or_else(|| tier_icon(charge_tier(battery.percentage, self.critical_threshold)))
            .to_owned()
    }
}
```

Replace the inlined block in the existing `format_segments_for_layout` with `let icon = self.current_icon(battery);`.

Then add the styled variant alongside it:

```rust
pub fn styled_segments_for_layout(
    &self,
    battery: &BatterySnapshot,
    mode: crate::config::FormatMode,
) -> Option<Vec<(String, crate::config::ResolvedStyle)>> {
    let icon = self.current_icon(battery);
    let value = format!("{}", battery.percentage.round() as u8);
    let state = battery_state_key(battery, self.critical_threshold);
    self.format.template_for(mode).map(|template| {
        template.render_styled_segments(
            &[
                ("icon", icon),
                ("value", value.clone()),
                ("percent", value),
                ("state", state.to_string()),
            ],
            Some(state),
        )
    })
}
```

- [ ] **Step 4: Wire view.rs through**

Find the call site in `src/modules/view.rs` that consumes the battery's `Vec<String>` and produce widgets. Add a parallel path that consumes `Vec<(String, ResolvedStyle)>` and applies the style to each iced text widget:

```rust
fn segment_widget(text: String, style: ResolvedStyle) -> iced::Element<'static, Msg> {
    let mut t = iced::widget::text(text);
    if let Some(size) = style.size { t = t.size(size); }
    if let Some(weight) = style.weight {
        let mut font = iced::Font::DEFAULT;
        font.weight = match weight {
            Weight::Light => iced::font::Weight::Light,
            Weight::Normal => iced::font::Weight::Normal,
            Weight::Bold => iced::font::Weight::Bold,
        };
        t = t.font(font);
    }
    if let Some(color) = style.color {
        t = t.style(move |_theme| iced::widget::text::Style { color: Some(color), ..Default::default() });
    }
    t.into()
}
```

Use this helper for any module that renders styled segments. Read `src/modules/view.rs` to find the right insertion point — the existing `render_segments`-consuming code is the model.

- [ ] **Step 5: Run battery tests**

Run: `cargo test --lib battery`
Expected: PASS.

- [ ] **Step 6: Commit (after user confirmation)**

```bash
git add src/modules/battery.rs src/modules/view.rs
git commit -m "feat(battery): render styled segments with state-driven coloring"
```

---

## Task 9: Volume — render styled segments

**Files:**
- Modify: `src/modules/volume.rs:90-100`
- Reuse: `segment_widget` helper from Task 8 in `src/modules/view.rs`

- [ ] **Step 1: Write the failing test**

Add a volume test: with a config that has `segment "{icon}" { style state="muted" color="overlay0" }`, a muted volume snapshot resolves to `Some(theme::OVERLAY0)`; an unmuted snapshot resolves to `None` (no override).

- [ ] **Step 2: Run test, expect FAIL**

Run: `cargo test --lib volume`

- [ ] **Step 3: Add a `styled_segments_for_layout` method on the volume module**

Same shape as Task 8. State key is `"muted"` if the volume is muted else `"unmuted"`. The `state` placeholder value passed into `render_styled_segments` and the `state` argument both use this string.

- [ ] **Step 4: Wire the view.rs call site**

Find the volume render path in `src/modules/view.rs`; switch it to consume styled segments and use `segment_widget`.

- [ ] **Step 5: Run volume tests**

Expected: PASS.

- [ ] **Step 6: Commit (after user confirmation)**

```bash
git add src/modules/volume.rs src/modules/view.rs
git commit -m "feat(volume): render styled segments"
```

---

## Task 10: Network — render styled segments

**Files:**
- Modify: `src/modules/network.rs:95-110`
- Reuse: `segment_widget`

- [ ] **Step 1: Write the failing test**

Test that with `style state="limited" color="warning"` and `style state="none" color="critical"`, a limited connectivity snapshot resolves to `theme::WARNING` and a none snapshot resolves to `theme::CRITICAL`.

- [ ] **Step 2: Run test, expect FAIL**

- [ ] **Step 3: Add `styled_segments_for_layout` on network**

State key is one of `"full" | "limited" | "none"`, matching the same logic the network module already uses to compute `{state}`.

- [ ] **Step 4: Wire view.rs call site**

- [ ] **Step 5: Run network tests**

Expected: PASS.

- [ ] **Step 6: Commit (after user confirmation)**

```bash
git add src/modules/network.rs src/modules/view.rs
git commit -m "feat(network): render styled segments"
```

---

## Task 11: Workspaces — render styled segments per item

**Files:**
- Modify: `src/modules/workspaces.rs:115-135`
- Reuse: `segment_widget`

Workspaces is per-item: each workspace produces its own segments and its own state. State key per item is one of `"focused" | "active" | "inactive"` based on the item's existing logic.

- [ ] **Step 1: Write the failing test**

Test that for a workspace list of three items (focused, active, inactive), with `segment "{label}" { style state="focused" color="accent" }`, only the focused item's resolved color is `theme::ACCENT`.

- [ ] **Step 2: Run test, expect FAIL**

- [ ] **Step 3: Switch the per-item render path to `render_styled_segments(values, Some(state))`**

The current code uses `render_joined`; change to `render_styled_segments` and produce one styled-segment widget per template segment per item.

- [ ] **Step 4: Wire view.rs call site**

- [ ] **Step 5: Run workspaces tests**

Expected: PASS.

- [ ] **Step 6: Commit (after user confirmation)**

```bash
git add src/modules/workspaces.rs src/modules/view.rs
git commit -m "feat(workspaces): render styled segments per item"
```

---

## Task 12: Stateless modules — clock, script, button, focused-window

**Files:**
- Modify: `src/modules/{clock,script,button,focused_window}.rs` (each one: replace `render_joined` with `render_styled_segments(values, None)` where the module is being styled)

These modules have no state vocabulary, so they only support `Static` styles. Each one needs:

- [ ] **Step 1: Write the failing test (one per module)**

For each module, test that an inline-styled segment (e.g. `segment "{value}" color="accent" weight="bold"`) renders with that color and weight.

- [ ] **Step 2: Run tests, expect FAIL**

Run: `cargo test --lib clock script button focused_window`

- [ ] **Step 3: Add `styled_segments_for_layout` on each module**

Same shape as Task 8 but pass `None` for the `state` argument. Reuse `segment_widget` in `src/modules/view.rs`.

- [ ] **Step 4: Wire view.rs call sites**

- [ ] **Step 5: Run module tests**

Expected: PASS.

- [ ] **Step 6: Commit (after user confirmation)**

```bash
git add src/modules/clock.rs src/modules/script.rs src/modules/button.rs src/modules/focused_window.rs src/modules/view.rs
git commit -m "feat(stateless modules): render styled segments"
```

---

## Task 13: End-to-end integration test

**Files:**
- Create or extend: `src/config/parse/tests.rs` (whichever existing place houses end-to-end parse-and-render assertions)

- [ ] **Step 1: Write the test**

Parse a full battery module config:

```kdl
module "battery" {
    format {
        segment "{icon}" {
            style color="accent"
            style state="critical" color="critical"
            style state="charging" color="success"
        }
        segment "{value}%" weight="bold"
    }
}
```

Then drive `styled_segments_for_layout` with three `BatterySnapshot` cases (charging, critical-discharging, normal) and assert the resolved color of the icon segment for each, plus that the value segment is always bold.

- [ ] **Step 2: Run, expect PASS**

Run: `cargo test --lib end_to_end_battery_segment_styles`

- [ ] **Step 3: Commit (after user confirmation)**

```bash
git add src/config/parse/tests.rs
git commit -m "test: end-to-end battery segment styles"
```

---

## Task 14: Verification gate

- [ ] **Step 1: Build (debug)**

Run: `cargo build`
Expected: clean build, zero warnings.

- [ ] **Step 2: Full test suite**

Run: `cargo test`
Expected: all tests pass.

- [ ] **Step 3: Clippy (if used in this repo)**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 4: Manual smoke test (user)**

Ask the user to run the bar locally with a sample config that exercises:
- battery `style state="critical"` while battery is below the critical threshold,
- volume `style state="muted"` while muted,
- one stateless module with inline color and weight.

Verify visually that the configured styles appear and that omitting `color` preserves the existing built-in state coloring.

- [ ] **Step 5: Update memory if any new feedback emerges**

If the user gives any guidance during smoke testing (e.g. preferred default colors, surprising fallback behavior), record it as a feedback memory.
