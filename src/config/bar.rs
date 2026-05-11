use iced_layershell::reexport::Anchor;

use super::style::Style;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum OutputTarget {
    #[default]
    All,
    Include(Vec<String>),
    Exclude(Vec<String>),
}

impl OutputTarget {
    pub fn from_entries(entries: Vec<String>, path: &str) -> anyhow::Result<Self> {
        if entries.is_empty() {
            anyhow::bail!("{path}: expected at least one Output name.");
        }
        if entries.len() == 1 && entries[0] == "all" {
            return Ok(OutputTarget::All);
        }
        if entries.iter().any(|entry| entry == "all") {
            anyhow::bail!("{path}: `all` cannot be mixed with exact Output Target entries.");
        }

        let has_positive = entries.iter().any(|entry| !entry.starts_with('!'));
        let has_negative = entries.iter().any(|entry| entry.starts_with('!'));
        if has_positive && has_negative {
            anyhow::bail!(
                "{path}: cannot mix include and exclude Output Target entries in one list."
            );
        }

        let mut seen = std::collections::HashSet::new();
        let mut normalized = Vec::with_capacity(entries.len());
        for entry in entries {
            let name = entry.strip_prefix('!').unwrap_or(entry.as_str());
            if name.is_empty() {
                anyhow::bail!("{path}: Output Target entries must not be empty.");
            }
            if !seen.insert(name.to_string()) {
                anyhow::bail!("{path}: duplicate Output Target entry {name:?}.");
            }
            normalized.push(name.to_string());
        }

        if has_negative {
            Ok(OutputTarget::Exclude(normalized))
        } else {
            Ok(OutputTarget::Include(normalized))
        }
    }

    pub fn matches(&self, output_name: &str) -> bool {
        match self {
            OutputTarget::All => true,
            OutputTarget::Include(outputs) => outputs.iter().any(|output| output == output_name),
            OutputTarget::Exclude(outputs) => !outputs.iter().any(|output| output == output_name),
        }
    }

    pub fn is_all(&self) -> bool {
        matches!(self, OutputTarget::All)
    }
}

/// Per-zone ordered list of module names for the bar layout.
#[derive(Debug, Clone, Default)]
pub struct BarModules {
    pub start: Vec<String>,
    pub center: Vec<String>,
    pub end: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PanelModules {
    pub start: Vec<String>,
    pub center: Vec<String>,
    pub end: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PanelAnimationConfig {
    pub open_ms: f32,
    pub close_ms: f32,
}

impl Default for PanelAnimationConfig {
    fn default() -> Self {
        Self {
            open_ms: 180.0,
            close_ms: 120.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PanelConfig {
    pub width: u32,
    pub max_width: Option<u32>,
    pub height: Option<u32>,
    pub max_height: Option<u32>,
    pub modules: PanelModules,
    pub animation: PanelAnimationConfig,
}

impl Default for PanelConfig {
    fn default() -> Self {
        Self {
            width: 360,
            max_width: None,
            height: None,
            max_height: Some(600),
            modules: PanelModules::default(),
            animation: PanelAnimationConfig::default(),
        }
    }
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
    /// Exact Output Target expression controlling which Outputs host this Bar.
    pub output_target: OutputTarget,
    pub edge: Edge,
    /// Compositor stacking layer used by this Bar's Bar Surfaces.
    pub layer: BarLayer,
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
    pub actions: crate::config::ActionsConfig,
    pub panel: PanelConfig,
}

impl Default for BarConfig {
    fn default() -> Self {
        BarConfig {
            name: "main".to_string(),
            output_target: OutputTarget::All,
            edge: Edge::Left,
            layer: BarLayer::Top,
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
            actions: crate::config::ActionsConfig::default(),
            panel: PanelConfig::default(),
        }
    }
}

impl BarConfig {
    pub fn matches_output(&self, output_name: &str) -> bool {
        self.output_target.matches(output_name)
    }

    pub fn targets_all_outputs(&self) -> bool {
        self.output_target.is_all()
    }
}

/// Compositor stacking layer for a Bar Surface.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BarLayer {
    Background,
    Bottom,
    #[default]
    Top,
    Overlay,
}

impl BarLayer {
    pub fn layer_shell(self) -> iced_layershell::reexport::Layer {
        match self {
            BarLayer::Background => iced_layershell::reexport::Layer::Background,
            BarLayer::Bottom => iced_layershell::reexport::Layer::Bottom,
            BarLayer::Top => iced_layershell::reexport::Layer::Top,
            BarLayer::Overlay => iced_layershell::reexport::Layer::Overlay,
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
    #[cfg(test)]
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
