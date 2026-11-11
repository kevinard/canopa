use crate::storage::{Snapshot, companion};
use anyhow::{Context, Result, ensure};
use canopa_theme::{Appearance, ColorScheme, Rgb};

#[derive(Clone, Debug)]
pub struct Personal {
    pub appearance: Appearance,
    pub lock: canopa_session_ui::settings::Settings,
    pub text_scale: u16,
    pub reduced_motion: bool,
}
impl Personal {
    pub fn from_lock(lock: canopa_session_ui::settings::Settings) -> Self {
        Self {
            appearance: lock.appearance.clone(),
            text_scale: lock.appearance.text_scale(),
            reduced_motion: lock.appearance.reduced_motion(),
            lock,
        }
    }

    pub fn load(snapshot: &Snapshot) -> Result<Self> {
        let request = if snapshot.text.is_empty() {
            canopa_config::Request::Defaults
        } else {
            canopa_config::Request::common(&snapshot.path)
        };
        let (lock, _) = canopa_session_ui::settings::Settings::load(
            &request,
            canopa_config::Component::Lockscreen,
        )?;
        Ok(Self::from_lock(lock))
    }
    pub fn text(&self, snapshot: &Snapshot) -> Result<String> {
        let mut document: kdl::KdlDocument = if snapshot.text.is_empty() {
            "schema-version 1\n".parse()?
        } else {
            snapshot.text.parse()?
        };
        let appearance = block(&mut document, "appearance")?;
        put(
            appearance,
            "color-scheme",
            Some(
                if self.appearance.color_scheme == ColorScheme::Light {
                    "light"
                } else {
                    "dark"
                }
                .into(),
            ),
        );
        put(
            appearance,
            "accent",
            self.appearance
                .accent
                .map(|Rgb(r, g, b)| format!("#{r:02x}{g:02x}{b:02x}").into()),
        );
        if self.text_scale != 100 || appearance.get("text-scale").is_some() {
            put(
                appearance,
                "text-scale",
                Some(i128::from(self.text_scale).into()),
            );
        }
        if self.reduced_motion || appearance.get("reduced-motion").is_some() {
            put(
                appearance,
                "reduced-motion",
                Some(self.reduced_motion.into()),
            );
        }
        let lock = block(&mut document, "lockscreen")?;
        put(
            lock,
            "clock-format",
            Some(self.lock.clock_format.clone().into()),
        );
        put(
            lock,
            "date-format",
            Some(self.lock.date_format.clone().into()),
        );
        put(lock, "dim", Some(i128::from(self.lock.dim).into()));
        put(lock, "font", Some(self.lock.font.clone().into()));
        put(
            lock,
            "avatar",
            self.lock
                .avatar
                .as_ref()
                .map(|v| v.to_string_lossy().into_owned().into()),
        );
        Ok(document.to_string())
    }
    pub fn save(&self, snapshot: &Snapshot) -> Result<Snapshot> {
        save_common(snapshot, &self.text(snapshot)?)
    }
}

/// Lockscreen fields that Personalization writes. Widgets writes the others.
#[cfg(feature = "desktop")]
const PERSONAL_LOCK_FIELDS: [&str; 5] = ["clock-format", "date-format", "dim", "font", "avatar"];

/// The parts of the shared configuration written by Personalization and by
/// Widgets, without formatting. Both pages edit `config.kdl`; each rebases its
/// pending edits on the other's save only if the save left its part unchanged.
#[cfg(feature = "desktop")]
pub(crate) fn common_sections(text: &str) -> Result<(String, String)> {
    let document: kdl::KdlDocument = if text.trim().is_empty() {
        kdl::KdlDocument::new()
    } else {
        text.parse()?
    };
    let plain = |node: &kdl::KdlNode| {
        let mut node = node.clone();
        node.clear_format_recursive();
        node.to_string().trim().to_owned()
    };
    let mut personal = document.get("appearance").map(plain).unwrap_or_default();
    let mut widgets = document.get("shell").map(plain).unwrap_or_default();
    for node in document
        .get("lockscreen")
        .and_then(kdl::KdlNode::children)
        .map(kdl::KdlDocument::nodes)
        .unwrap_or_default()
    {
        let part = if PERSONAL_LOCK_FIELDS.contains(&node.name().value()) {
            &mut personal
        } else {
            &mut widgets
        };
        part.push('\n');
        part.push_str(&plain(node));
    }
    Ok((personal, widgets))
}

