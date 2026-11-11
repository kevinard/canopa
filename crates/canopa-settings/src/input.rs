use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Input {
    pub layouts: String,
    pub variants: String,
    pub options: String,
    pub repeat_delay: u16,
    pub repeat_rate: u8,
    pub mouse_speed: f64,
    pub mouse_left_handed: bool,
    pub mouse_natural_scroll: bool,
    pub touchpad_speed: f64,
    pub touchpad_tap: bool,
    pub touchpad_natural_scroll: bool,
    pub touchpad_disable_while_typing: bool,
    /// Leave the power button to logind's system setting instead of Niri's
    /// default (suspend). See the Power page.
    pub power_key_to_logind: bool,
    /// Niri's `cursor { xcursor-size }`; `None` keeps Niri's default (24).
    pub cursor_size: Option<u16>,
}
/// The cursor sizes Settings offers, in pixels.
pub const CURSOR_SIZES: [u16; 5] = [24, 32, 48, 64, 96];
impl Default for Input {
    fn default() -> Self {
        Self {
            layouts: "us".into(),
            variants: String::new(),
            options: String::new(),
            repeat_delay: 600,
            repeat_rate: 25,
            mouse_speed: 0.,
            mouse_left_handed: false,
            mouse_natural_scroll: false,
            touchpad_speed: 0.,
            touchpad_tap: true,
            touchpad_natural_scroll: true,
            touchpad_disable_while_typing: true,
            power_key_to_logind: false,
            cursor_size: None,
        }
    }
}
impl Input {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.layouts.is_empty() && self.layouts.split(',').count() <= 8,
            "choose one to eight keyboard layouts"
        );
        for value in [&self.layouts, &self.variants, &self.options] {
            ensure!(
                value.len() <= 256
                    && value
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"_,:-+".contains(&c)),
                "keyboard identifiers contain unsupported characters"
            );
        }
        ensure!(
            (100..=2000).contains(&self.repeat_delay) && (1..=100).contains(&self.repeat_rate),
            "repeat delay/rate are outside their supported range"
        );
        ensure!(
            self.cursor_size
                .is_none_or(|size| (16..=256).contains(&size)),
            "cursor size is outside its supported range"
        );
        ensure!(
            (-1.0..=1.0).contains(&self.mouse_speed) && (-1.0..=1.0).contains(&self.touchpad_speed),
            "pointer speed must be between -1 and 1"
        );
        let context = xkbcommon::xkb::Context::new(xkbcommon::xkb::CONTEXT_NO_FLAGS);
        ensure!(
            xkbcommon::xkb::Keymap::new_from_names(
                &context,
                "",
                "",
                &self.layouts,
                &self.variants,
                (!self.options.is_empty()).then(|| self.options.clone()),
                xkbcommon::xkb::KEYMAP_COMPILE_NO_FLAGS
            )
            .is_some(),
            "keyboard layouts, variants or options could not be compiled"
        );
        Ok(())
    }
    pub fn text(&self) -> Result<String> {
        use crate::storage::quote;
        self.validate()?;
        let flag = |name: &str, enabled: bool| {
            if enabled {
                format!("        {name}\n")
            } else {
                String::new()
            }
        };
        Ok(format!(
            "// Managed by Canopa Settings.\ninput {{\n{}    keyboard {{\n        xkb {{ layout {}; variant {}; options {}; }}\n        repeat-delay {}\n        repeat-rate {}\n    }}\n    mouse {{\n        accel-speed {}\n{}{}    }}\n    touchpad {{\n        accel-speed {}\n{}{}{}    }}\n}}\n",
            if self.power_key_to_logind {
                "    disable-power-key-handling\n"
            } else {
                ""
            },
            quote(&self.layouts),
            quote(&self.variants),
            quote(&self.options),
            self.repeat_delay,
            self.repeat_rate,
            self.mouse_speed,
            flag("left-handed", self.mouse_left_handed),
            flag("natural-scroll", self.mouse_natural_scroll),
            self.touchpad_speed,
            flag("tap", self.touchpad_tap),
            flag("natural-scroll", self.touchpad_natural_scroll),
            flag("dwt", self.touchpad_disable_while_typing)
        ) + &self
            .cursor_size
            .map(|size| format!("cursor {{\n    xcursor-size {size}\n}}\n"))
            .unwrap_or_default())
    }
    pub fn parse(text: &str) -> Result<Self> {
        let document = kdl::KdlDocument::parse_v1(text)?;
        ensure!(
            document.nodes().len() <= 2
                && document
                    .nodes()
                    .iter()
                    .all(|node| ["input", "cursor"].contains(&node.name().value())),
            "custom input includes are read-only; keep additional options in niri-custom.kdl"
        );
        let mut result = Self::default();
        if let Some(cursor) = document.get("cursor") {
            for field in cursor.children().map(|c| c.nodes()).unwrap_or_default() {
                ensure!(
                    field.name().value() == "xcursor-size",
                    "custom cursor settings are read-only; keep them in niri-custom.kdl"
                );
                let size = field
                    .get(0)
                    .and_then(|value| value.as_integer())
                    .context("invalid cursor size")?;
                result.cursor_size = Some(size.try_into()?);
            }
        }
        let Some(input) = document.get("input").and_then(|n| n.children()) else {
            return Ok(result);
        };
        result.touchpad_tap = false;
        result.touchpad_natural_scroll = false;
        result.touchpad_disable_while_typing = false;
        for section in input.nodes() {
            if section.name().value() == "disable-power-key-handling"
                && section.children().is_none()
            {
                result.power_key_to_logind = boolean(section.get(0))?;
                continue;
            }
            let Some(fields) = section.children() else {
                continue;
            };
            for field in fields.nodes() {
                let value = field.get(0);
                match (section.name().value(), field.name().value()) {
                    ("keyboard", "xkb") => {
                        if let Some(xkb) = field.children() {
                            ensure!(
                                xkb.nodes()
                                    .iter()
                                    .all(|node| ["layout", "variant", "options"]
                                        .contains(&node.name().value())),
                                "custom keyboard rules are read-only; keep them in niri-custom.kdl"
                            );
                            for (name, target) in [
                                ("layout", &mut result.layouts),
                                ("variant", &mut result.variants),
                                ("options", &mut result.options),
                            ] {
                                if let Some(value) = xkb
                                    .get(name)
                                    .and_then(|n| n.get(0))
                                    .and_then(|e| e.as_string())
                                {
                                    *target = value.into();
                                }
                            }
                        }
                    }
                    ("keyboard", "repeat-delay") => {
                        if let Some(value) = value.and_then(|v| v.as_integer()) {
                            result.repeat_delay = value.try_into()?;
                        }
                    }
                    ("keyboard", "repeat-rate") => {
                        if let Some(value) = value.and_then(|v| v.as_integer()) {
                            result.repeat_rate = value.try_into()?;
                        }
                    }
                    ("mouse", "accel-speed") => result.mouse_speed = number(value)?,
                    ("touchpad", "accel-speed") => result.touchpad_speed = number(value)?,
                    ("mouse", "left-handed") => result.mouse_left_handed = boolean(value)?,
                    ("mouse", "natural-scroll") => result.mouse_natural_scroll = boolean(value)?,
                    ("touchpad", "tap") => result.touchpad_tap = boolean(value)?,
                    ("touchpad", "natural-scroll") => {
                        result.touchpad_natural_scroll = boolean(value)?
                    }
                    ("touchpad", "dwt") => result.touchpad_disable_while_typing = boolean(value)?,
                    _ => anyhow::bail!(
                        "input include has custom settings; move them to niri-custom.kdl before editing"
                    ),
                }
            }
        }
        result.validate()?;
        Ok(result)
    }
}

