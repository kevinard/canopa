//! Every Settings page is declared once, here.
//!
//! The sidebar, its groups and Ctrl+1–9 order, search, command-line and D-Bus
//! page ids, page titles and descriptions all come from [`PAGES`]; titles,
//! descriptions and translated keywords are Fluent messages
//! (`settings-page-<id>-title`, `-description`, `-keywords`). A page's
//! [`Kind`] selects its building block: a file-backed draft with Save
//! ([`crate::file_page`]) or a live service page ([`crate::live`]).

/// A page's stable identity. Its [`Entry::id`] is the public deep-link name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Page {
    Displays,
    Input,
    Network,
    Bluetooth,
    Sound,
    Personal,
    Accessibility,
    Power,
    DateTime,
    Notifications,
    DefaultApps,
    Launcher,
    Shortcuts,
    Widgets,
    About,
    Region,
    Users,
    Privacy,
    Printers,
    /// The live-page pattern on a fake service; debug builds only.
    #[cfg(debug_assertions)]
    LiveExample,
}

/// Sidebar sections, in display order. Empty groups are not shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Group {
    Devices,
    Input,
    Appearance,
    System,
    Desktop,
    #[cfg(debug_assertions)]
    Developer,
}

impl Group {
    pub const ALL: &[Self] = &[
        Self::Devices,
        Self::Input,
        Self::Appearance,
        Self::System,
        Self::Desktop,
        #[cfg(debug_assertions)]
        Self::Developer,
    ];

    /// The section heading, in the selected language.
    pub fn title(self) -> String {
        match self {
            Self::Devices => fl!("settings-group-devices"),
            Self::Input => fl!("settings-group-input"),
            Self::Appearance => fl!("settings-group-appearance"),
            Self::System => fl!("settings-group-system"),
            Self::Desktop => fl!("settings-group-desktop"),
            #[cfg(debug_assertions)]
            Self::Developer => fl!("settings-group-developer"),
        }
    }
}

/// A file a file-backed page reads and saves. Pages that share a document
/// follow the shared-document rule in [`crate::file_page`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Document {
    /// The shared Canopa `config.kdl`.
    Common,
    /// The managed Niri input include.
    Input,
    /// The Canopa session's main Niri configuration.
    Niri,
    /// The managed Niri display include and the display profiles.
    Displays,
    /// The Stasis idle policy.
    Idle,
    /// The session's lid-close choice.
    Power,
    /// Launcher preferences and clipboard history settings.
    Launcher,
    /// The shortcut changes over the shipped defaults.
    Shortcuts,
}

/// How a file-backed page applies its draft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Apply {
    /// Validate and save the draft.
    Save,
    /// Apply temporarily and save only after the user keeps the result.
    Trial,
}

/// The page pattern a page is built from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Edit drafts of these documents, then Save. The footer offers Save and
    /// "Reload this page".
    File {
        documents: &'static [Document],
        apply: Apply,
    },
    /// State comes from a system service and each change applies at once.
    /// There is no footer; each control reports its own progress and errors.
    Live,
}

/// A page's declaration.
#[derive(Debug)]
pub struct Entry {
    pub page: Page,
    /// Stable lowercase name for `canopa-settings <id>`, `--page <id>` and the
    /// D-Bus `OpenPage` method. Never rename one; existing callers use them.
    pub id: &'static str,
    /// The English title. Shown through [`Entry::title`]; searched in every
    /// language so English words keep finding pages.
    pub english: &'static str,
    /// A freedesktop icon name for launchers and, later, the sidebar.
    pub icon: &'static str,
    pub group: Group,
    /// Extra lowercase English search terms, searched in every language
    /// beside [`Entry::keywords_translated`]; titles are always searched.
    pub keywords: &'static [&'static str],
    pub kind: Kind,
    /// Reachable by id only: not in the sidebar, search or Ctrl+1–9.
    pub hidden: bool,
}

