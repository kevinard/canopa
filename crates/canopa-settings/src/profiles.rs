//! Saved display layouts and unambiguous matching against connected outputs.
use crate::{
    display::{self, Monitor},
    session::Paths,
    storage::{Snapshot, quote},
};
use anyhow::{Context, Result, bail, ensure};
use kdl::{KdlDocument, KdlNode, KdlValue};
use niri_ipc::{Output, Transform};
use std::{collections::HashMap, fmt, time::Duration};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Switching {
    Manual,
    #[default]
    Suggest,
    Automatic,
}
impl Switching {
    pub const ALL: [Self; 3] = [Self::Manual, Self::Suggest, Self::Automatic];

    fn key(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Suggest => "suggest",
            Self::Automatic => "automatic",
        }
    }
}
impl fmt::Display for Switching {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Manual => "Manual only",
            Self::Suggest => "Suggest matching profiles",
            Self::Automatic => "Switch automatically",
        })
    }
}

#[derive(Clone, Debug, Default)]
struct Selector {
    connector: Option<String>,
    make: Option<String>,
    model: Option<String>,
    serial: Option<String>,
}
impl Selector {
    fn matches(&self, output: &Output) -> bool {
        fn field(pattern: Option<&str>, value: Option<&str>) -> bool {
            pattern.is_none_or(|pattern| pattern == "*" || value == Some(pattern))
        }
        field(self.connector.as_deref(), Some(&output.name))
            && field(self.make.as_deref(), Some(&output.make))
            && field(self.model.as_deref(), Some(&output.model))
            && field(self.serial.as_deref(), output.serial.as_deref())
    }

    fn for_output(output: &Output, outputs: &HashMap<String, Output>) -> Self {
        let serial = output
            .serial
            .as_ref()
            .filter(|s| !s.trim().is_empty() && !s.eq_ignore_ascii_case("unknown"))
            .cloned();
        let mut selector = Self {
            make: Some(output.make.clone()),
            model: Some(output.model.clone()),
            serial,
            ..Self::default()
        };
        if selector.serial.is_none()
            || outputs.values().filter(|o| selector.matches(o)).count() != 1
        {
            selector.connector = Some(output.name.clone());
        }
        selector
    }
}

#[derive(Clone, Debug)]
struct Layout {
    selector: Selector,
    enabled: bool,
    mode: Option<(u16, u16, u32)>,
    custom: bool,
    scale: f64,
    transform: Transform,
    x: i32,
    y: i32,
}
impl Layout {
    fn parse(node: &KdlNode) -> Result<Self> {
        shape(
            node,
            0,
            &[
                "connector",
                "make",
                "model",
                "serial",
                "enabled",
                "width",
                "height",
                "refresh-mhz",
                "custom-mode",
                "scale",
                "transform",
                "x",
                "y",
            ],
        )?;
        ensure!(node.children().is_none(), "output rules use properties");
        let selector = Selector {
            connector: string_property(node, "connector")?,
            make: string_property(node, "make")?,
            model: string_property(node, "model")?,
            serial: string_property(node, "serial")?,
        };
        ensure!(
            selector.connector.is_some()
                || selector.make.is_some()
                || selector.model.is_some()
                || selector.serial.is_some(),
            "an output needs a selector such as connector=\"eDP-1\""
        );
        let enabled = boolean(node, "enabled", true)?;
        let width = integer::<u16>(node, "width")?;
        let height = integer::<u16>(node, "height")?;
        let rate = integer::<u32>(node, "refresh-mhz")?;
        let mode = match (width, height, rate) {
            (None, None, None) => None,
            (Some(w), Some(h), Some(r)) if w > 0 && h > 0 && r > 0 => Some((w, h, r)),
            _ => bail!("a mode requires positive width, height and refresh-mhz"),
        };
        let custom = boolean(node, "custom-mode", false)?;
        ensure!(
            !custom || mode.is_some(),
            "custom-mode requires an explicit mode"
        );
        let scale = match node.get("scale") {
            None => 1.,
            Some(KdlValue::Float(value)) => *value,
            Some(KdlValue::Integer(value)) => *value as f64,
            _ => bail!("scale must be a number"),
        };
        let transform = match string_property(node, "transform")?
            .as_deref()
            .unwrap_or("normal")
        {
            "normal" => Transform::Normal,
            "90" => Transform::_90,
            "180" => Transform::_180,
            "270" => Transform::_270,
            "flipped" => Transform::Flipped,
            "flipped-90" => Transform::Flipped90,
            "flipped-180" => Transform::Flipped180,
            "flipped-270" => Transform::Flipped270,
            _ => bail!("unknown display transform"),
        };
        let x = integer::<i32>(node, "x")?.unwrap_or(0);
        let y = integer::<i32>(node, "y")?.unwrap_or(0);
        ensure!(
            (0.5..=4.).contains(&scale)
                && (-32768..=32768).contains(&x)
                && (-32768..=32768).contains(&y),
            "display scale or position is outside its supported range"
        );
        Ok(Self {
            selector,
            enabled,
            mode,
            custom,
            scale,
            transform,
            x,
            y,
        })
    }

