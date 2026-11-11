//! The 12/24-hour clock preference for Canopa (ADR 0142).
//!
//! The preference has no key of its own: it is the hour directive of the
//! existing format strings, each Shell clock module's `time-format` and the
//! lock screen's `clock-format` in `config.kdl`. Settings recognizes the
//! usual 24- and 12-hour patterns and switches between them, with or without
//! seconds; any other pattern is the person's own and is never rewritten.
use anyhow::{Context, Result};
use kdl::{KdlDocument, KdlNode};

/// The Shell's shipped configuration, used when `config.kdl` has no `shell`.
const SHELL_DEFAULTS: &str = include_str!("../../canopa-shell/config/default.kdl");
/// The format clocks use when none is configured.
const DEFAULT_FORMAT: &str = "%H:%M";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HourCycle {
    H24,
    H12,
}

const H24: &[&str] = &["%H:%M", "%R", "%k:%M"];
const H24_SECONDS: &[&str] = &["%H:%M:%S", "%T", "%k:%M:%S"];
const H12: &[&str] = &["%-I:%M %p", "%I:%M %p", "%l:%M %p", "%-I:%M%p", "%I:%M%p"];
const H12_SECONDS: &[&str] = &["%-I:%M:%S %p", "%I:%M:%S %p", "%r", "%l:%M:%S %p"];

/// The cycle of a recognized pattern, and whether it shows seconds.
pub fn classify(format: &str) -> Option<(HourCycle, bool)> {
    [
        (H24, HourCycle::H24, false),
        (H24_SECONDS, HourCycle::H24, true),
        (H12, HourCycle::H12, false),
        (H12_SECONDS, HourCycle::H12, true),
    ]
    .into_iter()
    .find(|(patterns, ..)| patterns.contains(&format))
    .map(|(_, cycle, seconds)| (cycle, seconds))
}

/// The pattern Settings writes.
pub fn pattern(cycle: HourCycle, seconds: bool) -> &'static str {
    match (cycle, seconds) {
        (HourCycle::H24, false) => "%H:%M",
        (HourCycle::H24, true) => "%H:%M:%S",
        (HourCycle::H12, false) => "%-I:%M %p",
        (HourCycle::H12, true) => "%-I:%M:%S %p",
    }
}

/// Where a clock's format comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClockFormat {
    /// "Shell clock “clock”" or "Lock screen", for the page.
    pub place: String,
    pub format: String,
}

/// The clocks of `config.kdl`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClockFormats {
    pub clocks: Vec<ClockFormat>,
}

impl ClockFormats {
    /// The common cycle when every clock uses a recognized pattern of it.
    pub fn cycle(&self) -> Option<HourCycle> {
        let mut cycles = self
            .clocks
            .iter()
            .map(|clock| classify(&clock.format).map(|(cycle, _)| cycle));
        let first = cycles.next().flatten()?;
        cycles.all(|cycle| cycle == Some(first)).then_some(first)
    }

    /// Clocks with a pattern of the person's own.
    pub fn custom(&self) -> impl Iterator<Item = &ClockFormat> {
        self.clocks
            .iter()
            .filter(|clock| classify(&clock.format).is_none())
    }
}

fn document(text: &str) -> Result<KdlDocument> {
    Ok(if text.trim().is_empty() {
        "schema-version 1\n".parse()?
    } else {
        text.parse()?
    })
}

fn string(node: &KdlNode) -> Option<&str> {
    node.entries()
        .iter()
        .find(|entry| entry.name().is_none())
        .and_then(|entry| entry.value().as_string())
}

fn child_string<'a>(node: &'a KdlNode, name: &str) -> Option<&'a str> {
    node.children()?.get(name).and_then(string)
}

/// Whether a `module` declaration is a clock: the built-in `clock`, or one
/// with `type "clock"`.
fn is_clock(node: &KdlNode) -> bool {
    node.name().value() == "module"
        && (string(node) == Some("clock") || child_string(node, "type") == Some("clock"))
}

/// The `shell` block's children, or the shipped defaults when there is none.
fn shell(document: &KdlDocument) -> Result<KdlDocument> {
    match document.get("shell").and_then(KdlNode::children) {
        Some(children) => Ok(children.clone()),
        None => Ok(SHELL_DEFAULTS.parse()?),
    }
}