/// All pages. Within a group, pages appear in this order.
pub static PAGES: &[Entry] = &[
    Entry {
        page: Page::Displays,
        id: "displays",
        english: "Displays",
        icon: "preferences-desktop-display",
        group: Group::Devices,
        keywords: &[
            "monitor",
            "screen",
            "resolution",
            "refresh rate",
            "scale",
            "rotation",
            "position",
            "arrangement",
            "profile",
            "dock",
        ],
        kind: Kind::File {
            documents: &[Document::Displays],
            apply: Apply::Trial,
        },
        hidden: false,
    },
    Entry {
        page: Page::Input,
        id: "input",
        english: "Keyboard & pointer",
        icon: "input-keyboard",
        group: Group::Devices,
        keywords: &[
            "keyboard",
            "layout",
            "xkb",
            "repeat",
            "mouse",
            "touchpad",
            "trackpad",
            "pointer",
            "tap to click",
            "natural scrolling",
            "left-handed",
        ],
        kind: Kind::File {
            documents: &[Document::Input],
            apply: Apply::Save,
        },
        hidden: false,
    },
    Entry {
        page: Page::Network,
        id: "network",
        english: "Network",
        icon: "preferences-system-network",
        group: Group::Devices,
        keywords: &[
            "wifi",
            "wi-fi",
            "wireless",
            "internet",
            "ethernet",
            "wired",
            "cable",
            "ip address",
            "dns",
            "dhcp",
            "vpn",
            "wireguard",
            "openvpn",
            "ipsec",
            "hotspot",
            "tethering",
            "eduroam",
            "enterprise",
            "802.1x",
            "certificate",
            "metered",
            "proxy",
            "airplane",
        ],
        kind: Kind::Live,
        hidden: false,
    },
    Entry {
        page: Page::Bluetooth,
        id: "bluetooth",
        english: "Bluetooth",
        icon: "preferences-system-bluetooth",
        group: Group::Devices,
        keywords: &[
            "wireless",
            "pair",
            "pairing",
            "headphones",
            "headset",
            "earbuds",
            "speaker",
            "keyboard",
            "mouse",
            "controller",
            "gamepad",
            "phone",
            "battery",
            "discoverable",
            "visible",
            "trust",
            "forget",
            "pin",
        ],
        kind: Kind::Live,
        hidden: false,
    },
    Entry {
        page: Page::Printers,
        id: "printers",
        english: "Printers",
        icon: "printer",
        group: Group::Devices,
        keywords: &[
            "print",
            "printing",
            "cups",
            "queue",
            "jobs",
            "test page",
            "ipp",
            "airprint",
            "driverless",
            "scanner",
        ],
        kind: Kind::Live,
        hidden: false,
    },
    Entry {
        page: Page::Sound,
        id: "sound",
        english: "Sound",
        icon: "preferences-desktop-sound",
        group: Group::Devices,
        keywords: &[
            "audio",
            "volume",
            "speakers",
            "headphones",
            "headset",
            "microphone",
            "input level",
            "output",
            "mute",
            "balance",
            "hdmi",
            "bluetooth",
            "a2dp",
            "hfp",
            "profile",
            "applications",
            "alert sound",
            "notification sound",
        ],
        kind: Kind::Live,
        hidden: false,
    },
    Entry {
        page: Page::Personal,
        id: "personal",
        english: "Personalization",
        icon: "preferences-desktop-theme",
        group: Group::Appearance,
        keywords: &[
            "appearance",
            "theme",
            "dark",
            "light",
            "accent",
            "color",
            "wallpaper",
            "background",
            "lock screen",
            "clock",
            "text size",
            "font",
            "motion",
            "animation",
        ],
        kind: Kind::File {
            documents: &[Document::Common, Document::Niri],
            apply: Apply::Save,
        },
        hidden: false,
    },
    Entry {
        page: Page::Accessibility,
        id: "accessibility",
        english: "Accessibility",
        icon: "preferences-desktop-accessibility",
        group: Group::Appearance,
        keywords: &[
            "a11y",
            "contrast",
            "high contrast",
            "text size",
            "larger text",
            "cursor",
            "pointer size",
            "motion",
            "animation",
            "screen reader",
            "orca",
            "vision",
        ],
        kind: Kind::File {
            documents: &[Document::Common, Document::Input, Document::Niri],
            apply: Apply::Save,
        },
        hidden: false,
    },
    Entry {
        page: Page::Power,
        id: "power",
        english: "Power",
        icon: "preferences-system-power",
        group: Group::System,
        keywords: &[
            "battery",
            "health",
            "charge",
            "power mode",
            "power profile",
            "power saver",
            "performance",
            "brightness",
            "backlight",
            "lid",
            "power button",
            "idle",
            "sleep",
            "suspend",
            "hibernate",
            "lock",
            "screen off",
            "timeout",
            "stasis",
        ],
        // Battery, power mode and brightness apply at once; the lid choice
        // and idle timeouts are saved.
        kind: Kind::File {
            documents: &[Document::Idle, Document::Power],
            apply: Apply::Save,
        },
        hidden: false,
    },
    Entry {
        page: Page::DateTime,
        id: "date-time",
        english: "Date & time",
        icon: "preferences-system-time",
        group: Group::System,
        keywords: &[
            "date",
            "time",
            "clock",
            "time zone",
            "timezone",
            "ntp",
            "network time",
            "automatic",
            "location",
            "12-hour",
            "24-hour",
            "am",
            "pm",
        ],
        kind: Kind::Live,
        hidden: false,
    },
    Entry {
        page: Page::Region,
        id: "region",
        english: "Region & language",
        icon: "preferences-desktop-locale",
        group: Group::System,
        keywords: &[
            "language",
            "locale",
            "formats",
            "date format",
            "number format",
            "currency",
            "measurement",
            "metric",
            "country",
            "keyboard layout",
            "login screen",
            "translation",
        ],
        kind: Kind::Live,
        hidden: false,
    },
    Entry {
        page: Page::Users,
        id: "users",
        english: "Users",
        icon: "system-users",
        group: Group::System,
        keywords: &[
            "accounts",
            "user",
            "password",
            "picture",
            "avatar",
            "full name",
            "administrator",
            "admin",
            "wheel",
            "add user",
            "remove user",
            "family",
        ],
        kind: Kind::Live,
        hidden: false,
    },
    Entry {
        page: Page::Privacy,
        id: "privacy",
        english: "Privacy",
        icon: "preferences-system-privacy",
        group: Group::System,
        keywords: &[
            "permissions",
            "location",
            "camera",
            "microphone",
            "screenshot",
            "screen sharing",
            "screencast",
            "remote desktop",
            "background",
            "autostart",
            "startup",
            "login items",
            "portal",
            "flatpak",
            "revoke",
        ],
        kind: Kind::Live,
        hidden: false,
    },
    Entry {
        page: Page::Notifications,
        id: "notifications",
        english: "Notifications",
        icon: "preferences-system-notifications",
        group: Group::Desktop,
        keywords: &[
            "do not disturb",
            "dnd",
            "quiet",
            "focus",
            "schedule",
            "banners",
            "alerts",
            "alarms",
            "calls",
            "fullscreen",
            "screen sharing",
            "lock screen",
            "mute",
            "sound",
            "applications",
        ],
        kind: Kind::Live,
        hidden: false,
    },
    Entry {
        page: Page::DefaultApps,
        id: "default-apps",
        english: "Default applications",
        icon: "preferences-desktop-default-applications",
        group: Group::Desktop,
        keywords: &[
            "browser",
            "web",
            "mail",
            "email",
            "calendar",
            "music",
            "video",
            "photos",
            "images",
            "text editor",
            "file manager",
            "terminal",
            "file types",
            "mime",
            "open with",
            "mimeapps",
        ],
        kind: Kind::Live,
        hidden: false,
    },
    Entry {
        page: Page::Launcher,
        id: "launcher",
        english: "Launcher",
        icon: "system-search",
        group: Group::Desktop,
        keywords: &[
            "search",
            "extensions",
            "web search",
            "clipboard",
            "history",
            "applications",
        ],
        kind: Kind::File {
            documents: &[Document::Launcher],
            apply: Apply::Save,
        },
        hidden: false,
    },
    Entry {
        page: Page::Shortcuts,
        id: "shortcuts",
        english: "Shortcuts",
        icon: "preferences-desktop-keyboard-shortcuts",
        group: Group::Desktop,
        keywords: &[
            "keyboard shortcuts",
            "keybindings",
            "hotkeys",
            "bindings",
            "keys",
        ],
        kind: Kind::File {
            documents: &[Document::Shortcuts, Document::Niri],
            apply: Apply::Save,
        },
        hidden: false,
    },
    Entry {
        page: Page::Widgets,
        id: "widgets",
        english: "Widgets",
        icon: "preferences-desktop",
        group: Group::Desktop,
        keywords: &["bar", "panel", "modules", "layout", "lock screen", "shell"],
        kind: Kind::File {
            documents: &[Document::Common],
            apply: Apply::Save,
        },
        hidden: false,
    },
    Entry {
        page: Page::About,
        id: "about",
        english: "About",
        icon: "help-about",
        group: Group::System,
        keywords: &[
            "system",
            "information",
            "device name",
            "hostname",
            "computer name",
            "version",
            "operating system",
            "kernel",
            "hardware",
            "model",
            "processor",
            "cpu",
            "memory",
            "ram",
            "graphics",
            "gpu",
            "disk",
            "storage",
            "firmware",
        ],
        kind: Kind::Live,
        hidden: false,
    },
    #[cfg(debug_assertions)]
    Entry {
        page: Page::LiveExample,
        id: "live-example",
        english: "Live page example",
        icon: "applications-development",
        group: Group::Developer,
        keywords: &[],
        kind: Kind::Live,
        hidden: true,
    },
];