    fn resolve(&self, output: &Output) -> Result<Monitor> {
        let mut monitor = Monitor::from_output(output);
        monitor.enabled = self.enabled;
        monitor.custom_mode = self.custom;
        monitor.scale = self.scale;
        monitor.transform = self.transform;
        monitor.x = self.x;
        monitor.y = self.y;
        if self.enabled {
            monitor.mode = if let Some((width, height, refresh)) = self.mode {
                output
                    .modes
                    .iter()
                    .position(|m| (m.width, m.height, m.refresh_rate) == (width, height, refresh))
            } else {
                output
                    .modes
                    .iter()
                    .position(|m| m.is_preferred)
                    .or((!output.modes.is_empty()).then_some(0))
            };
            ensure!(
                monitor.mode.is_some(),
                "the saved mode for {} is unavailable",
                output.name
            );
        }
        Ok(monitor)
    }
}

#[derive(Clone, Debug)]
pub struct Profile {
    pub name: String,
    pub priority: i32,
    pub hotplug: bool,
    layouts: Vec<Layout>,
}
impl Profile {
    fn parse(node: &KdlNode) -> Result<Self> {
        shape(node, 1, &["priority", "hotplug"])?;
        let name = name(
            node.get(0)
                .and_then(KdlValue::as_string)
                .context("profile needs a name")?,
        )?;
        let rules = node
            .children()
            .context("profile needs output rules")?
            .nodes();
        ensure!(
            (1..=32).contains(&rules.len()),
            "profile needs 1 to 32 output rules"
        );
        let layouts = rules
            .iter()
            .map(|node| {
                ensure!(
                    node.name().value() == "output",
                    "unknown profile entry {}",
                    node.name()
                );
                Layout::parse(node)
            })
            .collect::<Result<Vec<_>>>()?;
        ensure!(
            layouts.iter().any(|rule| rule.enabled),
            "a profile must enable a display"
        );
        Ok(Self {
            name: name.to_owned(),
            priority: integer::<i32>(node, "priority")?.unwrap_or(0),
            hotplug: boolean(node, "hotplug", true)?,
            layouts,
        })
    }

    pub fn resolve(&self, outputs: &HashMap<String, Output>) -> Result<Vec<Monitor>> {
        ensure!(
            self.layouts.len() == outputs.len(),
            "connected displays differ from this profile"
        );
        let mut connected: Vec<_> = outputs.values().collect();
        connected.sort_unstable_by(|a, b| a.name.cmp(&b.name));
        let edges: Vec<Vec<usize>> = self
            .layouts
            .iter()
            .map(|rule| {
                connected
                    .iter()
                    .enumerate()
                    .filter_map(|(i, output)| rule.selector.matches(output).then_some(i))
                    .collect()
            })
            .collect();
        let matched =
            assignment(&edges, None).context("connected displays differ from this profile")?;
        for (rule, &output) in matched.iter().enumerate() {
            ensure!(
                assignment(&edges, Some((rule, output))).is_none(),
                "display selectors are ambiguous; specify a connector or serial"
            );
        }
        self.layouts
            .iter()
            .zip(matched)
            .map(|(rule, index)| rule.resolve(connected[index]))
            .collect()
    }
}