/// Whether Personalization, still based on `current`, can adopt `saved`: the
/// Widgets page saved it from that same version and left Personalization's
/// part unchanged. Any other difference is an external edit.
#[cfg(feature = "desktop")]
pub(crate) fn personal_rebase(current: &Snapshot, previous: &Snapshot, saved: &Snapshot) -> bool {
    current.same(previous)
        && matches!(
            (common_sections(&previous.text), common_sections(&saved.text)),
            (Ok((before, _)), Ok((after, _))) if before == after
        )
}

pub(crate) fn save_common(snapshot: &Snapshot, text: &str) -> Result<Snapshot> {
    if snapshot.text.is_empty() {
        for component in [
            canopa_config::Component::Shell,
            canopa_config::Component::Notifications,
        ] {
            ensure!(
                !matches!(
                    canopa_config::Request::Discover.resolve(component)?,
                    canopa_config::Request::File {
                        format: canopa_config::Format::Legacy,
                        ..
                    }
                ),
                "migrate existing legacy application configuration before creating shared settings; see the configuration guide"
            );
        }
    }
    snapshot.save(text, |path| {
        crate::storage::command(
            companion("canopa"),
            &[
                "config".into(),
                "check".into(),
                "--file".into(),
                path.to_string_lossy().into_owned(),
            ],
        )?;
        Ok(())
    })
}

pub(crate) fn block<'a>(
    document: &'a mut kdl::KdlDocument,
    name: &str,
) -> Result<&'a mut kdl::KdlDocument> {
    if document.get(name).is_none() {
        document.nodes_mut().push(kdl::KdlNode::new(name));
    }
    let node = document
        .get_mut(name)
        .context("missing configuration block")?;
    if node.children().is_none() {
        node.set_children(kdl::KdlDocument::new());
    }
    Ok(node.children_mut().as_mut().expect("children installed"))
}
fn put(document: &mut kdl::KdlDocument, name: &str, value: Option<kdl::KdlValue>) {
    if let Some(value) = value {
        if let Some(node) = document.get_mut(name) {
            node.entries_mut().clear();
            node.push(value);
        } else {
            let mut node = kdl::KdlNode::new(name);
            node.push(value);
            document.nodes_mut().push(node);
        }
    } else {
        document
            .nodes_mut()
            .retain(|node| node.name().value() != name);
    }
}