impl ClockFormats {
    pub fn read(text: &str) -> Result<Self> {
        let document = document(text)?;
        let shell = shell(&document)?;
        let mut clocks: Vec<ClockFormat> = shell
            .get("modules")
            .and_then(KdlNode::children)
            .map(KdlDocument::nodes)
            .unwrap_or_default()
            .iter()
            .filter(|node| is_clock(node))
            .map(|node| ClockFormat {
                place: format!("Shell clock “{}”", string(node).unwrap_or("clock")),
                format: child_string(node, "time-format")
                    .unwrap_or(DEFAULT_FORMAT)
                    .to_owned(),
            })
            .collect();
        if clocks.is_empty() {
            clocks.push(ClockFormat {
                place: "Shell clock “clock”".into(),
                format: DEFAULT_FORMAT.into(),
            });
        }
        clocks.push(ClockFormat {
            place: "Lock screen".into(),
            format: document
                .get("lockscreen")
                .and_then(|lock| child_string(lock, "clock-format"))
                .unwrap_or(DEFAULT_FORMAT)
                .to_owned(),
        });
        Ok(Self { clocks })
    }
}

/// Set `name` in `parent` to `value`, keeping the node's other entries.
fn set_string(parent: &mut KdlDocument, name: &str, value: &str) {
    match parent.get_mut(name) {
        Some(node) => {
            node.entries_mut().retain(|entry| entry.name().is_some());
            node.entries_mut().insert(0, kdl::KdlEntry::new(value));
        }
        None => {
            let mut node = KdlNode::new(name);
            node.push(value);
            parent.nodes_mut().push(node);
        }
    }
}

/// The new pattern for a clock showing `current`, or `None` to keep it.
fn switched(current: Option<&str>, cycle: HourCycle) -> Option<&'static str> {
    let (from, seconds) = classify(current.unwrap_or(DEFAULT_FORMAT))?;
    // An unset 24-hour default stays unset.
    if current.is_none() && from == cycle {
        return None;
    }
    let next = pattern(cycle, seconds);
    (current != Some(next)).then_some(next)
}

/// `text` with every recognized clock pattern switched to `cycle`. Custom
/// patterns, comments and every other setting are kept.
pub fn with_hour_cycle(text: &str, cycle: HourCycle) -> Result<String> {
    let mut document = document(text)?;
    let had_shell = document.get("shell").is_some();
    let mut shell = shell(&document)?;
    let mut shell_changed = false;
    if shell.get("modules").is_none() {
        shell.nodes_mut().push(KdlNode::new("modules"));
    }
    let modules = shell.get_mut("modules").context("missing modules block")?;
    if modules.children().is_none() {
        modules.set_children(KdlDocument::new());
    }
    let modules = modules.children_mut().as_mut().expect("children installed");
    let mut any_clock = false;
    for module in modules.nodes_mut().iter_mut().filter(|node| is_clock(node)) {
        any_clock = true;
        if let Some(next) = switched(child_string(module, "time-format"), cycle) {
            if module.children().is_none() {
                module.set_children(KdlDocument::new());
            }
            set_string(
                module.children_mut().as_mut().expect("children installed"),
                "time-format",
                next,
            );
            shell_changed = true;
        }
    }
    if !any_clock && let Some(next) = switched(None, cycle) {
        let mut module = KdlNode::new("module");
        module.push("clock");
        let mut children = KdlDocument::new();
        let mut format = KdlNode::new("time-format");
        format.push(next);
        children.nodes_mut().push(format);
        module.set_children(children);
        modules.nodes_mut().push(module);
        shell_changed = true;
    }
    if shell_changed {
        let block = crate::preferences::block(&mut document, "shell")?;
        *block = shell;
        if !had_shell {
            document.get_mut("shell").expect("shell added").autoformat();
        }
    }
    let lock_format = document
        .get("lockscreen")
        .and_then(|lock| child_string(lock, "clock-format"))
        .map(str::to_owned);
    if let Some(next) = switched(lock_format.as_deref(), cycle) {
        let lock = crate::preferences::block(&mut document, "lockscreen")?;
        set_string(lock, "clock-format", next);
    }
    Ok(document.to_string())
}

