mod defaults;
mod parse;
pub mod style;

use anyhow::Context;
use iced_layershell::reexport::Anchor;
use std::collections::HashMap;

#[allow(unused_imports)]
pub use style::{ColorValue, ResolvedStyle, SegmentStyles, Style, ThemeToken, Weight};

fn is_known_config_module_type(module_type: &str) -> bool {
    debug_assert_eq!(
        crate::modules::MODULE_DESCRIPTORS.len(),
        crate::modules::ModuleKind::CONFIG_KINDS.len()
    );
    crate::modules::MODULE_DESCRIPTORS
        .iter()
        .any(|descriptor| descriptor.config_name == module_type)
}

/// Format strings for the clock module (AC: #4, #5).
#[derive(Debug, Clone)]
pub struct ClockConfig {
    pub time_format: String,
    pub date_format: String,
    pub format: FormatConfig,
    pub tooltip: TooltipSetting,
}

impl Default for ClockConfig {
    fn default() -> Self {
        ClockConfig {
            time_format: String::from("%H:%M"),
            date_format: String::from("%a %d %b"),
            format: FormatConfig::default(),
            tooltip: TooltipSetting::Default,
        }
    }
}

/// Command and polling interval for the script module.
#[derive(Debug, Clone)]
pub struct ScriptConfig {
    pub command: String,
    pub interval: u64, // seconds between executions
    pub format: FormatConfig,
    pub tooltip: TooltipSetting,
}

/// Configuration for the button module — all fields required.
#[derive(Debug, Clone)]
pub struct ButtonConfig {
    pub label: String,
    pub icon: String,
    pub on_click: String,
    pub format: FormatConfig,
    pub tooltip: TooltipSetting,
}

/// Per-workspace label and icon for the workspaces module.
#[derive(Debug, Clone)]
pub struct WorkspaceLabelConfig {
    pub label: Option<String>,
    pub icon: Option<String>,
}

/// Configuration for the workspaces module — optional label/icon per workspace number.
#[derive(Debug, Clone)]
pub struct WorkspacesConfig {
    /// Maps workspace number (1-based: ws.idx + 1) to label/icon config.
    /// Absent key → workspace shows number only.
    pub workspaces: std::collections::HashMap<u32, WorkspaceLabelConfig>,
    pub format: FormatConfig,
    pub tooltip: TooltipSetting,
}

impl Default for WorkspacesConfig {
    fn default() -> Self {
        WorkspacesConfig {
            workspaces: std::collections::HashMap::default(),
            format: FormatConfig::default(),
            tooltip: TooltipSetting::Default,
        }
    }
}

/// A single named state with an upper-bound threshold.
/// `value <= threshold` matches; the smallest matching threshold wins.
#[derive(Debug, Clone, PartialEq)]
pub struct StateRule {
    pub name: String,
    pub threshold: u8,
}

/// Ordered list of named state rules sorted by threshold ascending.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StatesConfig {
    pub rules: Vec<StateRule>,
}

impl StatesConfig {
    /// Smallest-threshold-that-covers-`value` lookup. Falls back to the
    /// highest-threshold rule when `value` exceeds every threshold.
    pub fn resolve(&self, value: f64) -> Option<&str> {
        let v = value.clamp(0.0, 100.0).round() as u8;
        self.rules
            .iter()
            .find(|rule| rule.threshold >= v)
            .or_else(|| self.rules.last())
            .map(|rule| rule.name.as_str())
    }

    pub fn names(&self) -> Vec<&str> {
        self.rules.iter().map(|r| r.name.as_str()).collect()
    }

    /// Replace the rule whose name matches `name`, or insert a new one.
    /// Re-sorts by threshold ascending.
    pub fn upsert(&mut self, name: String, threshold: u8) {
        if let Some(existing) = self.rules.iter_mut().find(|r| r.name == name) {
            existing.threshold = threshold;
        } else {
            self.rules.push(StateRule { name, threshold });
        }
        self.rules.sort_by_key(|r| r.threshold);
    }

    /// Threshold for the rule named `name`, if any.
    pub fn threshold_of(&self, name: &str) -> Option<u8> {
        self.rules
            .iter()
            .find(|r| r.name == name)
            .map(|r| r.threshold)
    }

    pub fn battery_default() -> Self {
        StatesConfig {
            rules: vec![
                StateRule {
                    name: "critical".into(),
                    threshold: 15,
                },
                StateRule {
                    name: "low".into(),
                    threshold: 30,
                },
                StateRule {
                    name: "normal".into(),
                    threshold: 100,
                },
            ],
        }
    }

    pub fn volume_default() -> Self {
        StatesConfig {
            rules: vec![StateRule {
                name: "unmuted".into(),
                threshold: 100,
            }],
        }
    }
}

/// Configuration for the battery module.
#[derive(Debug, Clone)]
pub struct BatteryConfig {
    /// Percentage at or below which the critical badge and red icon appear.
    /// Derived from `states` (the threshold of the "critical" rule). Falls back
    /// to 15 when the user removed the "critical" state entirely.
    pub critical_threshold: u8,
    pub states: StatesConfig,
    pub icons: IconRulesConfig,
    pub format: FormatConfig,
    pub tooltip: TooltipSetting,
}