/// Only existing numeric timeout tokens are changed. Commands, custom profiles,
/// comments and inhibition policy remain byte-for-byte intact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdleTimes {
    pub ac: [u32; 3],
    pub battery: [u32; 3],
}
impl Default for IdleTimes {
    fn default() -> Self {
        Self {
            ac: [300, 60, 600],
            battery: [120, 30, 300],
        }
    }
}
impl IdleTimes {
    pub fn parse(text: &str) -> Result<Self> {
        let mut result = Self::default();
        let positions = timeouts(text)?;
        for (context, action, _, value) in positions {
            let target = if context == "battery" {
                &mut result.battery
            } else {
                &mut result.ac
            };
            target[action] = value;
        }
        Ok(result)
    }
    pub fn text(&self, original: &str) -> Result<String> {
        ensure!(
            self.ac
                .iter()
                .chain(self.battery.iter())
                .all(|v| (5..=86400).contains(v)),
            "idle stages must be between 5 seconds and one day"
        );
        let positions = timeouts(original)?;
        let mut lines: Vec<_> = original.split_inclusive('\n').map(str::to_owned).collect();
        for (context, action, index, _) in positions {
            let value = if context == "battery" {
                self.battery[action]
            } else {
                self.ac[action]
            };
            let line = &lines[index];
            let start = line.find("timeout").context("missing timeout")? + "timeout".len();
            let whitespace = line[start..].len() - line[start..].trim_start().len();
            let start = start + whitespace;
            let end = line[start..]
                .find(|c: char| !c.is_ascii_digit())
                .map_or(line.len(), |v| start + v);
            lines[index].replace_range(start..end, &value.to_string());
        }
        Ok(lines.concat())
    }
    pub fn save(&self, snapshot: &Snapshot) -> Result<Snapshot> {
        let text = self.text(&snapshot.text)?;
        // Numeric-only edits cannot add Rune syntax or change executable policy.
        let saved = snapshot.save(&text, |_| {
            timeouts(&text)?;
            Ok(())
        })?;
        if let Err(error) = idle_command(&["reload".into()]) {
            saved.save(&snapshot.text, |_| Ok(()))?;
            return Err(error.context("Stasis rejected the reload; the previous file was restored"));
        }
        Ok(saved)
    }
}

pub fn idle_command(arguments: &[String]) -> Result<String> {
    let reply = crate::storage::command(companion("canopa-idle"), arguments)?;
    validate_idle_reply(&reply)?;
    Ok(reply)
}
pub fn validate_idle_reply(reply: &str) -> Result<()> {
    let text = reply.trim();
    ensure!(
        !text.is_empty()
            && !text.to_ascii_uppercase().starts_with("ERROR:")
            && !text.to_ascii_lowercase().contains("not running"),
        "Stasis did not acknowledge the operation: {text}"
    );
    Ok(())
}
fn timeouts(text: &str) -> Result<Vec<(String, usize, usize, u32)>> {
    ensure!(
        text.len() <= crate::storage::MAX_BYTES,
        "idle configuration exceeds budget"
    );
    let mut stack: Vec<String> = Vec::new();
    let mut found = Vec::new();
    let mut keys = std::collections::HashSet::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.starts_with('#') || line.starts_with('@') || line.is_empty() {
            continue;
        }
        if line == "end" {
            ensure!(stack.pop().is_some(), "unbalanced Rune configuration");
            continue;
        }
        if let Some(name) = line.strip_suffix(':') {
            ensure!(
                name.bytes()
                    .all(|v| v.is_ascii_alphanumeric() || b"_-".contains(&v)),
                "custom Rune block syntax is read-only"
            );
            stack.push(name.into());
            continue;
        }
        if !line.starts_with("timeout ") && !line.starts_with("timeout\t") {
            continue;
        }
        let Some(action) = stack.last().and_then(|v| {
            ["lock_screen", "dpms", "suspend"]
                .iter()
                .position(|a| *a == v)
        }) else {
            continue;
        };
        let context = match stack.as_slice() {
            [root, _] if root == "default" => "default",
            [root, context, _]
                if root == "default" && ["ac", "battery"].contains(&context.as_str()) =>
            {
                context
            }
            _ => continue,
        };
        let value = line
            .split_whitespace()
            .nth(1)
            .context("missing idle timeout")?
            .parse::<u32>()?;
        ensure!(
            value <= 86400 && keys.insert((context.to_owned(), action)),
            "ambiguous or unsupported idle timeout"
        );
        found.push((context.to_owned(), action, index, value));
    }
    ensure!(
        stack.is_empty()
            && ["default", "ac", "battery"]
                .iter()
                .all(|context| (0..3).all(|action| keys.contains(&(context.to_string(), action)))),
        "custom idle plans are read-only; the standard lock/display/suspend stages are required"
    );
    Ok(found)
}