pub fn apply(
    paths: &crate::session::Paths,
    source: &crate::storage::Snapshot,
    input: &Input,
) -> Result<crate::storage::Snapshot> {
    ensure!(paths.active(), "log into Canopa to save input settings");
    Input::parse(&source.text)?;
    let client = crate::niri::Client::discover()?;
    let saved = source.save(&input.text()?, |path| {
        crate::niri::validate_fragment(paths, "niri-input.kdl", path)
    })?;
    if let Err(error) = client.reload() {
        saved.save(&source.text, |_| Ok(()))?;
        return Err(error.context("input reload failed; previous settings restored"));
    }
    Ok(saved)
}
/// Change only who handles the power button, keeping the other input
/// settings, and reload Niri. Returns the saved include.
pub fn set_power_key(
    paths: &crate::session::Paths,
    to_logind: bool,
) -> Result<crate::storage::Snapshot> {
    let source = crate::storage::Snapshot::load(paths.input())?;
    let mut input = Input::parse(&source.text)?;
    input.power_key_to_logind = to_logind;
    apply(paths, &source, &input)
}
/// Change only the cursor size, keeping the other input settings, and
/// reload Niri, which also passes the size to new applications.
pub fn set_cursor_size(
    paths: &crate::session::Paths,
    size: Option<u16>,
) -> Result<crate::storage::Snapshot> {
    let source = crate::storage::Snapshot::load(paths.input())?;
    let mut input = Input::parse(&source.text)?;
    input.cursor_size = size;
    apply(paths, &source, &input)
}
fn boolean(value: Option<&kdl::KdlValue>) -> Result<bool> {
    match value {
        None => Ok(true),
        Some(kdl::KdlValue::Bool(value)) => Ok(*value),
        _ => anyhow::bail!("invalid input flag"),
    }
}
fn number(value: Option<&kdl::KdlValue>) -> Result<f64> {
    match value {
        Some(kdl::KdlValue::Float(value)) => Ok(*value),
        Some(kdl::KdlValue::Integer(value)) => Ok(*value as f64),
        _ => anyhow::bail!("invalid pointer speed"),
    }
}

/// Keyboard & pointer is a file-backed page over the managed input include.
impl crate::file_page::FileModel for Input {
    const DOCUMENT: crate::registry::Document = crate::registry::Document::Input;
    const READ_ONLY: &'static str = "input configuration is read-only";

    fn path(paths: &crate::session::Paths) -> std::path::PathBuf {
        paths.input()
    }
    fn read(source: &crate::storage::Snapshot) -> Result<Self> {
        Self::parse(&source.text)
    }
    fn fallback() -> Self {
        Self::default()
    }
    fn write(
        &self,
        paths: &crate::session::Paths,
        source: &crate::storage::Snapshot,
    ) -> Result<crate::storage::Snapshot> {
        apply(paths, source, self)
    }
}