/// The lock screen's clock format in `text`, after [`with_hour_cycle`].
pub fn lock_format(text: &str) -> Option<String> {
    ClockFormats::read(text)
        .ok()?
        .clocks
        .pop()
        .map(|clock| clock.format)
}

/// Switch the clocks in `config.kdl`: validate and save it. Returns the
/// replaced and the saved versions, for the pages that share the file.
pub fn set_hour_cycle(
    paths: &crate::session::Paths,
    cycle: HourCycle,
) -> Result<(crate::storage::Snapshot, crate::storage::Snapshot)> {
    let source = crate::storage::Snapshot::load(paths.common())?;
    let text = with_hour_cycle(&source.text, cycle)?;
    let saved = crate::preferences::save_common(&source, &text)?;
    Ok((source, saved))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_patterns_switch_and_custom_ones_stay() {
        assert_eq!(classify("%H:%M"), Some((HourCycle::H24, false)));
        assert_eq!(classify("%I:%M %p"), Some((HourCycle::H12, false)));
        assert_eq!(classify("%r"), Some((HourCycle::H12, true)));
        assert_eq!(classify("%H.%M"), None);

        let text = "schema-version 1\n// keep\nshell {\n    bar \"main\" { modules { end \"clock\"; } }\n    modules {\n        module \"clock\" { format \"{time}\"; time-format \"%H:%M:%S\"; }\n        module \"world\" { type \"clock\"; time-format \"%H.%M UTC\"; }\n    }\n}\nlockscreen {\n    dim 40\n}\n";
        let formats = ClockFormats::read(text).unwrap();
        assert_eq!(formats.clocks.len(), 3);
        assert_eq!(formats.cycle(), None, "one clock is custom");
        assert_eq!(formats.custom().count(), 1);

        let twelve = with_hour_cycle(text, HourCycle::H12).unwrap();
        assert!(twelve.contains("time-format \"%-I:%M:%S %p\""), "{twelve}");
        assert!(twelve.contains("time-format \"%H.%M UTC\""));
        assert!(twelve.contains("clock-format \"%-I:%M %p\""));
        assert!(twelve.contains("// keep") && twelve.contains("dim 40"));
        assert!(twelve.contains("format \"{time}\""));
        let back = with_hour_cycle(&twelve, HourCycle::H24).unwrap();
        let formats = ClockFormats::read(&back).unwrap();
        assert_eq!(formats.clocks[0].format, "%H:%M:%S");
        assert_eq!(formats.clocks[2].format, "%H:%M");
        // Nothing else changes.
        assert_eq!(
            crate::preferences::common_sections(text)
                .unwrap()
                .0
                .contains("dim"),
            crate::preferences::common_sections(&back)
                .unwrap()
                .0
                .contains("dim")
        );
    }

    #[test]
    fn unset_clocks_follow_the_shipped_shell() {
        // No configuration: the 24-hour default stays implicit.
        assert_eq!(
            with_hour_cycle("", HourCycle::H24).unwrap(),
            "schema-version 1\n"
        );
        assert_eq!(
            ClockFormats::read("").unwrap().cycle(),
            Some(HourCycle::H24)
        );
        let twelve = with_hour_cycle("", HourCycle::H12).unwrap();
        let formats = ClockFormats::read(&twelve).unwrap();
        assert_eq!(formats.cycle(), Some(HourCycle::H12), "{twelve}");
        // The shipped bars and modules are written out, as the Widgets page does.
        let written: KdlDocument = twelve.parse().unwrap();
        assert!(
            written
                .get("shell")
                .and_then(KdlNode::children)
                .is_some_and(|shell| shell.get("bar").is_some())
        );
        assert_eq!(lock_format(&twelve).as_deref(), Some("%-I:%M %p"));

        // A shell block without clock modules gains one.
        let text =
            "schema-version 1\nshell {\n    bar \"main\" { modules { end \"clock\"; } }\n}\n";
        let twelve = with_hour_cycle(text, HourCycle::H12).unwrap();
        assert!(twelve.contains("module"), "{twelve}");
        assert_eq!(
            ClockFormats::read(&twelve).unwrap().cycle(),
            Some(HourCycle::H12)
        );
    }
}
