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
    OpenTerminal => ("open-terminal", "files-command-open-terminal", "utilities-terminal-symbolic", []),
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
    MoveTabLeft => ("move-tab-left", "files-command-move-tab-left", "go-previous-symbolic", ["<Control><Shift>Page_Up"]),
    MoveTabRight => ("move-tab-right", "files-command-move-tab-right", "go-next-symbolic", ["<Control><Shift>Page_Down"]),
    MoveTabToWindow => ("move-tab-to-window", "files-command-move-tab-to-window", "window-new-symbolic", []),
    Operations => ("operations", "files-command-operations", "view-list-symbolic", ["<Control>j"]),
    Recovery => ("recovery", "files-command-recovery", "document-revert-symbolic", []),
    CustomActions => ("custom-actions", "files-command-custom-actions", "system-run-symbolic", ["<Control><Shift>a"]),
    Preferences => ("preferences", "files-command-preferences", "preferences-system-symbolic", ["<Control>comma"]),
    Properties => ("properties", "files-command-properties", "document-properties-symbolic", ["<Alt>Return"]),
    BookmarkFolder => ("bookmark-folder", "files-command-bookmark-folder", "bookmark-new-symbolic", ["<Control>d"]),
}

impl Command {
    /// Commands the toolbar never shows as buttons: Quick Look has Space and
    /// the menus, and Search has the location field's own button.
    pub fn never_on_toolbar(self) -> bool {
        matches!(self, Self::QuickLook | Self::Search)
    }
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

/// Where Properties shows the selection: its own window, or a panel beside
/// the browser's listing that follows the selection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PropertiesDisplay {
    #[default]
    Window,
    Panel,
}
impl PropertiesDisplay {
    pub const ALL: [Self; 2] = [Self::Window, Self::Panel];
    pub fn id(self) -> &'static str {
        match self {
            Self::Window => "window",
            Self::Panel => "panel",
        }
    }
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|display| display.id() == id)
    }
    pub fn label(self) -> String {
        match self {
            Self::Window => fl!("files-properties-display-window"),
            Self::Panel => fl!("files-properties-display-panel"),
        }
    }
}

/// List view column widths chosen by dragging their header dividers, in
/// logical pixels. `None` keeps the width that follows the window and text size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ListColumns {
    pub name: Option<u16>,
    pub size: Option<u16>,
    pub modified: Option<u16>,
    pub kind: Option<u16>,
}
impl ListColumns {
    pub const NAME: std::ops::RangeInclusive<u16> = 160..=1200;
    pub const DETAIL: std::ops::RangeInclusive<u16> = 48..=480;
    pub fn detail(&self, detail: crate::preferences::Detail) -> Option<u16> {
        match detail {
            crate::preferences::Detail::Size => self.size,
            crate::preferences::Detail::Modified => self.modified,
            crate::preferences::Detail::Kind => self.kind,
        }
    }
    pub fn detail_mut(&mut self, detail: crate::preferences::Detail) -> &mut Option<u16> {
        match detail {
            crate::preferences::Detail::Size => &mut self.size,
            crate::preferences::Detail::Modified => &mut self.modified,
            crate::preferences::Detail::Kind => &mut self.kind,
        }
    }
    fn valid(&self) -> bool {
        self.name.is_none_or(|width| Self::NAME.contains(&width))
            && [self.size, self.modified, self.kind]
                .into_iter()
                .flatten()
                .all(|width| Self::DETAIL.contains(&width))
    }
}

