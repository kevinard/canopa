use crate::background::Fit;
use anyhow::{Result, bail, ensure};
use canopa_config::{Component, Document, Request};
use canopa_theme::{Appearance, Rgb};
use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
};

pub const PASSIVE_WIDGETS: &[&str] = &[
    "username",
    "avatar",
    "keyboard",
    "battery",
    "media",
    "weather",
    "notifications",
];

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Background {
    pub image: Option<PathBuf>,
    pub fit: Fit,
}
#[derive(Clone, Debug)]
pub struct Settings {
    pub background: Background,
    pub outputs: BTreeMap<String, Background>,
    pub hidden_widgets: BTreeMap<String, Vec<String>>,
    pub appearance: Appearance,
    pub color: Rgb,
    pub solid_color: bool,
    pub font: String,
    pub clock_format: String,
    pub date_format: String,
    pub widgets: Vec<String>,
    pub dim: u8,
    pub avatar: Option<PathBuf>,
    pub weather_file: Option<PathBuf>,
    pub clock_size: u32,
    pub text_size: u32,
    pub use_wallpaper: bool,
    pub auth_timeout: u64,
    pub layout: crate::layout::Layout,
    pub custom: BTreeMap<String, canopa_widgets::script::Script>,
}
impl Settings {
    pub fn load(request: &Request, component: Component) -> Result<(Self, Document)> {
        let document = Document::load(request, component, "")?;
        let value = Self::parse(
            document.selected(),
            document.appearance(),
            document.info().path.as_deref(),
            component,
        )?;
        Ok((value, document))
    }
    pub fn parse(
        nodes: &kdl::KdlDocument,
        appearance: Appearance,
        source: Option<&std::path::Path>,
        component: Component,
    ) -> Result<Self> {
        let mut value = Self {
            color: appearance.palette().background,
            solid_color: false,
            appearance: appearance.clone(),
            background: Background::default(),
            outputs: BTreeMap::new(),
            hidden_widgets: BTreeMap::new(),
            font: appearance
                .typography()
                .family
                .first()
                .cloned()
                .unwrap_or_else(|| "Inter".into()),
            clock_format: "%H:%M".into(),
            date_format: "%A, %B %-d".into(),
            widgets: vec![
                "battery".into(),
                "media".into(),
                "keyboard".into(),
                "username".into(),
            ],
            dim: 65,
            avatar: None,
            weather_file: None,
            clock_size: 80,
            text_size: 19,
            use_wallpaper: true,
            auth_timeout: 120,
            layout: Default::default(),
            custom: BTreeMap::new(),
        };
        if component == Component::Greeter {
            use crate::layout::{Anchor, Region};
            value.layout.margin = 42.;
            value.layout.gap = 18.;
            value.layout.regions = vec![
                Region {
                    anchor: Anchor::Center,
                    widgets: [
                        "clock",
                        "date",
                        "username",
                        "avatar",
                        "authentication",
                        "keyboard",
                    ]
                    .map(String::from)
                    .into(),
                    row: false,
                    x: 0.,
                    y: 0.,
                },
                Region {
                    anchor: Anchor::BottomCenter,
                    widgets: ["battery", "media", "weather"].map(String::from).into(),
                    row: true,
                    x: 0.,
                    y: 0.,
                },
            ];
            value.widgets = vec!["keyboard".into(), "battery".into()];
            value.use_wallpaper = false;
        }
        if component == Component::Lockscreen {
            value.layout.gap = 18.;
            value.layout.regions[0].y = 16.;
            value.layout.regions[0].widgets.push("keyboard".into());
            value.layout.regions[1]
                .widgets
                .retain(|name| name != "keyboard");
            // Notify's read-only lock-screen feed: critical items only by default.
            value.layout.regions[1]
                .widgets
                .insert(0, "notifications".into());
            value.widgets.push("notifications".into());
        }
        let mut explicit_layout = false;
        let mut seen = HashSet::new();
        for node in nodes.nodes() {
            let name = node.name().value();
            ensure!(
                node.ty().is_none(),
                "annotated session settings are unsupported"
            );
            if component != Component::Wallpaper && name == "layout" {
                ensure!(
                    !explicit_layout && node.entries().is_empty(),
                    "duplicate or invalid layout"
                );
                explicit_layout = true;
                value.layout = crate::layout::Layout::parse(
                    node.children()
                        .ok_or_else(|| anyhow::anyhow!("layout requires a block"))?,
                )?;
                continue;
            }
            if component != Component::Wallpaper && name == "widget" {
                let name = string(node)?;
                ensure!(
                    !name.is_empty()
                        && name.len() <= 64
                        && !matches!(name.as_str(), "clock" | "date" | "authentication")
                        && !PASSIVE_WIDGETS.contains(&name.as_str())
                        && !value.custom.contains_key(&name)
                        && value.custom.len() < 32,
                    "invalid, reserved or duplicate custom widget name"
                );
                value.custom.insert(name, parse_widget(node, source)?);
                continue;
            }
            if name == "output" {
                continue;
            }
            ensure!(seen.insert(name), "duplicate session setting");
            ensure!(
                node.children().is_none(),
                "unexpected session setting block"
            );
            match name {
                "auth-timeout" if component != Component::Wallpaper => {
                    ensure!(
                        node.entries().len() == 1
                            && node.entries()[0].name().is_none()
                            && node.entries()[0].ty().is_none(),
                        "auth-timeout requires an integer"
                    );
                    value.auth_timeout = node.entries()[0]
                        .value()
                        .as_integer()
                        .filter(|n| (5..=600).contains(n))
                        .ok_or_else(|| anyhow::anyhow!("auth-timeout must be 5..600 seconds"))?
                        as u64;
                }

                "avatar" | "weather-file" if component != Component::Wallpaper => {
                    let path = path(&string(node)?, source)?;
                    if name == "avatar" {
                        value.avatar = Some(path);
                    } else {
                        value.weather_file = Some(path);
                    }
                }
                "use-wallpaper" if component != Component::Wallpaper => {
                    ensure!(
                        node.entries().len() == 1,
                        "use-wallpaper requires one boolean"
                    );
                    let entry = &node.entries()[0];
                    ensure!(
                        entry.name().is_none() && entry.ty().is_none(),
                        "invalid boolean entry"
                    );
                    value.use_wallpaper = entry
                        .value()
                        .as_bool()
                        .ok_or_else(|| anyhow::anyhow!("use-wallpaper requires a boolean"))?;
                }
                "clock-size" | "text-size" if component != Component::Wallpaper => {
                    ensure!(node.entries().len() == 1, "size requires one integer");
                    let entry = &node.entries()[0];
                    ensure!(
                        entry.name().is_none() && entry.ty().is_none(),
                        "invalid size entry"
                    );
                    let size = entry
                        .value()
                        .as_integer()
                        .filter(|n| (8..=160).contains(n))
                        .ok_or_else(|| anyhow::anyhow!("size must be 8..160"))?
                        as u32;
                    if name == "clock-size" {
                        value.clock_size = size;
                    } else {
                        value.text_size = size;
                    }
                }
                "image" | "fit" => parse_background(&mut value.background, node, source)?,
                "color" => {
                    value.solid_color = true;
                    value.color = Rgb::parse_hex(&string(node)?)
                        .ok_or_else(|| anyhow::anyhow!("expected #rrggbb color"))?
                }
                "font" if component != Component::Wallpaper => value.font = string(node)?,
                "clock-format" | "date-format" if component != Component::Wallpaper => {
                    let format = string(node)?;
                    ensure!(
                        !chrono::format::StrftimeItems::new(&format)
                            .any(|i| matches!(i, chrono::format::Item::Error)),
                        "invalid clock/date format"
                    );
                    if name == "clock-format" {
                        value.clock_format = format;
                    } else {
                        value.date_format = format;
                    }
                }
                "dim" if component != Component::Wallpaper => {
                    ensure!(node.entries().len() == 1, "dim requires one integer");
                    let entry = &node.entries()[0];
                    ensure!(
                        entry.name().is_none() && entry.ty().is_none(),
                        "invalid dim entry"
                    );
                    let n = entry
                        .value()
                        .as_integer()
                        .filter(|n| (0..=100).contains(n))
                        .ok_or_else(|| anyhow::anyhow!("dim must be 0..100"))?;
                    value.dim = n as u8;
                }
                "widgets" if component != Component::Wallpaper => {
                    value.widgets = widget_names(node)?;
                }
                _ => bail!("unknown session setting"),
            }
        }
        // Output blocks are overrides, independent of their position in the file.
        for node in nodes
            .nodes()
            .iter()
            .filter(|n| n.name().value() == "output")
        {
            let output = string(node)?;
            ensure!(
                !output.is_empty() && !value.outputs.contains_key(&output),
                "duplicate or empty output name"
            );
            let children = node
                .children()
                .ok_or_else(|| anyhow::anyhow!("output requires a block"))?;
            let mut background = value.background.clone();
            let mut fields = HashSet::new();
            for field in children.nodes() {
                ensure!(
                    fields.insert(field.name().value()),
                    "duplicate output setting"
                );
                if component == Component::Lockscreen && field.name().value() == "hide-widgets" {
                    let widgets = widget_names(field)?;
                    ensure!(
                        widgets.iter().all(|name| {
                            matches!(name.as_str(), "clock" | "date")
                                || PASSIVE_WIDGETS.contains(&name.as_str())
                                || value.custom.contains_key(name)
                        }),
                        "only passive widgets can be hidden on an output"
                    );
                    value.hidden_widgets.insert(output.clone(), widgets);
                } else {
                    parse_background(&mut background, field, source)?;
                }
            }
            value.outputs.insert(output, background);
        }
        if component != Component::Wallpaper {
            for widget in &value.widgets {
                ensure!(
                    PASSIVE_WIDGETS.contains(&widget.as_str()) || value.custom.contains_key(widget),
                    "unknown enabled widget"
                );
                if !explicit_layout && value.custom.contains_key(widget) {
                    value
                        .layout
                        .regions
                        .last_mut()
                        .unwrap()
                        .widgets
                        .push(widget.clone());
                }
            }
            value.layout.validate(&value.widgets, &value.custom)?;
        }
        Ok(value)
    }
    pub fn for_output(&self, name: &str) -> &Background {
        self.outputs.get(name).unwrap_or(&self.background)
    }

