//! The 12/24-hour clock preference for Canopa (ADR 0142, amended by ADR 0145).
//!
//! The preference has no key of its own: it is the hour directive of the
//! existing format strings, each Shell clock module's `time-format` and the
//! lock screen's `clock-format` in `config.kdl`. Without a pattern a clock
//! follows the language's formats (Region & language): that is Automatic.
//! Settings recognizes the usual 24- and 12-hour patterns, switches between
//! them with or without seconds, and removes them for Automatic; any other
//! pattern is the person's own and is never rewritten.
use anyhow::{Context, Result};
use kdl::{KdlDocument, KdlNode};

/// The Shell's shipped configuration, used when `config.kdl` has no `shell`.
/// Its clocks may or may not set `time-format`; both are handled.
const SHELL_DEFAULTS: &str = include_str!("../../canopa-shell/config/default.kdl");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HourCycle {
    H24,
    H12,
}

/// What the Date & time page offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockChoice {
    /// No pattern: the clocks follow the language's formats.
    Automatic,
    /// A recognized pattern of this cycle.
    Fixed(HourCycle),
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

/// Which clock a format belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    /// A Shell clock module, by its name.
    Shell(String),
    LockScreen,
}

impl Place {
    /// "Shell clock “clock”" or "Lock screen", for the page.
    pub fn label(&self) -> String {
        match self {
            Self::Shell(name) => fl!("settings-clock-place-shell", name = name.as_str()),
            Self::LockScreen => fl!("settings-clock-place-lock"),
        }
    }
}

/// Where a clock's format comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClockFormat {
    pub place: Place,
    /// `None` when unset: the clock follows the language.
    pub format: Option<String>,
}

/// The clocks of `config.kdl`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClockFormats {
    pub clocks: Vec<ClockFormat>,
}

impl ClockFormats {
    /// The common choice: Automatic when no clock has a pattern, a cycle
    /// when every clock uses a recognized pattern of it, else `None`.
    pub fn choice(&self) -> Option<ClockChoice> {
        let mut choices = self.clocks.iter().map(|clock| match &clock.format {
            None => Some(ClockChoice::Automatic),
            Some(format) => classify(format).map(|(cycle, _)| ClockChoice::Fixed(cycle)),
        });
        let first = choices.next().flatten()?;
        choices.all(|choice| choice == Some(first)).then_some(first)
    }

    /// Clocks with a pattern of the person's own.
    pub fn custom(&self) -> impl Iterator<Item = &ClockFormat> {
        self.clocks.iter().filter(|clock| {
            clock
                .format
                .as_deref()
                .is_some_and(|format| classify(format).is_none())
        })
    }
}