// Find a full assignment. Removing each selected edge in turn detects any
// alternative full assignment without enumerating permutations of identical displays.
fn assignment(edges: &[Vec<usize>], excluded: Option<(usize, usize)>) -> Option<Vec<usize>> {
    fn assign(
        rule: usize,
        edges: &[Vec<usize>],
        owners: &mut [Option<usize>],
        seen: &mut [bool],
        excluded: Option<(usize, usize)>,
    ) -> bool {
        for &output in &edges[rule] {
            if seen[output] || excluded == Some((rule, output)) {
                continue;
            }
            seen[output] = true;
            if owners[output].is_none_or(|old| assign(old, edges, owners, seen, excluded)) {
                owners[output] = Some(rule);
                return true;
            }
        }
        false
    }

    let mut owners = vec![None; edges.len()];
    for rule in 0..edges.len() {
        if !assign(
            rule,
            edges,
            &mut owners,
            &mut vec![false; edges.len()],
            excluded,
        ) {
            return None;
        }
    }
    let mut matched = vec![0; edges.len()];
    for (output, owner) in owners.into_iter().enumerate() {
        matched[owner?] = output;
    }
    Some(matched)
}

#[derive(Clone, Debug)]
pub struct Book {
    pub source: Snapshot,
    pub switching: Switching,
    pub settle: Duration,
    pub offer: Duration,
    pub startup: bool,
    pub profiles: Vec<Profile>,
}
impl Book {
    pub fn load(paths: &Paths) -> Result<Self> {
        let source = Snapshot::load(paths.profiles())?;
        Self::from_source(source).context("invalid display-profiles.kdl")
    }

    fn from_source(source: Snapshot) -> Result<Self> {
        let document: KdlDocument = source.text.parse()?;
        let mut book = Self {
            source,
            switching: Switching::default(),
            settle: Duration::from_millis(600),
            offer: Duration::from_secs(15),
            startup: true,
            profiles: vec![],
        };
        let mut settings = std::collections::HashSet::new();
        for node in document.nodes() {
            let key = node.name().value();
            if key == "profile" {
                let profile = Profile::parse(node)?;
                ensure!(book.profiles.len() < 32, "too many display profiles");
                ensure!(
                    !book.profiles.iter().any(|p| p.name == profile.name),
                    "duplicate display profile {}",
                    profile.name
                );
                book.profiles.push(profile);
                continue;
            }
            ensure!(
                settings.insert(key),
                "duplicate display profile setting {key}"
            );
            shape(node, 1, &[])?;
            ensure!(node.children().is_none(), "invalid {key} setting");
            match key {
                "switching" => {
                    let value = node.get(0).and_then(KdlValue::as_string);
                    book.switching = Switching::ALL
                        .into_iter()
                        .find(|policy| Some(policy.key()) == value)
                        .context("switching must be manual, suggest or automatic")?;
                }
                "settle-ms" => {
                    let ms = node
                        .get(0)
                        .and_then(KdlValue::as_integer)
                        .filter(|ms| (100..=5000).contains(ms))
                        .context("settle-ms must be between 100 and 5000")?;
                    book.settle = Duration::from_millis(ms as u64);
                }
                "offer-ms" => {
                    let ms = node
                        .get(0)
                        .and_then(KdlValue::as_integer)
                        .filter(|ms| (1000..=120000).contains(ms))
                        .context("offer-ms must be between 1000 and 120000")?;
                    book.offer = Duration::from_millis(ms as u64);
                }
                "startup" => {
                    book.startup = node
                        .get(0)
                        .and_then(KdlValue::as_bool)
                        .context("startup must be a boolean")?
                }
                _ => bail!("unknown display profile setting {key}"),
            }
        }
        Ok(book)
    }

    pub fn get(&self, name: &str) -> Result<&Profile> {
        self.profiles
            .iter()
            .find(|p| p.name == name)
            .context("display profile no longer exists")
    }

    pub fn matching(
        &self,
        outputs: &HashMap<String, Output>,
    ) -> Result<Option<(&Profile, Vec<Monitor>)>> {
        let mut best: Option<(&Profile, Vec<Monitor>)> = None;
        let mut tied = false;
        for profile in self.profiles.iter().filter(|p| p.hotplug) {
            let Ok(layout) = profile.resolve(outputs) else {
                continue;
            };
            match best
                .as_ref()
                .map(|(p, _)| p.priority.cmp(&profile.priority))
            {
                None | Some(std::cmp::Ordering::Less) => {
                    best = Some((profile, layout));
                    tied = false;
                }
                Some(std::cmp::Ordering::Equal) => tied = true,
                Some(std::cmp::Ordering::Greater) => {}
            }
        }
        ensure!(!tied, "multiple display profiles match with equal priority");
        Ok(best)
    }