    pub fn hidden_on(&self, output: &str) -> &[String] {
        self.hidden_widgets.get(output).map_or(&[], Vec::as_slice)
    }
}

fn widget_names(node: &kdl::KdlNode) -> Result<Vec<String>> {
    ensure!(
        node.ty().is_none() && node.children().is_none(),
        "expected a list of widgets"
    );
    let mut names = Vec::new();
    for entry in node.entries() {
        ensure!(
            entry.name().is_none() && entry.ty().is_none(),
            "invalid widget entry"
        );
        let name = entry
            .value()
            .as_string()
            .ok_or_else(|| anyhow::anyhow!("expected widget name"))?;
        ensure!(
            !names.iter().any(|existing| existing == name),
            "duplicate widget"
        );
        names.push(name.to_owned());
    }
    Ok(names)
}
fn string(node: &kdl::KdlNode) -> Result<String> {
    ensure!(
        node.ty().is_none() && node.entries().len() == 1,
        "setting requires one string"
    );
    let entry = &node.entries()[0];
    ensure!(
        entry.name().is_none() && entry.ty().is_none(),
        "setting requires an unannotated string"
    );
    Ok(entry
        .value()
        .as_string()
        .ok_or_else(|| anyhow::anyhow!("setting requires a string"))?
        .to_owned())
}
fn parse_background(
    value: &mut Background,
    node: &kdl::KdlNode,
    base: Option<&std::path::Path>,
) -> Result<()> {
    ensure!(node.children().is_none(), "unexpected background block");
    let text = string(node)?;
    match node.name().value() {
        "image" => {
            let path = PathBuf::from(text);
            ensure!(!path.as_os_str().is_empty(), "image path is empty");
            value.image = Some(if path.is_absolute() {
                path
            } else {
                base.and_then(|p| p.parent())
                    .ok_or_else(|| anyhow::anyhow!("relative image requires a configuration file"))?
                    .join(path)
            });
        }
        "fit" => {
            value.fit = match text.as_str() {
                "fill" => Fit::Fill,
                "fit" => Fit::Fit,
                "stretch" => Fit::Stretch,
                "center" => Fit::Center,
                _ => bail!("unknown image fit"),
            }
        }
        _ => bail!("unknown output background setting"),
    }
    Ok(())
}

