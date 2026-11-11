//! User-configurable commands, places and view defaults, independent of the toolkit.
use crate::{actions::CustomAction, preferences::ViewSettings};
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    path::PathBuf,
};

macro_rules! commands {
    ($($name:ident => ($id:literal, $label:literal, $icon:literal, [$($key:literal),*])),* $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
        pub enum Command { $($name),* }
        impl Command {
            pub const ALL: &'static [Self] = &[$(Self::$name),*];
            pub fn id(self) -> &'static str { match self { $(Self::$name => $id),* } }
            pub fn label(self) -> &'static str { match self { $(Self::$name => $label),* } }
            pub fn icon(self) -> &'static str { match self { $(Self::$name => $icon),* } }
            pub fn defaults(self) -> &'static [&'static str] { match self { $(Self::$name => &[$($key),*]),* } }
            pub fn from_id(id: &str) -> Option<Self> { Self::ALL.iter().copied().find(|command| command.id() == id) }
        }
    }
}
commands! {
    Back => ("back", "Back", "go-previous-symbolic", ["<Alt>Left"]),
    Forward => ("forward", "Forward", "go-next-symbolic", ["<Alt>Right"]),
    Parent => ("parent", "Parent folder", "go-up-symbolic", ["Left", "BackSpace", "<Alt>Up"]),
    EnterFolder => ("enter-folder", "Enter folder", "go-next-symbolic", ["Right"]),
    Open => ("open", "Open selection", "document-open-symbolic", ["Return"]),
    QuickLook => ("quick-look", "Quick Look", "view-reveal-symbolic", ["space"]),
    Search => ("search", "Search filenames", "system-search-symbolic", ["<Control>f"]),
    SearchSubfolders => ("search-subfolders", "Include subfolders in search", "folder-open-symbolic", []),
    StopSearch => ("stop-search", "Stop search", "process-stop-symbolic", []),
    MoreSearchResults => ("more-search-results", "More search results", "list-add-symbolic", []),
    RevealSearchResult => ("reveal-search-result", "Show in containing folder", "folder-open-symbolic", []),
    SearchProblems => ("search-problems", "Unreadable search locations", "dialog-warning-symbolic", []),
    Location => ("location-focus", "Edit location", "folder-open-symbolic", ["<Control>l"]),
    Refresh => ("refresh", "Refresh", "view-refresh-symbolic", ["F5"]),
    ShowHidden => ("show-hidden", "Show hidden files", "view-reveal-symbolic", ["<Control>h"]),
    SelectAll => ("select-all", "Select all", "edit-select-all-symbolic", ["<Control>a"]),
    NewFolder => ("new-folder", "New folder", "folder-new-symbolic", ["<Control><Shift>n"]),
    Rename => ("rename", "Rename", "document-edit-symbolic", ["F2"]),
    Trash => ("trash", "Move to Trash", "user-trash-symbolic", ["Delete"]),
    DeletePermanently => ("delete-permanently", "Delete permanently", "edit-delete-symbolic", ["<Shift>Delete"]),
    ShowTrash => ("show-trash", "Open Trash", "user-trash-symbolic", []),
    Undo => ("undo", "Undo", "edit-undo-symbolic", ["<Control>z"]),
    Copy => ("copy", "Copy", "edit-copy-symbolic", ["<Control>c"]),
    Cut => ("cut", "Cut", "edit-cut-symbolic", ["<Control>x"]),
    Paste => ("paste", "Paste", "edit-paste-symbolic", ["<Control>v"]),
    Columns => ("columns", "Columns view", "view-dual-symbolic", ["<Control>1"]),
    List => ("list", "List view", "view-list-symbolic", ["<Control>2"]),
    Icons => ("icons", "Icons view", "view-grid-symbolic", ["<Control>3"]),
    NewTab => ("new-tab", "New tab", "tab-new-symbolic", ["<Control>t"]),
    NewWindow => ("new-window", "New window", "window-new-symbolic", ["<Control>n"]),
    CloseTab => ("close-tab", "Close tab", "window-close-symbolic", ["<Control>w"]),
    NextTab => ("next-tab", "Next tab", "go-next-symbolic", ["<Control>Tab"]),
    PreviousTab => ("previous-tab", "Previous tab", "go-previous-symbolic", ["<Control><Shift>Tab"]),
    Operations => ("operations", "Operations", "view-list-symbolic", ["<Control>j"]),
    Recovery => ("recovery", "Recovery", "document-revert-symbolic", []),
    CustomActions => ("custom-actions", "Custom actions", "system-run-symbolic", ["<Control><Shift>a"]),
    Preferences => ("preferences", "Preferences", "preferences-system-symbolic", ["<Control>comma"]),
    BookmarkFolder => ("bookmark-folder", "Bookmark this folder", "bookmark-new-symbolic", ["<Control>d"]),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SidebarMode {
    #[default]
    Auto,
    Always,
    Never,
}
impl SidebarMode {
    pub fn id(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Always => "always",
            Self::Never => "never",
        }
    }
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "auto" => Some(Self::Auto),
            "always" => Some(Self::Always),
            "never" => Some(Self::Never),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceKind {
    Home,
    Desktop,
    Documents,
    Downloads,
    Pictures,
    Filesystem,
    Trash,
    Recovery,
}
impl PlaceKind {
    pub const ALL: &'static [Self] = &[
        Self::Home,
        Self::Desktop,
        Self::Documents,
        Self::Downloads,
        Self::Pictures,
        Self::Filesystem,
        Self::Trash,
        Self::Recovery,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Desktop => "desktop",
            Self::Documents => "documents",
            Self::Downloads => "downloads",
            Self::Pictures => "pictures",
            Self::Filesystem => "filesystem",
            Self::Trash => "trash",
            Self::Recovery => "recovery",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Desktop => "Desktop",
            Self::Documents => "Documents",
            Self::Downloads => "Downloads",
            Self::Pictures => "Pictures",
            Self::Filesystem => "Filesystem",
            Self::Trash => "Trash",
            Self::Recovery => "Recovery",
        }
    }
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|place| place.id() == id)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    Builtin(PlaceKind),
    Bookmark { name: String, path: PathBuf },
}
impl Place {
    pub fn label(&self) -> &str {
        match self {
            Self::Builtin(place) => place.label(),
            Self::Bookmark { name, .. } => name,
        }
    }
}