/// The sidebar's and the Properties side panel's widths chosen by dragging
/// their dividers, in logical pixels. `None` keeps the default width.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PaneWidths {
    pub sidebar: Option<u16>,
    pub properties: Option<u16>,
}
impl PaneWidths {
    pub const SIDEBAR: std::ops::RangeInclusive<u16> = 160..=420;
    pub const PROPERTIES: std::ops::RangeInclusive<u16> = 260..=640;
    fn valid(&self) -> bool {
        self.sidebar
            .is_none_or(|width| Self::SIDEBAR.contains(&width))
            && self
                .properties
                .is_none_or(|width| Self::PROPERTIES.contains(&width))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceKind {
    Home,
    Desktop,
    Documents,
    Downloads,
    Music,
    Pictures,
    Videos,
    Templates,
    Public,
    Filesystem,
    Trash,
    Recovery,
}
impl PlaceKind {
    /// Every built-in place, in the order Preferences offers them.
    pub const ALL: &'static [Self] = &[
        Self::Home,
        Self::Desktop,
        Self::Documents,
        Self::Downloads,
        Self::Music,
        Self::Pictures,
        Self::Videos,
        Self::Templates,
        Self::Public,
        Self::Filesystem,
        Self::Trash,
        Self::Recovery,
    ];
    /// The sidebar of a new configuration: Templates and Public are offered
    /// in Preferences but not listed until added.
    pub const DEFAULT: &'static [Self] = &[
        Self::Home,
        Self::Desktop,
        Self::Documents,
        Self::Downloads,
        Self::Music,
        Self::Pictures,
        Self::Videos,
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
            Self::Music => "music",
            Self::Pictures => "pictures",
            Self::Videos => "videos",
            Self::Templates => "templates",
            Self::Public => "public",
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
            Self::Music => fl!("files-place-music"),
            Self::Pictures => fl!("files-place-pictures"),
            Self::Videos => fl!("files-place-videos"),
            Self::Templates => fl!("files-place-templates"),
            Self::Public => fl!("files-place-public"),
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

/// The icon size of the Icons view, which thumbnails fill, in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IconSize(u16);

impl Default for IconSize {
    fn default() -> Self {
        Self(96)
    }
}

impl IconSize {
    pub const MIN: u16 = 48;
    pub const MAX: u16 = 256;

    /// `pixels`, kept within the sizes the Icons view lays out.
    pub fn new(pixels: u16) -> Self {
        Self(pixels.clamp(Self::MIN, Self::MAX))
    }

    /// A size from configuration, in pixels within the laid-out range.
    pub fn from_value(value: &str) -> Option<Self> {
        value
            .parse::<u16>()
            .ok()
            .filter(|pixels| (Self::MIN..=Self::MAX).contains(pixels))
            .map(Self)
    }

    pub fn value(self) -> u16 {
        self.0
    }

    /// Logical pixels of an Icons-view icon or thumbnail.
    pub fn pixels(self) -> f32 {
        f32::from(self.0)
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
    pub properties: PropertiesDisplay,
    pub list_columns: ListColumns,
    pub panes: PaneWidths,
}
impl Default for InterfaceSettings {
    fn default() -> Self {
        Self {
            view: ViewSettings::default(),
            toolbar: Self::DEFAULT_TOOLBAR.map(str::to_owned).to_vec(),
            places: PlaceKind::DEFAULT
                .iter()
                .copied()
                .map(Place::Builtin)
                .collect(),
            sidebar: SidebarMode::Auto,
            shortcuts: BTreeMap::new(),
            thumbnails: ThumbnailSettings::default(),
            properties: PropertiesDisplay::default(),
            list_columns: ListColumns::default(),
            panes: PaneWidths::default(),
        }
    }
}
impl InterfaceSettings {
    pub const DEFAULT_TOOLBAR: [&'static str; 6] = [
        "back",
        "forward",
        "parent",
        "location",
        "new-folder",
        "view",
    ];
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
        if !self.list_columns.valid() {
            return Err(invalid(fl!("files-config-list-columns-invalid")));
        }
        if !self.panes.valid() {
            return Err(invalid(fl!("files-config-panes-invalid")));
        }
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
                    || id == "view"
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
    /// The chord as people read it beside a menu item: "Ctrl+Shift+N",
    /// "Alt+Enter", "F2".
    pub fn display(&self) -> String {
        let mut parts = Vec::new();
        for (bit, name) in [
            (1, fl!("files-key-ctrl")),
            (2, fl!("files-key-shift")),
            (4, fl!("files-key-alt")),
            (8, fl!("files-key-super")),
        ] {
            if self.modifiers & bit != 0 {
                parts.push(name);
            }
        }
        parts.push(match self.key.as_str() {
            "Return" => fl!("files-key-enter"),
            "space" => fl!("files-key-space"),
            "Delete" => fl!("files-key-delete"),
            "BackSpace" => fl!("files-key-backspace"),
            "Escape" => fl!("files-key-escape"),
            "Tab" => fl!("files-key-tab"),
            "Page_Up" => fl!("files-key-page-up"),
            "Page_Down" => fl!("files-key-page-down"),
            "Left" => "←".into(),
            "Right" => "→".into(),
            "Up" => "↑".into(),
            "Down" => "↓".into(),
            "comma" => ",".into(),
            "period" => ".".into(),
            "slash" => "/".into(),
            "backslash" => "\\".into(),
            "minus" => "-".into(),
            "plus" => "+".into(),
            "equal" => "=".into(),
            "semicolon" => ";".into(),
            "apostrophe" => "'".into(),
            "bracketleft" => "[".into(),
            "bracketright" => "]".into(),
            "grave" => "`".into(),
            key if key.chars().count() == 1 => key.to_uppercase(),
            key => key.to_owned(),
        });
        parts.join("+")
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