impl Entry {
    /// The page title in the selected language.
    pub fn title(&self) -> String {
        match self.page {
            Page::Displays => fl!("settings-page-displays-title"),
            Page::Input => fl!("settings-page-input-title"),
            Page::Network => fl!("settings-page-network-title"),
            Page::Bluetooth => fl!("settings-page-bluetooth-title"),
            Page::Printers => fl!("settings-page-printers-title"),
            Page::Sound => fl!("settings-page-sound-title"),
            Page::Personal => fl!("settings-page-personal-title"),
            Page::Accessibility => fl!("settings-page-accessibility-title"),
            Page::Power => fl!("settings-page-power-title"),
            Page::DateTime => fl!("settings-page-date-time-title"),
            Page::Region => fl!("settings-page-region-title"),
            Page::Users => fl!("settings-page-users-title"),
            Page::Privacy => fl!("settings-page-privacy-title"),
            Page::Notifications => fl!("settings-page-notifications-title"),
            Page::DefaultApps => fl!("settings-page-default-apps-title"),
            Page::Launcher => fl!("settings-page-launcher-title"),
            Page::Shortcuts => fl!("settings-page-shortcuts-title"),
            Page::Widgets => fl!("settings-page-widgets-title"),
            Page::About => fl!("settings-page-about-title"),
            #[cfg(debug_assertions)]
            Page::LiveExample => fl!("settings-page-live-example-title"),
        }
    }

