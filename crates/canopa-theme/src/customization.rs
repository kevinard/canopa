use crate::{Appearance, Rgb};

/// Logical pixels with millipixel precision. Integer storage gives stable equality
/// for reloads and excludes NaN from installed appearance snapshots.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pixels(u32);

impl Pixels {
    pub const ZERO: Self = Self(0);
    pub const fn whole(value: u32) -> Self {
        Self(value * 1000)
    }
    pub fn from_number(value: f64, max: f64) -> Option<Self> {
        (value.is_finite() && value >= 0.0 && value <= max)
            .then(|| Self((value * 1000.0).round() as u32))
    }
    pub fn px(self) -> f32 {
        self.0 as f32 / 1000.0
    }
    pub fn ceil(self) -> u32 {
        self.0.div_ceil(1000)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Insets {
    pub top: Pixels,
    pub right: Pixels,
    pub bottom: Pixels,
    pub left: Pixels,
}
impl Insets {
    pub const fn all(value: u32) -> Self {
        Self::axes(value, value)
    }
    pub const fn axes(vertical: u32, horizontal: u32) -> Self {
        Self {
            top: Pixels::whole(vertical),
            bottom: Pixels::whole(vertical),
            left: Pixels::whole(horizontal),
            right: Pixels::whole(horizontal),
        }
    }
    pub fn horizontal(self) -> f32 {
        self.left.px() + self.right.px()
    }
    pub fn vertical(self) -> f32 {
        self.top.px() + self.bottom.px()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum ColorRole {
    Background,
    Surface,
    Raised,
    Text,
    Muted,
    Disabled,
    Border,
    Accent,
    Danger,
    Warning,
    Success,
    OnAccent,
    OnDanger,
    Focus,
    Spine,
    AccentSurface,
    Brand,
}
impl ColorRole {
    pub const ALL: [Self; 17] = [
        Self::Background,
        Self::Surface,
        Self::Raised,
        Self::Text,
        Self::Muted,
        Self::Disabled,
        Self::Border,
        Self::Accent,
        Self::Danger,
        Self::Warning,
        Self::Success,
        Self::OnAccent,
        Self::OnDanger,
        Self::Focus,
        Self::Spine,
        Self::AccentSurface,
        Self::Brand,
    ];
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "background" | "base" => Self::Background,
            "surface" | "surface0" => Self::Surface,
            "raised" | "surface1" => Self::Raised,
            "text" => Self::Text,
            "muted" | "subtext" => Self::Muted,
            "disabled" | "overlay0" => Self::Disabled,
            "border" => Self::Border,
            "accent" => Self::Accent,
            "danger" | "critical" => Self::Danger,
            "warning" => Self::Warning,
            "success" => Self::Success,
            "on-accent" => Self::OnAccent,
            "on-danger" => Self::OnDanger,
            "focus" => Self::Focus,
            "spine" => Self::Spine,
            "accent-surface" => Self::AccentSurface,
            "brand" => Self::Brand,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorValue {
    Role(ColorRole),
    Rgba(Rgb, u8),
}
impl ColorValue {
    pub fn parse(value: &str) -> Option<Self> {
        if value == "transparent" {
            return Some(Self::Rgba(Rgb(0, 0, 0), 0));
        }
        if let Some(role) = ColorRole::parse(value) {
            return Some(Self::Role(role));
        }
        if value.len() == 9 && value.is_ascii() {
            return Some(Self::Rgba(
                Rgb::parse_hex(&value[..7])?,
                u8::from_str_radix(&value[7..], 16).ok()?,
            ));
        }
        Rgb::parse_hex(value).map(|rgb| Self::Rgba(rgb, 255))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum UiKind {
    Bar,
    Panel,
    Menu,
    Tooltip,
    Notification,
    Card,
    Module,
    Button,
    Input,
    MenuRow,
}
impl UiKind {
    pub const ALL: [Self; 10] = [
        Self::Bar,
        Self::Panel,
        Self::Menu,
        Self::Tooltip,
        Self::Notification,
        Self::Card,
        Self::Module,
        Self::Button,
        Self::Input,
        Self::MenuRow,
    ];
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "bar" => Self::Bar,
            "panel" => Self::Panel,
            "menu" => Self::Menu,
            "tooltip" => Self::Tooltip,
            "notification" => Self::Notification,
            "card" => Self::Card,
            "module" => Self::Module,
            "button" => Self::Button,
            "input" => Self::Input,
            "menu-row" => Self::MenuRow,
            _ => return None,
        })
    }
    pub fn is_control(self) -> bool {
        (self as usize) >= Self::Module as usize
    }
    pub fn has_gap(self) -> bool {
        !self.is_control() && self != Self::Tooltip
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StateColors {
    pub background: Option<ColorValue>,
    pub foreground: Option<ColorValue>,
    pub border_color: Option<ColorValue>,
}
impl StateColors {
    fn overlay(&mut self, other: Self) {
        self.background = other.background.or(self.background);
        self.foreground = other.foreground.or(self.foreground);
        self.border_color = other.border_color.or(self.border_color);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UiStyle {
    pub background: Option<ColorValue>,
    pub foreground: Option<ColorValue>,
    pub border_color: Option<ColorValue>,
    pub border_width: Option<Pixels>,
    pub radius: Option<[Pixels; 4]>,
    pub padding: Option<Insets>,
    pub margin: Option<Insets>,
    pub gap: Option<Pixels>,
    /// Selected, hover, pressed, disabled, focus (in composition order).
    pub states: [StateColors; 5],
}
impl UiStyle {
    pub fn overlay(&mut self, other: Self) {
        self.background = other.background.or(self.background);
        self.foreground = other.foreground.or(self.foreground);
        self.border_color = other.border_color.or(self.border_color);
        self.border_width = other.border_width.or(self.border_width);
        self.radius = other.radius.or(self.radius);
        self.padding = other.padding.or(self.padding);
        self.margin = other.margin.or(self.margin);
        self.gap = other.gap.or(self.gap);
        for (base, local) in self.states.iter_mut().zip(other.states) {
            base.overlay(local);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolvedStyle {
    pub background: ColorValue,
    pub foreground: ColorValue,
    pub border_color: ColorValue,
    pub border_width: Pixels,
    pub radius: [Pixels; 4],
    pub padding: Insets,
    pub margin: Insets,
    pub gap: Pixels,
    pub custom_border: bool,
    pub custom_radius: bool,
    pub custom_gap: bool,
    pub custom_padding: bool,
    pub states: [StateColors; 5],
}
impl ResolvedStyle {
    pub fn apply(mut self, style: UiStyle) -> Self {
        self.background = style.background.unwrap_or(self.background);
        self.foreground = style.foreground.unwrap_or(self.foreground);
        self.border_color = style.border_color.unwrap_or(self.border_color);
        self.border_width = style.border_width.unwrap_or(self.border_width);
        self.radius = style.radius.unwrap_or(self.radius);
        self.custom_padding |= style.padding.is_some();
        self.padding = style.padding.unwrap_or(self.padding);
        self.margin = style.margin.unwrap_or(self.margin);
        self.custom_gap |= style.gap.is_some();
        self.gap = style.gap.unwrap_or(self.gap);
        self.custom_border |= style.border_width.is_some() || style.border_color.is_some();
        self.custom_radius |= style.radius.is_some();
        for (base, local) in self.states.iter_mut().zip(style.states) {
            base.overlay(local);
        }
        self
    }
    pub fn state(
        mut self,
        selected: bool,
        hover: bool,
        pressed: bool,
        disabled: bool,
        focus: bool,
    ) -> Self {
        for (enabled, state) in [selected, hover, pressed, disabled, focus && !disabled]
            .into_iter()
            .zip(self.states)
        {
            if enabled {
                self.background = state.background.unwrap_or(self.background);
                self.foreground = state.foreground.unwrap_or(self.foreground);
                self.border_color = state.border_color.unwrap_or(self.border_color);
            }
        }
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Typography {
    pub family: Vec<String>,
    pub body_size: Pixels,
    pub label_size: Pixels,
    pub title_size: Pixels,
}
impl Default for Typography {
    fn default() -> Self {
        Self {
            family: vec!["Inter".into(), "sans-serif".into()],
            body_size: Pixels::whole(14),
            label_size: Pixels::whole(12),
            title_size: Pixels::whole(20),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Customization {
    pub palette: [Option<Rgb>; ColorRole::ALL.len()],
    pub typography: Typography,
    pub text_scale: Option<u16>,
    pub reduced_motion: bool,
    /// Maximum-contrast surfaces, text and borders (see `Palette`).
    pub high_contrast: bool,
    pub ui: [UiStyle; 10],
}

impl Appearance {
    pub const TEXT_SCALE_RANGE: std::ops::RangeInclusive<u16> = 100..=150;

    pub fn text_scale(&self) -> u16 {
        self.customization.text_scale.unwrap_or(100)
    }
    pub fn reduced_motion(&self) -> bool {
        self.customization.reduced_motion
    }
    pub fn high_contrast(&self) -> bool {
        self.customization.high_contrast
    }
    pub fn typography(&self) -> &Typography {
        &self.customization.typography
    }
    pub fn role(&self, role: ColorRole) -> Rgb {
        if let Some(color) = self.customization.palette[role as usize] {
            return color;
        }
        let p = self.palette();
        match role {
            ColorRole::Background => p.background,
            ColorRole::Surface => p.surface,
            ColorRole::Raised => p.raised,
            ColorRole::Text => p.text,
            ColorRole::Muted => p.muted,
            ColorRole::Disabled => p.disabled,
            ColorRole::Border => p.border,
            ColorRole::Accent => p.accent,
            ColorRole::Danger => p.danger,
            ColorRole::Warning => p.warning,
            ColorRole::Success => p.success,
            ColorRole::OnAccent if self.high_contrast() => p.accent.contrasting_text(),
            ColorRole::Spine if self.high_contrast() => p.background,
            ColorRole::AccentSurface if self.high_contrast() => p.raised,
            ColorRole::OnAccent => {
                if self.accent.is_some()
                    || self.customization.palette[ColorRole::Accent as usize].is_some()
                {
                    p.accent.contrasting_text()
                } else {
                    match self.color_scheme {
                        crate::ColorScheme::Dark => Rgb(0x18, 0x33, 0x2b),
                        crate::ColorScheme::Light => Rgb(0xf4, 0xfa, 0xf3),
                    }
                }
            }
            ColorRole::OnDanger => p.danger.contrasting_text(),
            ColorRole::Focus => p.accent,
            ColorRole::Spine => match self.color_scheme {
                crate::ColorScheme::Dark => Rgb(0x1b, 0x27, 0x2b),
                crate::ColorScheme::Light => Rgb(0xec, 0xf1, 0xeb),
            },
            ColorRole::AccentSurface => match self.color_scheme {
                crate::ColorScheme::Dark => Rgb(0x30, 0x4b, 0x41),
                crate::ColorScheme::Light => Rgb(0xe0, 0xee, 0xe1),
            },
            ColorRole::Brand => match self.color_scheme {
                crate::ColorScheme::Dark => Rgb(0xb9, 0xb4, 0xdc),
                crate::ColorScheme::Light => Rgb(0x71, 0x65, 0x9b),
            },
        }
    }
    pub fn style(&self, kind: UiKind) -> ResolvedStyle {
        use ColorRole::*;
        use ColorValue::Role;
        let transparent = ColorValue::Rgba(Rgb(0, 0, 0), 0);
        let (padding, radius, width, background, gap) = match kind {
            UiKind::Bar => (Insets::all(4), 0, 0, Role(Background), 4),
            UiKind::Panel => (Insets::all(24), 24, 1, Role(Surface), 8),
            UiKind::Menu => (Insets::all(8), 16, 1, Role(Surface), 4),
            UiKind::Tooltip => (Insets::axes(8, 12), 12, 1, Role(Raised), 0),
            UiKind::Notification => (Insets::all(16), 24, 1, Role(Surface), 8),
            UiKind::Card => (Insets::all(16), 16, 1, Role(Raised), 8),
            UiKind::Module => (Insets::all(2), 12, 0, transparent, 0),
            UiKind::Button => (Insets::axes(8, 12), 12, 1, Role(Raised), 0),
            UiKind::Input => (Insets::axes(10, 12), 12, 1, Role(Raised), 0),
            UiKind::MenuRow => (Insets::axes(10, 12), 12, 0, transparent, 0),
        };
        ResolvedStyle {
            background,
            foreground: Role(Text),
            border_color: Role(Border),
            border_width: Pixels::whole(width),
            radius: [Pixels::whole(radius); 4],
            padding,
            margin: Insets::default(),
            gap: Pixels::whole(gap),
            custom_border: false,
            custom_radius: false,
            custom_gap: false,
            custom_padding: false,
            states: Default::default(),
        }
        .apply(self.customization.ui[kind as usize])
    }
}