    fn edit(&self, edit: impl FnOnce(&mut KdlDocument) -> Result<()>) -> Result<Self> {
        let mut document: KdlDocument = self.source.text.parse()?;
        edit(&mut document)?;
        let source = self.source.save(&document.to_string(), |candidate| {
            Self::from_source(Snapshot::load(candidate)?)?;
            Ok(())
        })?;
        Self::from_source(source)
    }

    pub fn set_switching(&self, switching: Switching) -> Result<Self> {
        self.edit(|document| {
            if let Some(node) = document.get_mut("switching") {
                node.insert(0, switching.key());
            } else {
                let node: KdlNode = format!("switching {}\n", quote(switching.key())).parse()?;
                document.nodes_mut().insert(0, node);
            }
            Ok(())
        })
    }

    pub fn remove(&self, name: &str) -> Result<Self> {
        self.get(name)?;
        self.edit(|document| {
            document.nodes_mut().retain(|node| {
                node.name().value() != "profile"
                    || node.get(0).and_then(KdlValue::as_string) != Some(name)
            });
            Ok(())
        })
    }

    pub fn save_layout(
        &self,
        title: &str,
        monitors: &[Monitor],
        outputs: &HashMap<String, Output>,
    ) -> Result<Self> {
        use std::fmt::Write;
        let title = name(title)?;
        ensure!(
            !self.profiles.iter().any(|p| p.name == title),
            "a profile with that name already exists"
        );
        display::validate_profile(monitors, outputs)?;
        let mut text = format!("profile {} {{\n", quote(title));
        for monitor in monitors {
            let selector = Selector::for_output(&outputs[&monitor.name], outputs);
            text.push_str("    output");
            for (key, value) in [
                ("connector", selector.connector),
                ("make", selector.make),
                ("model", selector.model),
                ("serial", selector.serial),
            ] {
                if let Some(value) = value {
                    write!(text, " {key}={}", quote(&value))?;
                }
            }
            if monitor.enabled {
                let mode = monitor
                    .mode
                    .and_then(|i| monitor.modes.get(i))
                    .context("missing mode")?;
                write!(
                    text,
                    " width={} height={} refresh-mhz={} scale={} transform={} x={} y={}",
                    mode.width,
                    mode.height,
                    mode.refresh_rate,
                    monitor.scale,
                    quote(display::transform_name(monitor.transform)),
                    monitor.x,
                    monitor.y
                )?;
                if monitor.custom_mode {
                    text.push_str(" custom-mode=#true");
                }
            } else {
                text.push_str(" enabled=#false");
            }
            text.push('\n');
        }
        text.push_str("}\n");
        self.edit(|document| {
            document.nodes_mut().push(text.parse()?);
            Ok(())
        })
    }
}

fn name(value: &str) -> Result<&str> {
    ensure!(
        !value.is_empty()
            && value.trim() == value
            && value.len() <= 80
            && !value.chars().any(char::is_control),
        "profile names must be 1 to 80 bytes of visible text"
    );
    Ok(value)
}

fn shape(node: &KdlNode, positional: usize, keys: &[&str]) -> Result<()> {
    let mut props = std::collections::HashSet::new();
    let mut args = 0;
    for entry in node.entries() {
        if let Some(key) = entry.name() {
            ensure!(
                keys.contains(&key.value()) && props.insert(key.value()),
                "unknown or repeated {} property {}",
                node.name(),
                key.value()
            );
        } else {
            args += 1;
        }
    }
    ensure!(
        args == positional,
        "wrong argument count for {}",
        node.name()
    );
    Ok(())
}

fn string_property(node: &KdlNode, key: &str) -> Result<Option<String>> {
    node.get(key)
        .map(|value| {
            let value = value
                .as_string()
                .with_context(|| format!("{key} must be a string"))?;
            ensure!(
                value.len() <= 256 && !value.chars().any(char::is_control),
                "invalid {key} selector"
            );
            Ok(value.to_owned())
        })
        .transpose()
}

fn boolean(node: &KdlNode, key: &str, default: bool) -> Result<bool> {
    node.get(key).map_or(Ok(default), |value| {
        value
            .as_bool()
            .with_context(|| format!("{key} must be a boolean"))
    })
}

fn integer<T: TryFrom<i128>>(node: &KdlNode, key: &str) -> Result<Option<T>> {
    node.get(key)
        .map(|value| {
            value
                .as_integer()
                .and_then(|n| T::try_from(n).ok())
                .with_context(|| format!("invalid integer for {key}"))
        })
        .transpose()
}
