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
            /// The translated label; `$label` is its Fluent message ID.
            pub fn label(self) -> String { match self { $(Self::$name => fl!($label)),* } }
            pub fn icon(self) -> &'static str { match self { $(Self::$name => $icon),* } }
            pub fn defaults(self) -> &'static [&'static str] { match self { $(Self::$name => &[$($key),*]),* } }
            pub fn from_id(id: &str) -> Option<Self> { Self::ALL.iter().copied().find(|command| command.id() == id) }
        }
    }
}
commands! {
    Back => ("back", "files-command-back", "go-previous-symbolic", ["<Alt>Left"]),
    Forward => ("forward", "files-command-forward", "go-next-symbolic", ["<Alt>Right"]),
    Parent => ("parent", "files-command-parent", "go-up-symbolic", ["Left", "BackSpace", "<Alt>Up"]),
    EnterFolder => ("enter-folder", "files-command-enter-folder", "go-next-symbolic", ["Right"]),
    Open => ("open", "files-command-open", "document-open-symbolic", ["Return"]),
    QuickLook => ("quick-look", "files-command-quick-look", "view-reveal-symbolic", ["space"]),
    Search => ("search", "files-command-search", "system-search-symbolic", ["<Control>f"]),
    SearchSubfolders => ("search-subfolders", "files-command-search-subfolders", "folder-open-symbolic", []),
    StopSearch => ("stop-search", "files-command-stop-search", "process-stop-symbolic", []),
    MoreSearchResults => ("more-search-results", "files-command-more-search-results", "list-add-symbolic", []),
    RevealSearchResult => ("reveal-search-result", "files-command-reveal-search-result", "folder-open-symbolic", []),
    SearchProblems => ("search-problems", "files-command-search-problems", "dialog-warning-symbolic", []),
    Location => ("location-focus", "files-command-location-focus", "folder-open-symbolic", ["<Control>l"]),
    Refresh => ("refresh", "files-command-refresh", "view-refresh-symbolic", ["F5"]),
    ShowHidden => ("show-hidden", "files-command-show-hidden", "view-reveal-symbolic", ["<Control>h"]),
    SelectAll => ("select-all", "files-command-select-all", "edit-select-all-symbolic", ["<Control>a"]),
    NewFolder => ("new-folder", "files-command-new-folder", "folder-new-symbolic", ["<Control><Shift>n"]),
    Rename => ("rename", "files-command-rename", "document-edit-symbolic", ["F2"]),
    Trash => ("trash", "files-command-trash", "user-trash-symbolic", ["Delete"]),
    DeletePermanently => ("delete-permanently", "files-command-delete-permanently", "edit-delete-symbolic", ["<Shift>Delete"]),
    ShowTrash => ("show-trash", "files-command-show-trash", "user-trash-symbolic", []),
    ConnectToServer => ("connect-to-server", "files-command-connect-to-server", "network-server-symbolic", []),
    Undo => ("undo", "files-command-undo", "edit-undo-symbolic", ["<Control>z"]),
    Copy => ("copy", "files-command-copy", "edit-copy-symbolic", ["<Control>c"]),
    Cut => ("cut", "files-command-cut", "edit-cut-symbolic", ["<Control>x"]),
    Paste => ("paste", "files-command-paste", "edit-paste-symbolic", ["<Control>v"]),
    Columns => ("columns", "files-command-columns", "view-dual-symbolic", ["<Control>1"]),
    List => ("list", "files-command-list", "view-list-symbolic", ["<Control>2"]),
    Icons => ("icons", "files-command-icons", "view-grid-symbolic", ["<Control>3"]),
    NewTab => ("new-tab", "files-command-new-tab", "tab-new-symbolic", ["<Control>t"]),
    NewWindow => ("new-window", "files-command-new-window", "window-new-symbolic", ["<Control>n"]),
    CloseTab => ("close-tab", "files-command-close-tab", "window-close-symbolic", ["<Control>w"]),
    NextTab => ("next-tab", "files-command-next-tab", "go-next-symbolic", ["<Control>Tab"]),
    PreviousTab => ("previous-tab", "files-command-previous-tab", "go-previous-symbolic", ["<Control><Shift>Tab"]),
    Operations => ("operations", "files-command-operations", "view-list-symbolic", ["<Control>j"]),
    Recovery => ("recovery", "files-command-recovery", "document-revert-symbolic", []),
    CustomActions => ("custom-actions", "files-command-custom-actions", "system-run-symbolic", ["<Control><Shift>a"]),
    Preferences => ("preferences", "files-command-preferences", "preferences-system-symbolic", ["<Control>comma"]),
    BookmarkFolder => ("bookmark-folder", "files-command-bookmark-folder", "bookmark-new-symbolic", ["<Control>d"]),
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
    pub fn label(self) -> String {
        match self {
            Self::Home => fl!("files-place-home"),
            Self::Desktop => fl!("files-place-desktop"),
            Self::Documents => fl!("files-place-documents"),
            Self::Downloads => fl!("files-place-downloads"),
            Self::Pictures => fl!("files-place-pictures"),
            Self::Filesystem => fl!("files-place-filesystem"),
            Self::Trash => fl!("files-place-trash"),
            Self::Recovery => fl!("files-place-recovery"),
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
    pub fn label(&self) -> String {
        match self {
            Self::Builtin(place) => place.label(),
            Self::Bookmark { name, .. } => name.clone(),
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

    pub fn label(self) -> String {
        match self {
            Self::Local => fl!("files-thumbnails-local"),
            Self::Always => fl!("files-thumbnails-always"),
            Self::Never => fl!("files-thumbnails-never"),
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

    pub fn label(self) -> String {
        match self {
            Self::Small => fl!("files-icon-size-small"),
            Self::Medium => fl!("files-icon-size-medium"),
            Self::Large => fl!("files-icon-size-large"),
            Self::Huge => fl!("files-icon-size-huge"),
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
            return Err(invalid(fl!("files-config-toolbar-location")));
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
                return Err(invalid(fl!("files-config-toolbar-unknown")));
            }
        }
        if self.places.len() > 256 {
            return Err(invalid(fl!("files-config-too-many-places")));
        }
        let mut builtins = BTreeSet::new();
        for place in &self.places {
            match place {
                Place::Builtin(kind) if !builtins.insert(kind.id()) => {
                    return Err(invalid(fl!("files-config-duplicate-place")));
                }
                Place::Bookmark { name, path }
                    if name.trim().is_empty()
                        || name.len() > 128
                        || name.contains('\0')
                        || !path.is_absolute()
                        || path.as_os_str().as_encoded_bytes().len() > 16384
                        || path.as_os_str().as_encoded_bytes().contains(&0) =>
                {
                    return Err(invalid(fl!("files-config-bookmark-invalid")));
                }
                _ => {}
            }
        }
        let mut shortcuts = BTreeMap::new();
        for command in Command::ALL {
            let bindings = self.bindings(*command);
            if bindings.len() > 8 {
                return Err(invalid(fl!("files-config-too-many-shortcuts")));
            }
            for binding in bindings {
                register(&mut shortcuts, &binding, &command.label())?;
            }
        }
        for action in actions {
            if let Some(binding) = &action.shortcut {
                let chord = KeyChord::parse(binding)?;
                if !chord.can_bind_action() {
                    return Err(invalid(fl!("files-config-action-modifier")));
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
        return Err(invalid(fl!("files-config-reserved-keys")));
    }
    if let Some(other) = seen.insert(chord, label.to_owned()) {
        return Err(invalid(fl!(
            "files-config-shortcut-conflict",
            label = label.to_string(),
            other = other.to_string(),
            binding = binding.to_string()
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
            return Err(invalid(fl!("files-config-shortcut-too-long")));
        }
        while let Some(tail) = rest.strip_prefix('<') {
            let (name, tail) = tail
                .split_once('>')
                .ok_or_else(|| invalid(fl!("files-config-modifier-invalid")))?;
            let bit = match name.to_ascii_lowercase().as_str() {
                "control" | "ctrl" | "primary" => 1,
                "shift" => 2,
                "alt" | "mod1" => 4,
                "super" | "mod4" => 8,
                _ => return Err(invalid(fl!("files-config-modifier-unsupported"))),
            };
            if modifiers & bit != 0 {
                return Err(invalid(fl!("files-config-modifier-duplicate")));
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
                return Err(invalid(fl!("files-config-accelerator-hint")));
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