    /// The one-line description under the page title.
    pub fn description(&self) -> String {
        match self.page {
            Page::Displays => fl!("settings-page-displays-description"),
            Page::Input => fl!("settings-page-input-description"),
            Page::Network => fl!("settings-page-network-description"),
            Page::Bluetooth => fl!("settings-page-bluetooth-description"),
            Page::Printers => fl!("settings-page-printers-description"),
            Page::Sound => fl!("settings-page-sound-description"),
            Page::Personal => fl!("settings-page-personal-description"),
            Page::Accessibility => fl!("settings-page-accessibility-description"),
            Page::Power => fl!("settings-page-power-description"),
            Page::DateTime => fl!("settings-page-date-time-description"),
            Page::Region => fl!("settings-page-region-description"),
            Page::Users => fl!("settings-page-users-description"),
            Page::Privacy => fl!("settings-page-privacy-description"),
            Page::Notifications => fl!("settings-page-notifications-description"),
            Page::DefaultApps => fl!("settings-page-default-apps-description"),
            Page::Launcher => fl!("settings-page-launcher-description"),
            Page::Shortcuts => fl!("settings-page-shortcuts-description"),
            Page::Widgets => fl!("settings-page-widgets-description"),
            Page::About => fl!("settings-page-about-description"),
            #[cfg(debug_assertions)]
            Page::LiveExample => fl!("settings-page-live-example-description"),
        }
    }