/// Which files get thumbnails.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThumbnailPolicy {
    /// Files on local disks; not on network shares or removable media.
    #[default]
    Local,
    Always,
    Never,
}

impl ThumbnailPolicy {
    pub const ALL: [Self; 3] = [Self::Local, Self::Always, Self::Never];

    pub fn id(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Always => "always",
            Self::Never => "never",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|policy| policy.id() == id)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Local => "Local files only",
            Self::Always => "All files",
            Self::Never => "Never",
        }
    }
}

/// The icon size of the Icons view, which thumbnails fill.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IconSize {
    Small,
    #[default]
    Medium,
    Large,
    Huge,
}

impl IconSize {
    pub const ALL: [Self; 4] = [Self::Small, Self::Medium, Self::Large, Self::Huge];

    pub fn id(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
            Self::Huge => "huge",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|size| size.id() == id)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Small => "Small",
            Self::Medium => "Medium",
            Self::Large => "Large",
            Self::Huge => "Huge",
        }
    }

    /// Logical pixels of an Icons-view icon or thumbnail.
    pub fn pixels(self) -> f32 {
        match self {
            Self::Small => 48.0,
            Self::Medium => 64.0,
            Self::Large => 96.0,
            Self::Huge => 128.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ThumbnailSettings {
    pub show: ThumbnailPolicy,
    pub size: IconSize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterfaceSettings {
    pub view: ViewSettings,
    /// Command IDs, `action:<id>` references and exactly one `location` field.
    pub toolbar: Vec<String>,
    pub places: Vec<Place>,
    pub sidebar: SidebarMode,
    /// Absent means default; an empty list disables a command's bindings.
    pub shortcuts: BTreeMap<Command, Vec<String>>,
    pub thumbnails: ThumbnailSettings,
}
impl Default for InterfaceSettings {
    fn default() -> Self {
        Self {
            view: ViewSettings::default(),
            toolbar: ["back", "forward", "parent", "location", "search"]
                .map(str::to_owned)
                .to_vec(),
            places: PlaceKind::ALL.iter().copied().map(Place::Builtin).collect(),
            sidebar: SidebarMode::Auto,
            shortcuts: BTreeMap::new(),
            thumbnails: ThumbnailSettings::default(),
        }
    }
}
impl InterfaceSettings {
    pub fn bindings(&self, command: Command) -> Vec<String> {
        self.shortcuts
            .get(&command)
            .cloned()
            .unwrap_or_else(|| command.defaults().iter().map(|s| (*s).to_owned()).collect())
    }
    pub fn command_for(&self, chord: &KeyChord) -> Option<Command> {
        Command::ALL.iter().copied().find(|command| {
            self.bindings(*command)
                .iter()
                .any(|binding| KeyChord::parse(binding).is_ok_and(|parsed| &parsed == chord))
        })
    }
    pub fn validate(&self, actions: &[CustomAction]) -> io::Result<()> {
        self.view.validate()?;
        if self.toolbar.len() > 64
            || self
                .toolbar
                .iter()
                .filter(|id| id.as_str() == "location")
                .count()
                != 1
        {
            return Err(invalid(
                "the toolbar needs exactly one location field and at most 64 items",
            ));
        }
        let mut seen = BTreeSet::new();
        for id in &self.toolbar {
            if !seen.insert(id)
                || !(id == "location"
                    || Command::from_id(id).is_some()
                    || id
                        .strip_prefix("action:")
                        .is_some_and(|id| actions.iter().any(|action| action.id == id)))
            {
                return Err(invalid("unknown or duplicate toolbar item"));
            }
        }
        if self.places.len() > 256 {
            return Err(invalid("at most 256 locations are supported"));
        }
        let mut builtins = BTreeSet::new();
        for place in &self.places {
            match place {
                Place::Builtin(kind) if !builtins.insert(kind.id()) => {
                    return Err(invalid("duplicate built-in location"));
                }
                Place::Bookmark { name, path }
                    if name.trim().is_empty()
                        || name.len() > 128
                        || name.contains('\0')
                        || !path.is_absolute()
                        || path.as_os_str().as_encoded_bytes().len() > 16384
                        || path.as_os_str().as_encoded_bytes().contains(&0) =>
                {
                    return Err(invalid(
                        "a bookmark needs a name and an absolute local folder",
                    ));
                }
                _ => {}
            }
        }
        let mut shortcuts = BTreeMap::new();
        for command in Command::ALL {
            let bindings = self.bindings(*command);
            if bindings.len() > 8 {
                return Err(invalid("at most eight shortcuts per command are supported"));
            }
            for binding in bindings {
                register(&mut shortcuts, &binding, command.label())?;
            }
        }
        for action in actions {
            if let Some(binding) = &action.shortcut {
                let chord = KeyChord::parse(binding)?;
                if !chord.can_bind_action() {
                    return Err(invalid(
                        "custom actions need a Ctrl, Alt, Super or function-key shortcut",
                    ));
                }
                register(&mut shortcuts, binding, &action.name)?;
            }
        }
        Ok(())
    }
}
fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}
fn register(seen: &mut BTreeMap<KeyChord, String>, binding: &str, label: &str) -> io::Result<()> {
    let chord = KeyChord::parse(binding)?;
    if !chord.has_command_modifier()
        && (matches!(
            chord.key.as_str(),
            "Up" | "Down" | "Home" | "End" | "Escape" | "Menu" | "Tab"
        ) || chord.key == "F10" && chord.modifiers & 2 != 0
            || chord.key.len() == 1
            || matches!(
                chord.key.as_str(),
                "comma"
                    | "period"
                    | "slash"
                    | "backslash"
                    | "minus"
                    | "plus"
                    | "equal"
                    | "semicolon"
                    | "apostrophe"
                    | "bracketleft"
                    | "bracketright"
                    | "grave"
            ))
    {
        return Err(invalid(
            "selection, focus, Escape, context-menu and unmodified typing keys stay available",
        ));
    }
    if let Some(other) = seen.insert(chord, label.to_owned()) {
        return Err(invalid(format!(
            "{label} and {other} use the same shortcut ({binding})"
        )));
    }
    Ok(())
}

/// Canonical accelerator, also used by the native key adapter. Lock modifiers are ignored there.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct KeyChord {
    modifiers: u8,
    key: String,
}
impl KeyChord {
    pub fn parse(input: &str) -> io::Result<Self> {
        let mut rest = input.trim();
        let mut modifiers = 0;
        if rest.len() > 128 {
            return Err(invalid("shortcut is too long"));
        }
        while let Some(tail) = rest.strip_prefix('<') {
            let (name, tail) = tail
                .split_once('>')
                .ok_or_else(|| invalid("invalid shortcut modifier"))?;
            let bit = match name.to_ascii_lowercase().as_str() {
                "control" | "ctrl" | "primary" => 1,
                "shift" => 2,
                "alt" | "mod1" => 4,
                "super" | "mod4" => 8,
                _ => return Err(invalid("unsupported shortcut modifier")),
            };
            if modifiers & bit != 0 {
                return Err(invalid("duplicate shortcut modifier"));
            }
            modifiers |= bit;
            rest = tail;
        }
        let lower = rest.to_ascii_lowercase();
        let key = match lower.as_str() {
            "enter" | "return" | "kp_enter" => "Return",
            "tab" | "kp_tab" => "Tab",
            "iso_left_tab" => {
                modifiers |= 2;
                "Tab"
            }
            "left" | "kp_left" => "Left",
            "right" | "kp_right" => "Right",
            "up" | "kp_up" => "Up",
            "down" | "kp_down" => "Down",
            "home" | "kp_home" => "Home",
            "end" | "kp_end" => "End",
            "page_up" | "prior" | "kp_page_up" => "Page_Up",
            "page_down" | "next" | "kp_page_down" => "Page_Down",
            "backspace" => "BackSpace",
            "delete" | "kp_delete" => "Delete",
            "insert" | "kp_insert" => "Insert",
            "escape" | "esc" => "Escape",
            "space" | " " => "space",
            "menu" => "Menu",
            "," | "comma" => "comma",
            "." | "period" => "period",
            "/" | "slash" => "slash",
            "\\" | "backslash" => "backslash",
            "-" | "minus" => "minus",
            "+" | "plus" => "plus",
            "=" | "equal" => "equal",
            ";" | "semicolon" => "semicolon",
            "'" | "apostrophe" => "apostrophe",
            "[" | "bracketleft" => "bracketleft",
            "]" | "bracketright" => "bracketright",
            "`" | "grave" => "grave",
            _ if lower.len() == 1 && lower.as_bytes()[0].is_ascii_alphanumeric() => lower.as_str(),
            _ if lower
                .strip_prefix("kp_")
                .is_some_and(|n| n.len() == 1 && n.as_bytes()[0].is_ascii_digit()) =>
            {
                &lower[3..]
            }
            _ if lower
                .strip_prefix('f')
                .and_then(|n| n.parse::<u8>().ok().filter(|key| n == key.to_string()))
                .is_some_and(|n| (1..=24).contains(&n)) =>
            {
                return Ok(Self {
                    modifiers,
                    key: lower.to_ascii_uppercase(),
                });
            }
            _ => {
                return Err(invalid(
                    "use an accelerator such as <Control><Shift>f or F6",
                ));
            }
        }
        .to_owned();
        Ok(Self { modifiers, key })
    }
    pub fn accelerator(&self) -> String {
        let mut text = String::new();
        for (bit, name) in [
            (1, "<Control>"),
            (2, "<Shift>"),
            (4, "<Alt>"),
            (8, "<Super>"),
        ] {
            if self.modifiers & bit != 0 {
                text.push_str(name);
            }
        }
        text.push_str(&self.key);
        text
    }
    /// Selection and ordinary typing remain available to the browser and editors.
    pub(crate) fn can_bind_action(&self) -> bool {
        self.has_command_modifier() || self.is_function()
    }
    fn has_command_modifier(&self) -> bool {
        self.modifiers & (1 | 4 | 8) != 0
    }
    fn is_function(&self) -> bool {
        self.key.starts_with('F') && self.key[1..].parse::<u8>().is_ok()
    }
}