fn path(text: &str, base: Option<&std::path::Path>) -> Result<PathBuf> {
    let path = PathBuf::from(text);
    ensure!(!text.is_empty(), "empty asset path");
    Ok(if path.is_absolute() {
        path
    } else {
        base.and_then(|p| p.parent())
            .ok_or_else(|| anyhow::anyhow!("relative asset requires a configuration file"))?
            .join(path)
    })
}

fn parse_widget(
    node: &kdl::KdlNode,
    base: Option<&std::path::Path>,
) -> Result<canopa_widgets::script::Script> {
    let children = node
        .children()
        .ok_or_else(|| anyhow::anyhow!("widget requires a block"))?;
    let mut widget = canopa_widgets::script::Script {
        directory: base
            .and_then(|p| p.parent())
            .map(std::path::Path::to_path_buf),
        ..Default::default()
    };
    let mut seen = HashSet::new();
    for field in children.nodes() {
        let name = field.name().value();
        ensure!(
            field.ty().is_none() && seen.insert(name),
            "duplicate or annotated widget field"
        );
        if name == "command" {
            if let Some(children) = field.children() {
                ensure!(
                    field.entries().is_empty() && children.nodes().len() == 1,
                    "command requires argv"
                );
                let argv = &children.nodes()[0];
                ensure!(
                    argv.name().value() == "argv"
                        && argv.ty().is_none()
                        && argv.children().is_none(),
                    "command requires argv"
                );
                for entry in argv.entries() {
                    ensure!(
                        entry.name().is_none() && entry.ty().is_none(),
                        "invalid argv entry"
                    );
                    widget.argv.push(
                        entry
                            .value()
                            .as_string()
                            .ok_or_else(|| anyhow::anyhow!("argv requires strings"))?
                            .to_owned(),
                    );
                }
            } else {
                widget.argv = shlex::split(&string(field)?)
                    .ok_or_else(|| anyhow::anyhow!("unmatched command quote"))?;
            }
            continue;
        }
        ensure!(field.children().is_none(), "unexpected widget block");
        match name {
            "type" => ensure!(
                string(field)? == "script",
                "custom widget type must be script"
            ),
            "format" => widget.format = string(field)?,
            "color" => {
                let color = string(field)?;
                ensure!(
                    ["text", "muted", "accent", "warning", "danger", "success"]
                        .contains(&color.as_str())
                        || Rgb::parse_hex(&color).is_some(),
                    "invalid widget color"
                );
                widget.color = color;
            }
            "preview" => widget.preview = string(field)?,
            "interval" | "timeout" | "size" => {
                ensure!(
                    field.entries().len() == 1
                        && field.entries()[0].name().is_none()
                        && field.entries()[0].ty().is_none(),
                    "widget dimension requires an integer"
                );
                let number = field.entries()[0]
                    .value()
                    .as_integer()
                    .ok_or_else(|| anyhow::anyhow!("widget dimension requires an integer"))?;
                let (min, max) = match name {
                    "interval" => (1, 86400),
                    "timeout" => (1, 60),
                    _ => (8, 160),
                };
                ensure!(
                    (min..=max).contains(&number),
                    "widget dimension out of range"
                );
                match name {
                    "interval" => widget.interval = number as u64,
                    "timeout" => widget.timeout = number as u64,
                    _ => widget.size = number as u32,
                }
            }
            _ => bail!("unknown widget field"),
        }
    }
    ensure!(
        !widget.argv.is_empty()
            && !widget.argv[0].is_empty()
            && widget.argv.len() <= 64
            && widget.argv.iter().all(|arg| !arg.contains('\0'))
            && widget.format.len() <= 4096
            && widget.preview.len() <= 4096,
        "invalid or missing widget command/format"
    );
    Ok(widget)
}