    /// Search terms in the selected language, lowercase. In English they are
    /// [`Entry::keywords`].
    pub fn keywords_translated(&self) -> Vec<String> {
        let list = match self.page {
            Page::Displays => fl!("settings-page-displays-keywords"),
            Page::Input => fl!("settings-page-input-keywords"),
            Page::Network => fl!("settings-page-network-keywords"),
            Page::Bluetooth => fl!("settings-page-bluetooth-keywords"),
            Page::Printers => fl!("settings-page-printers-keywords"),
            Page::Sound => fl!("settings-page-sound-keywords"),
            Page::Personal => fl!("settings-page-personal-keywords"),
            Page::Accessibility => fl!("settings-page-accessibility-keywords"),
            Page::Power => fl!("settings-page-power-keywords"),
            Page::DateTime => fl!("settings-page-date-time-keywords"),
            Page::Region => fl!("settings-page-region-keywords"),
            Page::Users => fl!("settings-page-users-keywords"),
            Page::Privacy => fl!("settings-page-privacy-keywords"),
            Page::Notifications => fl!("settings-page-notifications-keywords"),
            Page::DefaultApps => fl!("settings-page-default-apps-keywords"),
            Page::Launcher => fl!("settings-page-launcher-keywords"),
            Page::Shortcuts => fl!("settings-page-shortcuts-keywords"),
            Page::Widgets => fl!("settings-page-widgets-keywords"),
            Page::About => fl!("settings-page-about-keywords"),
            #[cfg(debug_assertions)]
            Page::LiveExample => return Vec::new(),
        };
        list.split(',')
            .map(|keyword| keyword.trim().to_lowercase())
            .filter(|keyword| !keyword.is_empty())
            .collect()
    }

    /// Every search term for launchers: the translated ones first, then the
    /// English ones not already listed, at most `limit`.
    pub fn search_keywords(&self, limit: usize) -> Vec<String> {
        let mut keywords = self.keywords_translated();
        for keyword in self.keywords {
            if !keywords.iter().any(|known| known == keyword) {
                keywords.push((*keyword).to_owned());
            }
        }
        keywords.truncate(limit);
        keywords
    }
}

impl Page {
    pub fn entry(self) -> &'static Entry {
        PAGES
            .iter()
            .find(|entry| entry.page == self)
            .expect("every page has a registry entry")
    }

    pub fn id(self) -> &'static str {
        self.entry().id
    }

    /// The page title in the selected language.
    pub fn title(self) -> String {
        self.entry().title()
    }

    pub fn kind(self) -> Kind {
        self.entry().kind
    }

    /// Whether this page edits `document` (the shared-document rule).
    pub fn edits(self, document: Document) -> bool {
        matches!(self.kind(), Kind::File { documents, .. } if documents.contains(&document))
    }

    /// The page for a deep-link id. Hidden pages are included.
    pub fn from_id(id: &str) -> Option<Self> {
        PAGES
            .iter()
            .find(|entry| entry.id == id)
            .map(|entry| entry.page)
    }
}

