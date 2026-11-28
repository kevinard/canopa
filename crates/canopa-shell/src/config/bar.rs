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
}

/// Per-zone ordered list of module names for the bar layout.
#[derive(Debug, Clone, Default, PartialEq)]
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
            open_ms: 240.0,
            close_ms: 150.0,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PanelConfig {
    pub title: Option<String>,
    pub ui: canopa_theme::UiStyle,
    pub presentation: PanelPresentation,
    pub drawer_mode: DrawerMode,
    pub width: Option<u32>,
    pub launcher_width: Option<u32>,
    pub max_width: Option<u32>,
    pub height: Option<u32>,
    pub max_height: Option<u32>,
    pub modules: PanelModules,
    pub animation: PanelAnimationConfig,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PanelPresentation {
    #[default]
    Auto,
    Drawer,
    Adaptive,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DrawerMode {
    #[default]
    Overlay,
    PushBar,
}

/// Bar-level configuration: placement edge, size, layer, modules, and panel behavior.
#[derive(Debug, Clone, PartialEq)]
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
    pub length: BarLength,
    pub align: BarAlignment,
    pub margin: canopa_theme::Insets,
    pub reserve_space: bool,
    /// Whether the bar stays on screen or slides away until the pointer
    /// touches its screen edge.
    pub visibility: BarVisibility,
    pub ui: canopa_theme::UiStyle,
    /// Dwell delay (ms) before a module's tooltip appears on hover.
    /// Default 500. Hide is always immediate on hover-exit.
    pub tooltip_delay_ms: u32,
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
            length: BarLength::Fill,
            align: BarAlignment::Center,
            margin: Default::default(),
            reserve_space: true,
            visibility: BarVisibility::Always,
            ui: Default::default(),
            tooltip_delay_ms: 500,
            style: Style::default(),
            modules: BarModules::default(),
            actions: crate::config::ActionsConfig::default(),
            panel: PanelConfig::default(),
        }
    }
}

impl BarConfig {
    /// Space is reserved only for a bar that stays on screen.
    pub fn reserves_space(&self) -> bool {
        self.reserve_space && self.visibility == BarVisibility::Always
    }

    pub fn outer_margins(&self) -> (u32, u32, u32, u32) {
        let m = self.margin;
        match self.edge {
            Edge::Top => (m.top.ceil(), m.bottom.ceil(), m.left.ceil(), m.right.ceil()),
            Edge::Bottom => (m.bottom.ceil(), m.top.ceil(), m.left.ceil(), m.right.ceil()),
            Edge::Left => (m.left.ceil(), m.right.ceil(), m.top.ceil(), m.bottom.ceil()),
            Edge::Right => (m.right.ceil(), m.left.ceil(), m.top.ceil(), m.bottom.ceil()),
        }
    }

    /// A drawer is a vertical-bar presentation; horizontal bars keep automatic sizing.
    pub fn uses_drawer(&self) -> bool {
        self.panel.presentation != PanelPresentation::Auto && !self.edge.is_horizontal()
    }

    pub fn drawer_pushes_bar(&self) -> bool {
        self.uses_drawer() && self.panel.drawer_mode == DrawerMode::PushBar
    }

    pub fn uses_drawer_for(&self, content: &crate::surface::panel::PanelContent) -> bool {
        self.uses_drawer()
            && !(self.panel.presentation == PanelPresentation::Adaptive
                && matches!(content, crate::surface::panel::PanelContent::ModulePanel(_)))
    }

    pub fn matches_output(&self, output_name: &str) -> bool {
        self.output_target.matches(output_name)
    }
}

/// Whether a bar stays on screen (`always`) or hides until the pointer
/// reaches its screen edge (`auto-hide`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BarVisibility {
    #[default]
    Always,
    AutoHide,
}

impl BarVisibility {
    pub fn toggled(self) -> Self {
        match self {
            Self::Always => Self::AutoHide,
            Self::AutoHide => Self::Always,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BarLength {
    #[default]
    Fill,
    Pixels(u32),
    /// Hundredths of one percent, in 1..=10000.
    Percent(u32),
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BarAlignment {
    Start,
    #[default]
    Center,
    End,
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
#[derive(Debug, Clone, Default, PartialEq)]
pub enum Edge {
    #[default]
    Left,
    Right,
    Top,
    Bottom,
}

impl Edge {
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