impl Default for BatteryConfig {
    fn default() -> Self {
        let states = StatesConfig::battery_default();
        BatteryConfig {
            critical_threshold: states.threshold_of("critical").unwrap_or(15),
            states,
            icons: IconRulesConfig::default(),
            format: FormatConfig::default(),
            tooltip: TooltipSetting::Default,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct IconRulesConfig {
    pub states: HashMap<String, String>,
    pub thresholds: Vec<IconThreshold>,
}

impl IconRulesConfig {
    pub fn icon_for_state(&self, state: &str) -> Option<&str> {
        self.states.get(state).map(String::as_str)
    }

    pub fn icon_for_value(&self, value: f64) -> Option<&str> {
        self.thresholds
            .iter()
            .rev()
            .find(|threshold| value >= threshold.at)
            .map(|threshold| threshold.icon.as_str())
    }

    pub fn icon_for_state_or_value(&self, state: Option<&str>, value: Option<f64>) -> Option<&str> {
        state
            .and_then(|state| self.icon_for_state(state))
            .or_else(|| value.and_then(|value| self.icon_for_value(value)))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct IconThreshold {
    pub at: f64,
    pub icon: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FormatConfig {
    pub collapsed: Option<FormatTemplate>,
    pub expanded: Option<FormatTemplate>,
    pub horizontal: Option<FormatTemplate>,
}

impl FormatConfig {
    pub fn template_for(&self, mode: FormatMode) -> Option<&FormatTemplate> {
        match mode {
            FormatMode::Collapsed => self.collapsed.as_ref(),
            FormatMode::Expanded => self.expanded.as_ref().or(self.collapsed.as_ref()),
            FormatMode::Horizontal => self
                .horizontal
                .as_ref()
                .or(self.expanded.as_ref())
                .or(self.collapsed.as_ref()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TooltipSetting {
    Default,
    Disabled,
    Custom(FormatTemplate),
}

impl Default for TooltipSetting {
    fn default() -> Self {
        TooltipSetting::Default
    }
}

impl TooltipSetting {
    pub fn resolve(&self, default_template: Option<FormatTemplate>) -> Option<FormatTemplate> {
        match self {
            TooltipSetting::Default => default_template,
            TooltipSetting::Disabled => None,
            TooltipSetting::Custom(t) => Some(t.clone()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatMode {
    Collapsed,
    Expanded,
    Horizontal,
}

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
        StyledSegment {
            template,
            styles: SegmentStyles::None,
        }
    }

    #[cfg(test)]
    pub fn styles(&self) -> &SegmentStyles {
        &self.styles
    }
    pub fn set_styles(&mut self, styles: SegmentStyles) {
        self.styles = styles;
    }
}

impl FormatTemplate {
    #[cfg(test)]
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

    #[cfg(test)]
    pub fn segments(&self) -> &[StyledSegment] {
        &self.segments
    }

    #[cfg(test)]
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

    #[cfg(test)]
    pub fn render_styled_segments(
        &self,
        values: &[(&str, String)],
        state: Option<&str>,
    ) -> Vec<(String, ResolvedStyle)> {
        self.render_styled_segments_with_default(values, state, &Style::default())
    }

    pub fn render_styled_segments_with_default(
        &self,
        values: &[(&str, String)],
        state: Option<&str>,
        default_style: &Style,
    ) -> Vec<(String, ResolvedStyle)> {
        self.segments
            .iter()
            .map(|s| {
                (
                    s.template.render_pairs(values),
                    s.styles
                        .style_for(state)
                        .merge_over(*default_style)
                        .resolve(),
                )
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextTemplate {
    parts: Vec<TemplatePart>,
}

#[derive(Debug, Clone, PartialEq)]
enum TemplatePart {
    Literal(String),
    Placeholder(String),
}

impl TextTemplate {
    pub fn parse(raw: &str, allowed: &[&str], path: &str) -> anyhow::Result<Self> {
        if raw.is_empty() {
            anyhow::bail!("{path}: value must not be empty.");
        }

        let mut parts = Vec::new();
        let mut literal = String::new();
        let mut chars = raw.chars().peekable();

        while let Some(ch) = chars.next() {
            match ch {
                '{' => {
                    if chars.peek() == Some(&'{') {
                        chars.next();
                        literal.push('{');
                        continue;
                    }
                    if !literal.is_empty() {
                        parts.push(TemplatePart::Literal(std::mem::take(&mut literal)));
                    }

                    let mut name = String::new();
                    let mut closed = false;
                    for inner in chars.by_ref() {
                        if inner == '}' {
                            closed = true;
                            break;
                        }
                        name.push(inner);
                    }

                    if !closed {
                        anyhow::bail!("{path}: unmatched '{{' in format template.");
                    }
                    if name.is_empty() {
                        anyhow::bail!("{path}: empty placeholder is not allowed.");
                    }
                    if !name
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                    {
                        anyhow::bail!("{path}: invalid placeholder {name:?}.");
                    }
                    if !allowed.contains(&name.as_str()) {
                        anyhow::bail!("{path}: unknown placeholder {name:?}.");
                    }
                    parts.push(TemplatePart::Placeholder(name));
                }
                '}' => {
                    if chars.peek() == Some(&'}') {
                        chars.next();
                        literal.push('}');
                    } else {
                        anyhow::bail!("{path}: unmatched '}}' in format template.");
                    }
                }
                _ => literal.push(ch),
            }
        }

        if !literal.is_empty() {
            parts.push(TemplatePart::Literal(literal));
        }

        Ok(TextTemplate { parts })
    }

    #[cfg(test)]
    pub fn render(&self, values: &HashMap<&str, &str>) -> String {
        let mut rendered = String::new();
        for part in &self.parts {
            match part {
                TemplatePart::Literal(value) => rendered.push_str(value),
                TemplatePart::Placeholder(name) => {
                    if let Some(value) = values.get(name.as_str()) {
                        rendered.push_str(value);
                    }
                }
            }
        }
        rendered
    }

    pub fn render_pairs(&self, values: &[(&str, String)]) -> String {
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
}

/// Configuration for the network module.
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    pub icons: IconRulesConfig,
    pub format: FormatConfig,
    pub tooltip: TooltipSetting,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        NetworkConfig {
            icons: IconRulesConfig::default(),
            format: FormatConfig::default(),
            tooltip: TooltipSetting::Default,
        }
    }
}

/// Configuration for the tray module.
#[derive(Debug, Clone)]
pub struct TrayConfig {
    pub tooltip: TooltipSetting,
}

impl Default for TrayConfig {
    fn default() -> Self {
        TrayConfig {
            tooltip: TooltipSetting::Default,
        }
    }
}

/// Configuration for the focused-window module (no user-configurable params).
#[derive(Debug, Clone)]
pub struct FocusedWindowConfig {
    pub format: FormatConfig,
    pub tooltip: TooltipSetting,
}

impl Default for FocusedWindowConfig {
    fn default() -> Self {
        FocusedWindowConfig {
            format: FormatConfig::default(),
            tooltip: TooltipSetting::Default,
        }
    }
}

/// Configuration for the volume module — scroll step in percent.
#[derive(Debug, Clone)]
pub struct VolumeConfig {
    /// Volume adjustment step per scroll event (percent, 1–100). Default: 5.
    pub step: u8,
    pub states: StatesConfig,
    pub icons: IconRulesConfig,
    pub format: FormatConfig,
    pub tooltip: TooltipSetting,
}

impl Default for VolumeConfig {
    fn default() -> Self {
        VolumeConfig {
            step: 5,
            states: StatesConfig::volume_default(),
            icons: IconRulesConfig::default(),
            format: FormatConfig::default(),
            tooltip: TooltipSetting::Default,
        }
    }
}

/// Module-specific parameters. Extensible for Stories 2.4–2.5.
#[derive(Debug, Clone)]
pub enum ModuleParams {
    None,
    Battery(BatteryConfig),
    Network(NetworkConfig),
    Volume(VolumeConfig),
    Clock(ClockConfig),
    Script(ScriptConfig),
    Button(ButtonConfig),
    Workspaces(WorkspacesConfig),
    Tray(TrayConfig),
    FocusedWindow(FocusedWindowConfig),
}

/// Shared user-triggered actions available on every config-declared module.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModuleActionsConfig {
    pub click: Option<String>,
    pub right_click: Option<String>,
}

/// A single module entry from the config, specifying name, type, and module params.
/// Zone placement is determined by `bar.modules`, not by this struct.
#[derive(Debug, Clone)]
pub struct ModuleConfig {
    /// Instance name — unique identifier for this module instance (e.g. "clock", "work-status").
    pub name: String,
    /// Module type — determines which module implementation to use (e.g. "clock", "script").
    /// Inferred from `name` when the name matches a known module type; otherwise explicit.
    pub module_type: String,
    pub params: ModuleParams,
    pub actions: ModuleActionsConfig,
}

/// Parsed configuration for niribar.
/// Fully populated before any surface is created (AR7).
#[derive(Debug, Clone)]
pub struct Config {
    /// All named bar sections. At least one is always present (falls back to defaults when
    /// no `bar` sections appear in the config file).
    pub bars: Vec<BarConfig>,
    pub modules: Vec<ModuleConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            bars: vec![BarConfig::default()],
            modules: vec![],
        }
    }
}

impl Config {
    /// Return all bar configs that should appear on the given output.
    ///
    /// A bar with `output == "all"` appears on every connected output.
    /// A bar with a specific output name appears only on that output.
    /// In AllScreens mode (`output_name == ""`), returns all bars with `output == "all"`.
    pub fn bars_for_output(&self, output_name: &str) -> Vec<&BarConfig> {
        self.bars
            .iter()
            .filter(|b| b.output == "all" || b.output == output_name)
            .collect()
    }
}

/// Per-zone ordered list of module names for the bar layout.
#[derive(Debug, Clone, Default)]
pub struct BarModules {
    pub top: Vec<String>,
    pub center: Vec<String>,
    pub bottom: Vec<String>,
}

/// Animation timing for expand/collapse transitions.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimationConfig {
    /// Duration of the expand animation in milliseconds. Default: 280.0.
    pub expand_ms: f32,
    /// Duration of the collapse animation in milliseconds. Default: 200.0.
    pub collapse_ms: f32,
}

impl Default for AnimationConfig {
    fn default() -> Self {
        AnimationConfig {
            expand_ms: 280.0,
            collapse_ms: 200.0,
        }
    }
}

/// Bar-level configuration: placement edge, width targets, and expand trigger.
#[derive(Debug, Clone)]
pub struct BarConfig {
    /// Instance name — unique identifier for this bar (e.g. "main", "top-strip").
    pub name: String,
    /// Wayland output name this bar targets, or `"all"` to appear on every output.
    /// Default: `"all"`.
    pub output: String,
    pub edge: Edge,
    /// Collapsed bar width, in **logical pixels**.
    /// The compositor multiplies this by the output scale factor for physical rendering (NFR11).
    pub collapsed_width: u32,
    /// Expanded bar width, in **logical pixels**.
    /// The compositor multiplies this by the output scale factor for physical rendering (NFR11).
    pub expanded_width: u32,
    /// Whether the expand/collapse overlay feature is enabled. Default: true.
    /// When false, the bar is permanently collapsed and all expand actions are ignored.
    pub expand: bool,
    /// What user action triggers expand/collapse. Default: Click.
    pub expand_trigger: ExpandTrigger,
    /// Dwell delay (ms) before hover expands the bar. Only used when expand_trigger = Hover.
    pub hover_delay: u64,
    /// Dwell delay (ms) before a module's tooltip appears on hover.
    /// Default 500. Hide is always immediate on hover-exit.
    pub tooltip_delay_ms: u32,
    /// Keybind for in-bar expand/collapse while the bar has keyboard focus.
    /// Also documents the compositor keybind the user should bind to `niribar --toggle-expand`
    /// for global (unfocused) toggling.
    pub expand_keybind: Option<String>,
    /// Animation timing for expand/collapse transitions.
    pub animation: AnimationConfig,
    /// Default text segment style for modules rendered in this bar.
    pub style: Style,
    /// Ordered module names per zone. Empty lists mean no modules in that zone.
    pub modules: BarModules,
}

impl Default for BarConfig {
    fn default() -> Self {
        BarConfig {
            name: "main".to_string(),
            output: "all".to_string(),
            edge: Edge::Left,
            collapsed_width: 48,
            expanded_width: 200,
            expand: true,
            expand_trigger: ExpandTrigger::Click,
            hover_delay: 0,
            tooltip_delay_ms: 500,
            expand_keybind: None,
            animation: AnimationConfig::default(),
            style: Style::default(),
            modules: BarModules::default(),
        }
    }
}

/// Which screen edge the bar is anchored to.
#[derive(Debug, Clone, Default)]
pub enum Edge {
    #[default]
    Left,
    Right,
    Top,
    Bottom,
}

impl Edge {
    /// Parse a KDL string value into an Edge variant.
    pub(crate) fn from_str(s: &str) -> anyhow::Result<Edge> {
        match s {
            "left" => Ok(Edge::Left),
            "right" => Ok(Edge::Right),
            "top" => Ok(Edge::Top),
            "bottom" => Ok(Edge::Bottom),
            other => {
                anyhow::bail!(
                    "bar.edge: unknown value {other:?}. Expected: one of: left, right, top, bottom."
                )
            }
        }
    }

    /// Returns `true` for edges that span the full screen width (top/bottom).
    pub fn is_horizontal(&self) -> bool {
        matches!(self, Edge::Top | Edge::Bottom)
    }

    /// Map this edge to iced_layershell anchor bits.
    ///
    /// Side edges (left/right) stretch the full screen height.
    /// Top/bottom edges stretch the full screen width.
    pub fn anchor(&self) -> Anchor {
        match self {
            Edge::Left => Anchor::Left | Anchor::Top | Anchor::Bottom,
            Edge::Right => Anchor::Right | Anchor::Top | Anchor::Bottom,
            Edge::Top => Anchor::Top | Anchor::Left | Anchor::Right,
            Edge::Bottom => Anchor::Bottom | Anchor::Left | Anchor::Right,
        }
    }
}

#[cfg(test)]
mod icon_rule_tests {
    use super::*;

    #[test]
    fn icon_rules_select_highest_threshold_at_or_below_value() {
        let rules = IconRulesConfig {
            states: std::collections::HashMap::new(),
            thresholds: vec![
                IconThreshold {
                    at: 10.0,
                    icon: "low".into(),
                },
                IconThreshold {
                    at: 50.0,
                    icon: "mid".into(),
                },
                IconThreshold {
                    at: 90.0,
                    icon: "high".into(),
                },
            ],
        };

        assert_eq!(rules.icon_for_value(65.0), Some("mid"));
    }

    #[test]
    fn icon_rules_return_none_below_lowest_threshold() {
        let rules = IconRulesConfig {
            states: std::collections::HashMap::new(),
            thresholds: vec![IconThreshold {
                at: 10.0,
                icon: "low".into(),
            }],
        };

        assert_eq!(rules.icon_for_value(9.0), None);
    }

    #[test]
    fn icon_rules_state_takes_precedence_over_value() {
        let mut states = std::collections::HashMap::new();
        states.insert("charging".into(), "plug".into());
        let rules = IconRulesConfig {
            states,
            thresholds: vec![IconThreshold {
                at: 10.0,
                icon: "low".into(),
            }],
        };

        assert_eq!(
            rules.icon_for_state_or_value(Some("charging"), Some(15.0)),
            Some("plug")
        );
    }
}

#[cfg(test)]
mod format_template_tests {
    use super::*;

    fn vars<'a>(pairs: &[(&'a str, &'a str)]) -> HashMap<&'a str, &'a str> {
        pairs.iter().copied().collect()
    }

    #[test]
    fn text_template_substitutes_value() {
        let template = TextTemplate::parse("{value}% left", &["value"], "test.format").unwrap();

        assert_eq!(template.render(&vars(&[("value", "42")])), "42% left");
    }

    #[test]
    fn text_template_substitutes_repeated_placeholders() {
        let template = TextTemplate::parse("{value}/{value}", &["value"], "test.format").unwrap();

        assert_eq!(template.render(&vars(&[("value", "42")])), "42/42");
    }

    #[test]
    fn text_template_escapes_literal_braces() {
        let template = TextTemplate::parse("{{{value}}}", &["value"], "test.format").unwrap();

        assert_eq!(template.render(&vars(&[("value", "42")])), "{42}");
    }

    #[test]
    fn text_template_rejects_unmatched_open_brace() {
        let err = TextTemplate::parse("{value", &["value"], "test.format").unwrap_err();

        assert!(err.to_string().contains("unmatched"));
    }

    #[test]
    fn text_template_rejects_empty_placeholder() {
        let err = TextTemplate::parse("{}", &["value"], "test.format").unwrap_err();

        assert!(err.to_string().contains("empty placeholder"));
    }

    #[test]
    fn text_template_rejects_disallowed_placeholder() {
        let err = TextTemplate::parse("{missing}", &["value"], "test.format").unwrap_err();

        assert!(err.to_string().contains("unknown placeholder"));
    }

    #[test]
    fn format_template_renders_multiple_segments() {
        let template =
            FormatTemplate::parse(&["{icon}", "{value}%"], &["icon", "value"], "test.format")
                .unwrap();

        assert_eq!(
            template.render_segments(&[("icon", "▓".to_string()), ("value", "42".to_string())]),
            vec!["▓".to_string(), "42%".to_string()]
        );
    }

    #[test]
    fn render_styled_segments_returns_resolved_style_per_segment() {
        use crate::config::style::{ColorValue, SegmentStyles, Style, ThemeToken};

        let mut tmpl =
            FormatTemplate::parse(&["{icon}", "{value}%"], &["icon", "value"], "test.format")
                .unwrap();
        tmpl.set_segment_styles(
            0,
            SegmentStyles::Static(Style {
                color: Some(ColorValue::Token(ThemeToken::Accent)),
                ..Default::default()
            }),
        );
        let rendered =
            tmpl.render_styled_segments(&[("icon", "▓".into()), ("value", "42".into())], None);
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
        tmpl.set_segment_styles(
            0,
            SegmentStyles::Stateful {
                default: Some(Style {
                    color: Some(ColorValue::Token(ThemeToken::Accent)),
                    ..Default::default()
                }),
                by_state: vec![(
                    "critical".into(),
                    Style {
                        color: Some(ColorValue::Token(ThemeToken::Critical)),
                        ..Default::default()
                    },
                )],
            },
        );
        let crit = tmpl.render_styled_segments(&[("icon", "x".into())], Some("critical"));
        assert_eq!(crit[0].1.color, Some(ThemeToken::Critical.resolve()));
        let norm = tmpl.render_styled_segments(&[("icon", "x".into())], Some("normal"));
        assert_eq!(norm[0].1.color, Some(ThemeToken::Accent.resolve()));
    }

    #[test]
    fn render_styled_segments_merges_default_style_under_segment_style() {
        use crate::config::style::{ColorValue, SegmentStyles, Style, ThemeToken, Weight};

        let mut tmpl =
            FormatTemplate::parse(&["{icon}", "{value}%"], &["icon", "value"], "test.format")
                .unwrap();
        tmpl.set_segment_styles(
            0,
            SegmentStyles::Static(Style {
                weight: Some(Weight::Bold),
                ..Default::default()
            }),
        );

        let default_style = Style {
            color: Some(ColorValue::Token(ThemeToken::Subtext)),
            size: Some(13.0),
            weight: Some(Weight::Light),
        };
        let rendered = tmpl.render_styled_segments_with_default(
            &[("icon", "x".into()), ("value", "42".into())],
            None,
            &default_style,
        );

        assert_eq!(rendered[0].1.color, Some(ThemeToken::Subtext.resolve()));
        assert_eq!(rendered[0].1.size, Some(13.0));
        assert_eq!(rendered[0].1.weight, Some(Weight::Bold));
        assert_eq!(rendered[1].1.color, Some(ThemeToken::Subtext.resolve()));
        assert_eq!(rendered[1].1.size, Some(13.0));
        assert_eq!(rendered[1].1.weight, Some(Weight::Light));
    }
}

/// Determines what user action triggers bar expand/collapse.
///
/// Hover and click are mutually exclusive: when `Hover` is set, click has no effect.
/// Default: `Click`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum ExpandTrigger {
    #[default]
    Click,
    Hover,
}

impl ExpandTrigger {
    pub(crate) fn from_str(s: &str) -> anyhow::Result<ExpandTrigger> {
        match s {
            "click" => Ok(ExpandTrigger::Click),
            "hover" => Ok(ExpandTrigger::Hover),
            other => anyhow::bail!(
                "bar.expand-trigger: unknown value {other:?}. \
                 Expected: one of: click, hover."
            ),
        }
    }
}

/// Return the path to the user config file, following XDG conventions.
///
/// Returns an error if neither `XDG_CONFIG_HOME` nor `HOME` is set or non-empty.
fn config_file_path() -> anyhow::Result<std::path::PathBuf> {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|_| {
            let home = std::env::var("HOME")
                .context("neither XDG_CONFIG_HOME nor HOME environment variable is set")?;
            anyhow::ensure!(!home.is_empty(), "HOME environment variable is empty");
            Ok::<_, anyhow::Error>(std::path::PathBuf::from(home).join(".config"))
        })?;
    Ok(base.join("niribar").join("config.kdl"))
}

/// Load configuration from disk or fall back to the embedded default.
///
/// Config is fully parsed before returning — no lazy evaluation (AR7).
pub fn load() -> anyhow::Result<Config> {
    let path = config_file_path()?;
    let content = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => defaults::DEFAULT_CONFIG.to_string(),
        Err(e) => {
            anyhow::bail!("failed to read config file {}: {e}", path.display());
        }
    };
    let config = parse::parse(&content)?;
    validate(&config)?;
    Ok(config)
}

/// Validate a fully-parsed config for constraint violations.
///
/// Returns an error with a spec-formatted message if any constraint is violated.
/// Called by `load()` after parsing. Invalid config → process exits with code 1 (via main).
pub(crate) fn validate(config: &Config) -> anyhow::Result<()> {
    // Duplicate bar name check
    let mut seen_bar_names = std::collections::HashSet::new();
    for bar in &config.bars {
        if !seen_bar_names.insert(bar.name.as_str()) {
            anyhow::bail!(
                "bar {:?}: duplicate name. Each bar section must have a unique name.",
                bar.name
            );
        }
    }

    // Per-bar width constraints
    for bar in &config.bars {
        if bar.collapsed_width < 2 {
            anyhow::bail!(
                "bar {:?}: collapsed-width: value {} is too small. Expected: 2 or greater \
                 (minimum 1px content + 1px border).",
                bar.name,
                bar.collapsed_width
            );
        }
        if bar.expanded_width < bar.collapsed_width {
            anyhow::bail!(
                "bar {:?}: expanded-width: value {} must be >= collapsed-width ({}). \
                 Expected: a width greater than or equal to the collapsed width.",
                bar.name,
                bar.expanded_width,
                bar.collapsed_width
            );
        }
    }

    for (i, mc) in config.modules.iter().enumerate() {
        if !is_known_config_module_type(&mc.module_type) {
            anyhow::bail!(
                "modules[{i}].type: unknown module type {:?}. \
                 Expected one of: battery, clock, network, volume, workspaces, focused-window, script, button, tray.",
                mc.module_type
            );
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    #[test]
    fn known_module_types_include_all_config_facing_names() {
        for name in [
            "battery",
            "button",
            "clock",
            "focused-window",
            "network",
            "script",
            "tray",
            "volume",
            "workspaces",
        ] {
            assert!(
                is_known_config_module_type(name),
                "missing known module type {name:?}"
            );
        }
    }

    #[test]
    fn parse_default_config_succeeds() {
        let config = parse::parse(defaults::DEFAULT_CONFIG).unwrap();
        assert_eq!(config.bars.len(), 1);
        assert_eq!(config.bars[0].collapsed_width, 48);
        assert_eq!(config.bars[0].expanded_width, 200);
        // UX-DR18: zone assignments
        assert_eq!(config.bars[0].modules.top, vec!["workspaces"]);
        assert_eq!(config.bars[0].modules.center, vec!["focused-window"]);
        assert_eq!(
            config.bars[0].modules.bottom,
            vec!["volume", "network", "battery", "clock"]
        );
        // All 6 default modules declared
        assert_eq!(config.modules.len(), 6);
        let names: Vec<&str> = config.modules.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"workspaces"));
        assert!(names.contains(&"focused-window"));
        assert!(names.contains(&"volume"));
        assert!(names.contains(&"network"));
        assert!(names.contains(&"battery"));
        assert!(names.contains(&"clock"));
    }

    #[test]
    fn missing_bar_section_uses_defaults() {
        // Empty input falls back to a single default bar.
        let config = parse::parse("").unwrap();
        assert_eq!(config.bars[0].collapsed_width, 48);
        assert_eq!(config.bars[0].expanded_width, 200);
    }

    #[test]
    fn edge_left_gives_correct_anchor() {
        let edge = Edge::Left;
        let anchor = edge.anchor();
        assert!(anchor.contains(Anchor::Left));
        assert!(anchor.contains(Anchor::Top));
        assert!(anchor.contains(Anchor::Bottom));
    }

    #[test]
    fn edge_from_str_known_values() {
        assert!(matches!(Edge::from_str("left").unwrap(), Edge::Left));
        assert!(matches!(Edge::from_str("right").unwrap(), Edge::Right));
        assert!(matches!(Edge::from_str("top").unwrap(), Edge::Top));
        assert!(matches!(Edge::from_str("bottom").unwrap(), Edge::Bottom));
    }

    #[test]
    fn edge_from_str_unknown_returns_error() {
        assert!(Edge::from_str("center").is_err());
    }

    #[test]
    fn validate_rejects_collapsed_width_below_2() {
        for w in [0u32, 1u32] {
            let mut config = Config::default();
            config.bars[0].collapsed_width = w;
            let err = validate(&config).unwrap_err();
            let msg = err.to_string();
            assert!(
                msg.contains("collapsed-width:"),
                "collapsed_width={w}: expected 'collapsed-width:' in message, got: {msg}"
            );
            assert!(
                msg.contains("Expected:"),
                "collapsed_width={w}: expected 'Expected:' in message, got: {msg}"
            );
        }
    }

    #[test]
    fn validate_accepts_collapsed_width_at_minimum() {
        let mut config = Config::default();
        config.bars[0].collapsed_width = 2;
        assert!(validate(&config).is_ok());
    }

    #[test]
    fn validate_unknown_module_name_errors() {
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "volumee".into(),
            module_type: "volumee".into(),
            params: ModuleParams::None,
            actions: crate::config::ModuleActionsConfig::default(),
        }];
        let err = validate(&config).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.starts_with("modules[0].type:"),
            "expected 'modules[0].type:' prefix, got: {msg}"
        );
        assert!(
            msg.contains("volumee"),
            "expected module name in error, got: {msg}"
        );
    }

    #[test]
    fn validate_known_module_name_passes() {
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "clock".into(),
            module_type: "clock".into(),
            params: ModuleParams::None,
            actions: crate::config::ModuleActionsConfig::default(),
        }];
        assert!(validate(&config).is_ok());
    }

    #[test]
    fn validate_tray_module_type_passes() {
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "tray".into(),
            module_type: "tray".into(),
            params: ModuleParams::Tray(TrayConfig::default()),
            actions: crate::config::ModuleActionsConfig::default(),
        }];
        assert!(validate(&config).is_ok());
    }