/// A clock's pattern; an empty one is unset.
fn format_of(value: Option<&str>) -> Option<String> {
    value.filter(|format| !format.is_empty()).map(str::to_owned)
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
                place: Place::Shell(string(node).unwrap_or("clock").to_owned()),
                format: format_of(child_string(node, "time-format")),
            })
            .collect();
        if clocks.is_empty() {
            clocks.push(ClockFormat {
                place: Place::Shell("clock".into()),
                format: None,
            });
        }
        clocks.push(ClockFormat {
            place: Place::LockScreen,
            format: format_of(
                document
                    .get("lockscreen")
                    .and_then(|lock| child_string(lock, "clock-format")),
            ),
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

/// What becomes of one clock's pattern.
#[derive(Debug, PartialEq, Eq)]
enum Change {
    Keep,
    Set(&'static str),
    Remove,
}

/// The change for a clock showing `current` (empty is unset).
fn switched(current: Option<&str>, choice: ClockChoice) -> Change {
    let current = current.filter(|format| !format.is_empty());
    let recognized = match current {
        None => None,
        Some(format) => match classify(format) {
            Some(found) => Some(found),
            // The person's own pattern.
            None => return Change::Keep,
        },
    };
    match (choice, recognized) {
        (ClockChoice::Automatic, None) => Change::Keep,
        (ClockChoice::Automatic, Some(_)) => Change::Remove,
        (ClockChoice::Fixed(cycle), found) => {
            let seconds = found.is_some_and(|(_, seconds)| seconds);
            let next = pattern(cycle, seconds);
            if current == Some(next) {
                Change::Keep
            } else {
                Change::Set(next)
            }
        }
    }
}

/// `text` with every clock set to `choice`: recognized patterns (and, for a
/// cycle, unset ones) switched to it, or removed for Automatic. Custom
/// patterns, comments and every other setting are kept.
pub fn with_choice(text: &str, choice: ClockChoice) -> Result<String> {
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
        match switched(child_string(module, "time-format"), choice) {
            Change::Keep => {}
            Change::Set(next) => {
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
            Change::Remove => {
                if let Some(children) = module.children_mut() {
                    children
                        .nodes_mut()
                        .retain(|node| node.name().value() != "time-format");
                }
                shell_changed = true;
            }
        }
    }
    if !any_clock && let Change::Set(next) = switched(None, choice) {
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
    match switched(lock_format.as_deref(), choice) {
        Change::Keep => {}
        Change::Set(next) => {
            let lock = crate::preferences::block(&mut document, "lockscreen")?;
            set_string(lock, "clock-format", next);
        }
        Change::Remove => {
            let lock = crate::preferences::block(&mut document, "lockscreen")?;
            lock.nodes_mut()
                .retain(|node| node.name().value() != "clock-format");
        }
    }
    Ok(document.to_string())
}

/// The lock screen's clock format in `text` (empty when unset), after
/// [`with_choice`].
pub fn lock_format(text: &str) -> Option<String> {
    ClockFormats::read(text)
        .ok()?
        .clocks
        .pop()
        .map(|clock| clock.format.unwrap_or_default())
}

/// Set the clocks in `config.kdl`: validate and save it. Returns the
/// replaced and the saved versions, for the pages that share the file.
pub fn set_choice(
    paths: &crate::session::Paths,
    choice: ClockChoice,
) -> Result<(crate::storage::Snapshot, crate::storage::Snapshot)> {
    let source = crate::storage::Snapshot::load(paths.common())?;
    let text = with_choice(&source.text, choice)?;
    let saved = crate::preferences::save_common(&source, &text)?;
    Ok((source, saved))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ClockChoice::{Automatic, Fixed};

    fn formats(text: &str) -> Vec<Option<String>> {
        ClockFormats::read(text)
            .unwrap()
            .clocks
            .into_iter()
            .map(|clock| clock.format)
            .collect()
    }

    #[test]
    fn known_patterns_switch_and_custom_ones_stay() {
        assert_eq!(classify("%H:%M"), Some((HourCycle::H24, false)));
        assert_eq!(classify("%I:%M %p"), Some((HourCycle::H12, false)));
        assert_eq!(classify("%r"), Some((HourCycle::H12, true)));
        assert_eq!(classify("%H.%M"), None);

        let text = "schema-version 1\n// keep\nshell {\n    bar \"main\" { modules { end \"clock\"; } }\n    modules {\n        module \"clock\" { format \"{time}\"; time-format \"%H:%M:%S\"; }\n        module \"world\" { type \"clock\"; time-format \"%H.%M UTC\"; }\n    }\n}\nlockscreen {\n    dim 40\n}\n";
        let read = ClockFormats::read(text).unwrap();
        assert_eq!(read.clocks.len(), 3);
        assert_eq!(read.choice(), None, "one clock is custom");
        assert_eq!(read.custom().count(), 1);
        assert_eq!(read.clocks[1].place, Place::Shell("world".into()));
        assert_eq!(read.clocks[1].place.label(), "Shell clock “world”");
        assert_eq!(read.clocks[2].place.label(), "Lock screen");

        let twelve = with_choice(text, Fixed(HourCycle::H12)).unwrap();
        assert!(twelve.contains("time-format \"%-I:%M:%S %p\""), "{twelve}");
        assert!(twelve.contains("time-format \"%H.%M UTC\""));
        // An unset lock-screen clock gets the explicit pattern.
        assert!(twelve.contains("clock-format \"%-I:%M %p\""));
        assert!(twelve.contains("// keep") && twelve.contains("dim 40"));
        assert!(twelve.contains("format \"{time}\""));
        let back = with_choice(&twelve, Fixed(HourCycle::H24)).unwrap();
        let read = formats(&back);
        assert_eq!(read[0].as_deref(), Some("%H:%M:%S"));
        assert_eq!(read[2].as_deref(), Some("%H:%M"));
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
    fn automatic_removes_recognized_patterns_only() {
        let text = "schema-version 1\nshell {\n    modules {\n        module \"clock\" { format \"{time}\"; time-format \"%-I:%M:%S %p\"; }\n        module \"world\" { type \"clock\"; time-format \"%H.%M UTC\"; }\n    }\n}\nlockscreen {\n    clock-format \"%H:%M\"\n    dim 40\n}\n";
        let automatic = with_choice(text, Automatic).unwrap();
        let read = formats(&automatic);
        assert_eq!(
            read,
            [None, Some("%H.%M UTC".to_owned()), None],
            "{automatic}"
        );
        assert!(automatic.contains("format \"{time}\"") && automatic.contains("dim 40"));
        let read = ClockFormats::read(&automatic).unwrap();
        // The custom clock still disagrees, and is still named.
        assert_eq!(read.choice(), None);
        assert_eq!(read.custom().count(), 1);
        assert_eq!(lock_format(&automatic).as_deref(), Some(""));
        // Without the custom clock every clock is automatic.
        let plain = "schema-version 1\nshell {\n    modules {\n        module \"clock\" { time-format \"%H:%M\"; }\n    }\n}\n";
        let automatic = with_choice(plain, Automatic).unwrap();
        assert_eq!(
            ClockFormats::read(&automatic).unwrap().choice(),
            Some(Automatic)
        );
        assert_eq!(with_choice(&automatic, Automatic).unwrap(), automatic);
        // An empty pattern is unset.
        assert_eq!(
            ClockFormats::read("schema-version 1\nlockscreen { clock-format \"\"; }\n")
                .unwrap()
                .clocks[1]
                .format,
            None
        );
    }

    #[test]
    fn unset_clocks_follow_the_shipped_shell() {
        // Whether or not the shipped Shell sets time-format, an empty
        // configuration reads as one choice, and Automatic leaves the clocks
        // without a pattern.
        let shipped = ClockFormats::read("").unwrap().choice();
        assert!(
            matches!(shipped, Some(Automatic | Fixed(HourCycle::H24))),
            "{shipped:?}"
        );
        let automatic = with_choice("", Automatic).unwrap();
        assert_eq!(
            ClockFormats::read(&automatic).unwrap().choice(),
            Some(Automatic),
            "{automatic}"
        );
        let twelve = with_choice("", Fixed(HourCycle::H12)).unwrap();
        let read = ClockFormats::read(&twelve).unwrap();
        assert_eq!(read.choice(), Some(Fixed(HourCycle::H12)), "{twelve}");
        // The shipped bars and modules are written out, as the Widgets page does.
        let written: KdlDocument = twelve.parse().unwrap();
        assert!(
            written
                .get("shell")
                .and_then(KdlNode::children)
                .is_some_and(|shell| shell.get("bar").is_some())
        );
        assert_eq!(lock_format(&twelve).as_deref(), Some("%-I:%M %p"));
        // A 24-hour choice is explicit too: unset clocks follow the language.
        let read = ClockFormats::read(&with_choice("", Fixed(HourCycle::H24)).unwrap()).unwrap();
        assert_eq!(read.choice(), Some(Fixed(HourCycle::H24)));

        // A shell block without clock modules gains one.
        let text =
            "schema-version 1\nshell {\n    bar \"main\" { modules { end \"clock\"; } }\n}\n";
        let twelve = with_choice(text, Fixed(HourCycle::H12)).unwrap();
        assert!(twelve.contains("module"), "{twelve}");
        assert_eq!(
            ClockFormats::read(&twelve).unwrap().choice(),
            Some(Fixed(HourCycle::H12))
        );
    }
}
