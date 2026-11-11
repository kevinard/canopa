//! Every Settings page is declared once, here.
//!
//! The sidebar, its groups and Ctrl+1–9 order, search, command-line and D-Bus
//! page ids, page titles and descriptions all come from [`PAGES`]. A page's
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
    Power,
    DateTime,
    Notifications,
    DefaultApps,
    Launcher,
    Shortcuts,
    Widgets,
    About,
    Region,
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

    pub fn title(self) -> &'static str {
        match self {
            Self::Devices => "Devices",
            Self::Input => "Input",
            Self::Appearance => "Appearance",
            Self::System => "System",
            Self::Desktop => "Desktop",
            #[cfg(debug_assertions)]
            Self::Developer => "Developer",
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
    pub title: &'static str,
    pub description: &'static str,
    /// A freedesktop icon name for launchers and, later, the sidebar.
    pub icon: &'static str,
    pub group: Group,
    /// Extra lowercase search terms; titles are always searched.
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
        title: "Displays",
        description: "Manage connected monitors and save display layouts.",
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
        title: "Keyboard & pointer",
        description: "Adjust keyboard, mouse, and touchpad behavior.",
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
        title: "Network",
        description: "Connect to Wi-Fi, wired networks and VPNs, and share a hotspot.",
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
        title: "Bluetooth",
        description: "Pair and connect headphones, keyboards, mice and other wireless devices.",
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
        page: Page::Sound,
        id: "sound",
        title: "Sound",
        description: "Choose speakers and microphones, and set application volumes.",
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
        title: "Personalization",
        description: "Choose how your desktop and lock screen look and feel.",
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
        page: Page::Power,
        id: "power",
        title: "Power",
        description: "Battery, power mode, brightness, the lid and idle timeouts.",
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
        title: "Date & time",
        description: "Set the time zone, network time and the 12- or 24-hour clock.",
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
        title: "Region & language",
        description: "Choose your language and formats, and the system's language and keyboard.",
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
        page: Page::Notifications,
        id: "notifications",
        title: "Notifications",
        description: "Do not disturb, its schedule, and each application's notifications.",
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
        title: "Default applications",
        description: "Choose which applications open web links, mail, music, files and more.",
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
        title: "Launcher",
        description: "Choose search providers and manage clipboard history.",
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
        title: "Shortcuts",
        description: "Find and edit keyboard shortcuts for your desktop.",
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
        title: "Widgets",
        description: "Arrange widgets on your desktop and lock screen.",
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
        title: "About",
        description: "This computer's name, hardware and software.",
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
        title: "Live page example",
        description: "The live service page pattern on a simulated service.",
        icon: "applications-development",
        group: Group::Developer,
        keywords: &[],
        kind: Kind::Live,
        hidden: true,
    },
];

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

    pub fn title(self) -> &'static str {
        self.entry().title
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

/// How well one search term matches one entry; 0 is no match.
fn term_score(entry: &Entry, term: &str) -> u32 {
    let title = entry.title.to_lowercase();
    let words = |text: &str| {
        text.split(|c: char| !c.is_alphanumeric())
            .filter(|word| !word.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    if title.starts_with(term) {
        4
    } else if words(&title).iter().any(|word| word.starts_with(term)) {
        3
    } else if entry.keywords.iter().any(|keyword| {
        keyword.starts_with(term) || words(keyword).iter().any(|w| w.starts_with(term))
    }) {
        2
    } else if title.contains(term) || entry.keywords.iter().any(|keyword| keyword.contains(term)) {
        1
    } else {
        0
    }
}

/// Visible pages whose title or keywords match every word of `query`, best
/// first. Case is ignored. An empty query matches nothing.
pub fn search(query: &str) -> Vec<&'static Entry> {
    let query = query.trim().to_lowercase();
    let terms: Vec<_> = query.split_whitespace().collect();
    if terms.is_empty() {
        return Vec::new();
    }
    let mut found: Vec<_> = navigation()
        .into_iter()
        .enumerate()
        .filter_map(|(order, entry)| {
            terms
                .iter()
                .map(|term| term_score(entry, term))
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
                Page::Sound,
                Page::Personal,
                Page::Power,
                Page::DateTime,
                Page::Region,
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
        assert_eq!(pages("microphone"), [Page::Sound]);
        assert_eq!(pages("wireguard"), [Page::Network]);
        assert_eq!(pages("wi-fi")[0], Page::Network);
        assert_eq!(pages("lid"), [Page::Power]);
        assert_eq!(pages("pair")[0], Page::Bluetooth);
        assert_eq!(pages("do not disturb"), [Page::Notifications]);
        assert_eq!(pages("browser"), [Page::DefaultApps]);
        assert_eq!(pages("timezone"), [Page::DateTime]);
        assert_eq!(pages("hostname"), [Page::About]);
        assert_eq!(pages("language")[0], Page::Region);
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
}