    #[test]
    fn validate_error_includes_expected_list() {
        let mut config = Config::default();
        config.modules = vec![ModuleConfig {
            name: "unknown".into(),
            module_type: "unknown".into(),
            params: ModuleParams::None,
            actions: crate::config::ModuleActionsConfig::default(),
        }];
        let err = validate(&config).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("Expected one of:"),
            "expected 'Expected one of:' in: {msg}"
        );
        assert!(
            msg.contains("clock"),
            "expected 'clock' in expected list: {msg}"
        );
    }

    #[test]
    fn validate_error_index_reflects_position() {
        let mut config = Config::default();
        config.modules = vec![
            ModuleConfig {
                name: "clock".into(),
                module_type: "clock".into(),
                params: ModuleParams::None,
                actions: crate::config::ModuleActionsConfig::default(),
            },
            ModuleConfig {
                name: "bad-module".into(),
                module_type: "bad-module".into(),
                params: ModuleParams::None,
                actions: crate::config::ModuleActionsConfig::default(),
            },
        ];
        let err = validate(&config).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.starts_with("modules[1].type:"),
            "expected 'modules[1].type:' prefix, got: {msg}"
        );
    }

    #[test]
    fn validate_rejects_expanded_width_less_than_collapsed() {
        let mut config = Config::default();
        config.bars[0].collapsed_width = 100;
        config.bars[0].expanded_width = 50; // less than collapsed
        let err = validate(&config).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("expanded-width:"),
            "expected 'expanded-width:' in message, got: {msg}"
        );
        assert!(
            msg.contains("Expected:"),
            "expected 'Expected:' in message, got: {msg}"
        );
    }

    #[test]
    fn validate_accepts_expanded_width_equal_to_collapsed() {
        let mut config = Config::default();
        config.bars[0].collapsed_width = 48;
        config.bars[0].expanded_width = 48; // equal is OK
        assert!(validate(&config).is_ok());
    }

    #[test]
    fn clock_config_default_formats() {
        let cfg = ClockConfig::default();
        assert_eq!(cfg.time_format, "%H:%M");
        assert_eq!(cfg.date_format, "%a %d %b");
    }

    #[test]
    fn module_params_clock_stores_config() {
        let params = ModuleParams::Clock(ClockConfig {
            time_format: "%I:%M %p".into(),
            date_format: "%Y-%m-%d".into(),
            format: FormatConfig::default(),
            tooltip: TooltipSetting::Default,
        });
        match params {
            ModuleParams::Clock(cfg) => {
                assert_eq!(cfg.time_format, "%I:%M %p");
                assert_eq!(cfg.date_format, "%Y-%m-%d");
            }
            _ => panic!("expected ModuleParams::Clock"),
        }
    }

    #[test]
    fn script_config_stores_command_and_interval() {
        let cfg = ScriptConfig {
            command: "echo hi".into(),
            interval: 10,
            format: FormatConfig::default(),
            tooltip: TooltipSetting::Default,
        };
        assert_eq!(cfg.command, "echo hi");
        assert_eq!(cfg.interval, 10);
    }

    #[test]
    fn module_params_script_stores_config() {
        let params = ModuleParams::Script(ScriptConfig {
            command: "date".into(),
            interval: 30,
            format: FormatConfig::default(),
            tooltip: TooltipSetting::Default,
        });
        match params {
            ModuleParams::Script(cfg) => {
                assert_eq!(cfg.command, "date");
                assert_eq!(cfg.interval, 30);
            }
            _ => panic!("expected ModuleParams::Script"),
        }
    }

    #[test]
    fn button_config_stores_fields() {
        let cfg = ButtonConfig {
            label: "Launch Terminal".into(),
            icon: ">".into(),
            on_click: "kitty".into(),
            format: FormatConfig::default(),
            tooltip: TooltipSetting::Default,
        };
        assert_eq!(cfg.label, "Launch Terminal");
        assert_eq!(cfg.icon, ">");
        assert_eq!(cfg.on_click, "kitty");
    }

    #[test]
    fn module_params_button_stores_config() {
        let params = ModuleParams::Button(ButtonConfig {
            label: "launch".into(),
            icon: ">".into(),
            on_click: "kitty".into(),
            format: FormatConfig::default(),
            tooltip: TooltipSetting::Default,
        });
        match params {
            ModuleParams::Button(cfg) => {
                assert_eq!(cfg.label, "launch");
                assert_eq!(cfg.icon, ">");
                assert_eq!(cfg.on_click, "kitty");
            }
            _ => panic!("expected ModuleParams::Button"),
        }
    }

    #[test]
    fn bar_config_default_expand_keybind_is_none() {
        assert!(BarConfig::default().expand_keybind.is_none());
    }

    #[test]
    fn bar_config_default_expand_is_true() {
        assert!(BarConfig::default().expand);
    }

    // --- Story 9.1: Named bar sections with output targeting ---

    #[test]
    fn bars_for_output_all_matches_any_output() {
        let config = Config::default(); // single bar "main" with output "all"
        let bars = config.bars_for_output("DP-1");
        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].name, "main");
    }

    #[test]
    fn bars_for_output_specific_name_matches_only_that_output() {
        let mut config = Config::default();
        config.bars = vec![
            BarConfig {
                name: "main".into(),
                output: "all".into(),
                ..BarConfig::default()
            },
            BarConfig {
                name: "side".into(),
                output: "DP-1".into(),
                ..BarConfig::default()
            },
        ];
        let bars = config.bars_for_output("DP-1");
        assert_eq!(bars.len(), 2); // both "main" (all) and "side" (DP-1)
        let bars_edp = config.bars_for_output("eDP-1");
        assert_eq!(bars_edp.len(), 1); // only "main"
    }

    #[test]
    fn bars_for_output_no_all_bars_specific_output_not_connected() {
        let mut config = Config::default();
        config.bars = vec![BarConfig {
            name: "side".into(),
            output: "DP-1".into(),
            ..BarConfig::default()
        }];
        let bars = config.bars_for_output("eDP-1");
        assert!(bars.is_empty());
    }

    #[test]
    fn bars_for_output_empty_string_matches_only_all() {
        // AllScreens mode: output_name is ""
        let config = Config::default();
        let bars = config.bars_for_output("");
        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].output, "all");
    }

    #[test]
    fn validate_duplicate_bar_name_errors() {
        let config = Config {
            bars: vec![
                BarConfig {
                    name: "main".into(),
                    output: "all".into(),
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "main".into(),
                    output: "DP-1".into(),
                    ..BarConfig::default()
                },
            ],
            modules: vec![],
        };
        let err = validate(&config).unwrap_err();
        assert!(err.to_string().contains("duplicate name"), "got: {err}");
    }

    #[test]
    fn validate_bar_collapsed_width_too_small_errors() {
        let config = Config {
            bars: vec![BarConfig {
                name: "x".into(),
                output: "all".into(),
                collapsed_width: 1,
                ..BarConfig::default()
            }],
            modules: vec![],
        };
        let err = validate(&config).unwrap_err();
        assert!(err.to_string().contains("collapsed-width"), "got: {err}");
    }

    // --- Additional Story 9.1 config tests ---

    #[test]
    fn bar_config_default_name_is_main() {
        assert_eq!(BarConfig::default().name, "main");
    }

    #[test]
    fn bar_config_default_output_is_all() {
        assert_eq!(BarConfig::default().output, "all");
    }

    #[test]
    fn config_default_has_one_bar() {
        assert_eq!(Config::default().bars.len(), 1);
    }

    #[test]
    fn config_default_bar_name_is_main() {
        assert_eq!(Config::default().bars[0].name, "main");
    }

    #[test]
    fn config_default_bar_output_is_all() {
        assert_eq!(Config::default().bars[0].output, "all");
    }

    #[test]
    fn bars_for_output_all_targets_any_named_output() {
        let config = Config::default();
        // "all" should match any output name
        assert_eq!(config.bars_for_output("eDP-1").len(), 1);
        assert_eq!(config.bars_for_output("HDMI-A-1").len(), 1);
        assert_eq!(config.bars_for_output("DP-2").len(), 1);
    }

    #[test]
    fn bars_for_output_multiple_all_bars_returns_all() {
        let config = Config {
            bars: vec![
                BarConfig {
                    name: "a".into(),
                    output: "all".into(),
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "b".into(),
                    output: "all".into(),
                    ..BarConfig::default()
                },
            ],
            modules: vec![],
        };
        let bars = config.bars_for_output("eDP-1");
        assert_eq!(bars.len(), 2);
    }

    #[test]
    fn bars_for_output_mixed_returns_both_matching() {
        let config = Config {
            bars: vec![
                BarConfig {
                    name: "main".into(),
                    output: "all".into(),
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "extra".into(),
                    output: "eDP-1".into(),
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "other".into(),
                    output: "DP-1".into(),
                    ..BarConfig::default()
                },
            ],
            modules: vec![],
        };
        let bars = config.bars_for_output("eDP-1");
        assert_eq!(bars.len(), 2); // "main" (all) + "extra" (eDP-1)
        let names: Vec<&str> = bars.iter().map(|b| b.name.as_str()).collect();
        assert!(names.contains(&"main"));
        assert!(names.contains(&"extra"));
        assert!(!names.contains(&"other"));
    }

    #[test]
    fn bars_for_output_preserves_order() {
        let config = Config {
            bars: vec![
                BarConfig {
                    name: "first".into(),
                    output: "all".into(),
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "second".into(),
                    output: "all".into(),
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "third".into(),
                    output: "all".into(),
                    ..BarConfig::default()
                },
            ],
            modules: vec![],
        };
        let bars = config.bars_for_output("DP-1");
        assert_eq!(bars[0].name, "first");
        assert_eq!(bars[1].name, "second");
        assert_eq!(bars[2].name, "third");
    }

    #[test]
    fn validate_multiple_bars_no_duplicate_names_passes() {
        let config = Config {
            bars: vec![
                BarConfig {
                    name: "main".into(),
                    output: "all".into(),
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "side".into(),
                    output: "DP-1".into(),
                    ..BarConfig::default()
                },
            ],
            modules: vec![],
        };
        assert!(validate(&config).is_ok());
    }

    #[test]
    fn validate_duplicate_name_error_includes_bar_name() {
        let config = Config {
            bars: vec![
                BarConfig {
                    name: "side".into(),
                    output: "all".into(),
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "side".into(),
                    output: "DP-1".into(),
                    ..BarConfig::default()
                },
            ],
            modules: vec![],
        };
        let err = validate(&config).unwrap_err();
        assert!(err.to_string().contains("side"), "got: {err}");
    }

    #[test]
    fn validate_second_bar_expanded_less_than_collapsed_errors() {
        let config = Config {
            bars: vec![
                BarConfig {
                    name: "main".into(),
                    ..BarConfig::default()
                },
                BarConfig {
                    name: "side".into(),
                    output: "DP-1".into(),
                    collapsed_width: 100,
                    expanded_width: 50,
                    ..BarConfig::default()
                },
            ],
            modules: vec![],
        };
        let err = validate(&config).unwrap_err();
        assert!(
            err.to_string().contains("side"),
            "error should name the offending bar: {err}"
        );
        assert!(err.to_string().contains("expanded-width"), "got: {err}");
    }

    #[test]
    fn bars_for_output_case_sensitive() {
        let config = Config {
            bars: vec![BarConfig {
                name: "main".into(),
                output: "dp-1".into(),
                ..BarConfig::default()
            }],
            modules: vec![],
        };
        // "DP-1" (uppercase) does not match "dp-1" (lowercase)
        assert!(config.bars_for_output("DP-1").is_empty());
        assert_eq!(config.bars_for_output("dp-1").len(), 1);
    }

    #[test]
    fn state_selectors_per_module() {
        use crate::config::states;
        assert_eq!(states::BATTERY, &["charging", "critical", "low", "normal"]);
        assert_eq!(states::VOLUME, &["muted", "unmuted"]);
        assert_eq!(states::NETWORK, &["full", "limited", "none"]);
        assert_eq!(states::WORKSPACES, &["focused", "active", "inactive"]);
        assert!(states::EMPTY.is_empty());
    }
}

pub mod states {
    #[cfg(test)]
    pub const BATTERY: &[&str] = &["charging", "critical", "low", "normal"];
    #[cfg(test)]
    pub const VOLUME: &[&str] = &["muted", "unmuted"];
    pub const NETWORK: &[&str] = &["full", "limited", "none"];
    pub const WORKSPACES: &[&str] = &["focused", "active", "inactive"];
    pub const EMPTY: &[&str] = &[];
}