impl std::str::FromStr for Page {
    type Err = String;

    fn from_str(id: &str) -> Result<Self, String> {
        Self::from_id(id).ok_or_else(|| {
            format!(
                "unknown Settings page `{id}`; expected one of: {}",
                ids().collect::<Vec<_>>().join(", ")
            )
        })
    }
}

/// Visible page ids, in sidebar order.
pub fn ids() -> impl Iterator<Item = &'static str> {
    navigation().into_iter().map(|entry| entry.id)
}

/// Visible pages in sidebar order: by group, then by declaration.
/// Ctrl+1–9 select the first nine.
pub fn navigation() -> Vec<&'static Entry> {
    let mut pages: Vec<_> = PAGES.iter().filter(|entry| !entry.hidden).collect();
    // A stable sort keeps declaration order within each group.
    pages.sort_by_key(|entry| entry.group);
    pages
}

/// Visible pages grouped for the sidebar; empty groups are left out.
pub fn groups() -> Vec<(Group, Vec<&'static Entry>)> {
    Group::ALL
        .iter()
        .filter_map(|&group| {
            let pages: Vec<_> = navigation()
                .into_iter()
                .filter(|entry| entry.group == group)
                .collect();
            (!pages.is_empty()).then_some((group, pages))
        })
        .collect()
}

/// The visible page `offset` places after `page` in sidebar order, wrapping.
pub fn step(page: Page, offset: isize) -> Page {
    let pages = navigation();
    let Some(index) = pages.iter().position(|entry| entry.page == page) else {
        return pages.first().map_or(page, |entry| entry.page);
    };
    let len = pages.len() as isize;
    pages[(index as isize + offset).rem_euclid(len) as usize].page
}

/// Lowercase without the accents of Latin letters, so `ecran` finds `Écrans`.
fn fold(text: &str) -> String {
    let mut folded = String::with_capacity(text.len());
    for c in text.chars().flat_map(char::to_lowercase) {
        match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => folded.push('a'),
            'ç' => folded.push('c'),
            'è' | 'é' | 'ê' | 'ë' => folded.push('e'),
            'ì' | 'í' | 'î' | 'ï' => folded.push('i'),
            'ñ' => folded.push('n'),
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' => folded.push('o'),
            'ù' | 'ú' | 'û' | 'ü' => folded.push('u'),
            'ý' | 'ÿ' => folded.push('y'),
            'œ' => folded.push_str("oe"),
            'æ' => folded.push_str("ae"),
            '’' => folded.push('\''),
            c => folded.push(c),
        }
    }
    folded
}

/// One page's searchable text, folded, in the selected language and in English.
struct Terms {
    titles: Vec<String>,
    keywords: Vec<String>,
}

impl Terms {
    fn of(entry: &Entry) -> Self {
        let mut titles = vec![fold(&entry.title()), fold(entry.english)];
        titles.dedup();
        let mut keywords: Vec<String> = entry
            .keywords_translated()
            .iter()
            .map(|keyword| fold(keyword))
            .collect();
        keywords.extend(entry.keywords.iter().map(|keyword| fold(keyword)));
        Self { titles, keywords }
    }

    /// How well one folded search term matches; 0 is no match.
    fn score(&self, term: &str) -> u32 {
        let words = |text: &str| {
            text.split(|c: char| !c.is_alphanumeric())
                .filter(|word| !word.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        };
        if self.titles.iter().any(|title| title.starts_with(term)) {
            4
        } else if self
            .titles
            .iter()
            .any(|title| words(title).iter().any(|word| word.starts_with(term)))
        {
            3
        } else if self.keywords.iter().any(|keyword| {
            keyword.starts_with(term) || words(keyword).iter().any(|w| w.starts_with(term))
        }) {
            2
        } else if self.titles.iter().any(|title| title.contains(term))
            || self.keywords.iter().any(|keyword| keyword.contains(term))
        {
            1
        } else {
            0
        }
    }
}

/// Visible pages whose title or keywords match every word of `query`, best
/// first. Titles and keywords match in the selected language and in English;
/// case and accents are ignored. An empty query matches nothing.
pub fn search(query: &str) -> Vec<&'static Entry> {
    let query = fold(query.trim());
    let terms: Vec<_> = query.split_whitespace().collect();
    if terms.is_empty() {
        return Vec::new();
    }
    let mut found: Vec<_> = navigation()
        .into_iter()
        .enumerate()
        .filter_map(|(order, entry)| {
            let page = Terms::of(entry);
            terms
                .iter()
                .map(|term| page.score(term))
                .try_fold(0, |total, score| (score > 0).then_some(total + score))
                .map(|score| (score, order, entry))
        })
        .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    found.into_iter().map(|(_, _, entry)| entry).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ids_are_unique_stable_and_round_trip() {
        let mut ids = HashSet::new();
        let mut pages = HashSet::new();
        for entry in PAGES {
            assert!(ids.insert(entry.id), "duplicate id {}", entry.id);
            assert!(pages.insert(entry.page), "duplicate page {:?}", entry.page);
            assert!(
                entry
                    .id
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte == b'-'),
                "{}",
                entry.id
            );
            assert_eq!(Page::from_id(entry.id), Some(entry.page));
            assert_eq!(entry.page.entry().id, entry.id);
            assert!(entry.keywords.iter().all(|k| *k == k.to_lowercase()));
        }
        // Callers (the Shell's launcher and notices, profile review) use these.
        for id in [
            "displays",
            "input",
            "personal",
            "power",
            "launcher",
            "shortcuts",
            "widgets",
            "network",
            "sound",
            "bluetooth",
            "notifications",
            "default-apps",
            "date-time",
            "about",
            "region",
            "users",
            "privacy",
            "printers",
            "accessibility",
        ] {
            assert!(Page::from_id(id).is_some(), "{id}");
        }
        assert!("unknown".parse::<Page>().is_err());
    }

    #[test]
    fn navigation_keeps_the_original_page_order() {
        let order: Vec<_> = navigation().iter().map(|entry| entry.page).collect();
        assert_eq!(
            order,
            [
                Page::Displays,
                Page::Input,
                Page::Network,
                Page::Bluetooth,
                Page::Printers,
                Page::Sound,
                Page::Personal,
                Page::Accessibility,
                Page::Power,
                Page::DateTime,
                Page::Region,
                Page::Users,
                Page::Privacy,
                Page::About,
                Page::Notifications,
                Page::DefaultApps,
                Page::Launcher,
                Page::Shortcuts,
                Page::Widgets,
            ]
        );
        let grouped: Vec<_> = groups()
            .into_iter()
            .flat_map(|(_, pages)| pages)
            .map(|entry| entry.page)
            .collect();
        assert_eq!(grouped, order);
        assert_eq!(step(Page::Widgets, 1), Page::Displays);
        assert_eq!(step(Page::Displays, -1), Page::Widgets);
        assert_eq!(step(Page::Input, 1), Page::Network);
    }

    #[test]
    fn search_matches_titles_and_keywords_best_first() {
        let pages = |query| search(query).iter().map(|e| e.page).collect::<Vec<_>>();
        assert!(pages("").is_empty());
        assert!(pages("   ").is_empty());
        assert_eq!(pages("DISP")[0], Page::Displays);
        assert_eq!(pages("monitor"), [Page::Displays]);
        assert_eq!(pages("touchpad"), [Page::Input]);
        assert_eq!(pages("pointer")[0], Page::Input);
        assert_eq!(pages("wallpaper"), [Page::Personal]);
        assert_eq!(pages("dark"), [Page::Personal]);
        assert_eq!(pages("suspend"), [Page::Power]);
        assert_eq!(pages("clipboard"), [Page::Launcher]);
        assert_eq!(pages("microphone"), [Page::Sound, Page::Privacy]);
        assert_eq!(pages("wireguard"), [Page::Network]);
        assert_eq!(pages("wi-fi")[0], Page::Network);
        assert_eq!(pages("lid"), [Page::Power]);
        assert_eq!(pages("pair")[0], Page::Bluetooth);
        assert_eq!(pages("do not disturb"), [Page::Notifications]);
        assert_eq!(pages("browser"), [Page::DefaultApps]);
        assert_eq!(pages("timezone"), [Page::DateTime]);
        assert_eq!(pages("hostname"), [Page::About]);
        assert_eq!(pages("language")[0], Page::Region);
        assert_eq!(pages("password")[0], Page::Users);
        assert_eq!(pages("camera"), [Page::Privacy]);
        assert_eq!(pages("autostart"), [Page::Privacy]);
        assert_eq!(pages("cups"), [Page::Printers]);
        assert_eq!(pages("terminal"), [Page::DefaultApps]);
        assert_eq!(pages("bluetooth")[0], Page::Bluetooth);
        assert_eq!(pages("hotkeys"), [Page::Shortcuts]);
        // A title match ranks above keyword matches on other pages.
        let keyboard = pages("keyboard");
        assert_eq!(keyboard[0], Page::Input);
        assert!(keyboard.contains(&Page::Shortcuts));
        // Every word must match.
        assert_eq!(pages("lock screen bar"), [Page::Widgets]);
        assert!(pages("nothing-like-this").is_empty());
        // Hidden pages are reachable by id only.
        assert!(search("live").iter().all(|entry| !entry.hidden));
    }

    #[test]
    fn english_messages_repeat_the_registry() {
        for entry in PAGES {
            assert_eq!(entry.title(), entry.english);
            assert!(!entry.description().is_empty());
            assert_eq!(entry.keywords_translated(), entry.keywords, "{}", entry.id);
        }
        assert_eq!(Group::Devices.title(), "Devices");
    }

    #[test]
    fn french_titles_show_and_english_words_still_search() {
        crate::i18n::isolated(
            concat!(
                module_path!(),
                "::french_titles_show_and_english_words_still_search"
            ),
            || {
                let _french = crate::i18n::Language::select("fr");
                assert_eq!(Page::Displays.title(), "Écrans");
                assert_eq!(Page::Region.title(), "Région et langue");
                assert_eq!(Group::System.title(), "Système");
                let pages = |query| search(query).iter().map(|e| e.page).collect::<Vec<_>>();
                // English titles and keywords keep finding pages.
                assert_eq!(pages("display")[0], Page::Displays);
                assert_eq!(pages("monitor"), [Page::Displays]);
                assert_eq!(pages("wallpaper"), [Page::Personal]);
                assert_eq!(pages("touchpad"), [Page::Input]);
                // French words, with or without accents.
                assert_eq!(pages("écran")[0], Page::Displays);
                assert_eq!(pages("ecran")[0], Page::Displays);
                assert_eq!(pages("fond d'écran"), [Page::Personal]);
                assert_eq!(pages("langue")[0], Page::Region);
                assert_eq!(pages("pavé tactile")[0], Page::Input);
                // Launchers get both, translated first, within their limit.
                let keywords = Page::Displays.entry().search_keywords(32);
                assert_eq!(keywords[0], "écran");
                assert!(keywords.iter().any(|keyword| keyword == "monitor"));
                assert!(Page::Network.entry().search_keywords(32).len() <= 32);
            },
        );
    }
}
