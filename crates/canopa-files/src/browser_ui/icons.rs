#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Icon {
    Back,
    Forward,
    ActiveForward,
    ActiveBack,
    Up,
    Search,
    Eye,
    More,
    Folder,
    ActiveFolder,
    ActiveColumns,
    ActiveList,
    ActiveGrid,
    Home,
    Desktop,
    Documents,
    Downloads,
    Pictures,
    Music,
    Videos,
    Templates,
    Public,
    Drive,
    Trash,
    Recovery,
    Recent,
    Eject,
    Columns,
    List,
    Grid,
    Edit,
    Places,
    Settings,
    Activity,
    Terminal,
    Info,
    Question,
    Network,
    Phone,
    Camera,
    DangerTrash,
    Warning,
    Plus,
    Minus,
    Close,
    Fit,
    Image,
    NewFolder,
    Computer,
    Down,
    Disk,
    Toolbar,
    Check,
    /// Menu commands.
    CloseOthers,
    Open,
    OpenWith,
    Copy,
    Cut,
    Paste,
    Tab,
    Window,
    Bookmark,
    SelectAll,
    Undo,
    Hidden,
    Compress,
    Extract,
    /// Emblems drawn on file icons: a link, no access, and read-only.
    Link,
    Lock,
    ReadOnly,
}

impl Icon {
    pub const ALL: [Self; 70] = [
        Self::Back,
        Self::Forward,
        Self::ActiveForward,
        Self::ActiveBack,
        Self::Up,
        Self::Search,
        Self::Eye,
        Self::More,
        Self::Folder,
        Self::ActiveFolder,
        Self::ActiveColumns,
        Self::ActiveList,
        Self::ActiveGrid,
        Self::Home,
        Self::Desktop,
        Self::Documents,
        Self::Downloads,
        Self::Pictures,
        Self::Music,
        Self::Videos,
        Self::Templates,
        Self::Public,
        Self::Drive,
        Self::Trash,
        Self::Recovery,
        Self::Recent,
        Self::Eject,
        Self::Columns,
        Self::List,
        Self::Grid,
        Self::Edit,
        Self::Places,
        Self::Settings,
        Self::Activity,
        Self::Terminal,
        Self::Info,
        Self::Question,
        Self::Network,
        Self::Phone,
        Self::Camera,
        Self::DangerTrash,
        Self::Warning,
        Self::Plus,
        Self::Minus,
        Self::Close,
        Self::Fit,
        Self::Image,
        Self::NewFolder,
        Self::Computer,
        Self::Down,
        Self::Disk,
        Self::Toolbar,
        Self::Check,
        Self::CloseOthers,
        Self::Open,
        Self::OpenWith,
        Self::Copy,
        Self::Cut,
        Self::Paste,
        Self::Tab,
        Self::Window,
        Self::Bookmark,
        Self::SelectAll,
        Self::Undo,
        Self::Hidden,
        Self::Compress,
        Self::Extract,
        Self::Link,
        Self::Lock,
        Self::ReadOnly,
    ];

    /// The icon drawn for `self` in the current writing direction: arrows and
    /// chevrons point the other way in right-to-left languages.
    pub fn directional(self) -> Self {
        if !canopa_ui::direction::is_rtl() {
            return self;
        }
        match self {
            Self::Back => Self::Forward,
            Self::Forward => Self::Back,
            Self::ActiveForward => Self::ActiveBack,
            Self::ActiveBack => Self::ActiveForward,
            other => other,
        }
    }

    pub fn svg(self) -> String {
        let path = match self {
            Self::Back | Self::ActiveBack => "m14 6-6 6 6 6",
            Self::Forward | Self::ActiveForward => "m10 6 6 6-6 6",
            Self::Up => "m6 15 6-6 6 6",
            Self::Down => "m6 9 6 6 6-6",
            Self::Check => "m5 12.5 4.5 4.5L19 7.5",
            Self::CloseOthers => "M9 4h11v11M4 9h11v11H4Zm3.5 3.5 4 4m0-4-4 4",
            Self::Open => "M14 4h6v6m0-6-9 9m7 1v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5",
            Self::OpenWith => {
                "M4 4h6.5v6.5H4Zm9.5 0H20v6.5h-6.5ZM4 13.5h6.5V20H4Zm12.75 0v6.5M13.5 16.75H20"
            }
            Self::Copy => "M9 9h11v11H9Zm-4 6H4V4h11v1",
            Self::Cut => {
                "M9 6a3 3 0 1 1-6 0 3 3 0 0 1 6 0Zm0 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0ZM20 4 8.1 15.9m6.4-1.4L20 20M8.1 8.1 12 12"
            }
            Self::Paste => "M9 3h6v4H9Zm0 2H5v16h14V5h-4",
            Self::Tab => "M3 5h18v14H3Zm0 4h18M10 5v4",
            Self::Window => "M8 4h12v11H8Zm0 3.5h12M4 9v11h12",
            Self::Bookmark => "M6 3h12v18l-6-4.5L6 21Z",
            Self::SelectAll => "M4 4h16v16H4Zm4.5 8.5 2.5 2.5 5-5.5",
            Self::Undo => "M9 14 4 9l5-5M4 9h11a5 5 0 0 1 0 10h-4",
            // A box with its lid; an arrow goes into it or comes out.
            Self::Compress => "M3 4h18v4H3Zm2 4v12h14V8m-7 2.5v6.5m-3-3 3 3 3-3",
            Self::Extract => "M3 4h18v4H3Zm2 4v12h14V8m-7 9v-6.5m-3 3 3-3 3 3",
            Self::Hidden => {
                "M2 12s3.6-6 10-6 10 6 10 6-3.6 6-10 6S2 12 2 12Zm12.5 0a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0ZM4 4l16 16"
            }
            Self::Link => {
                "M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1"
            }
            Self::Lock => "M7.5 11V8a4.5 4.5 0 0 1 9 0v3M5.5 11h13v9.5h-13Z",
            Self::ReadOnly => "M5 19h3.5L18 9.5 14.5 6 5 15.5Zm-1.5-15.5 17 17",
            Self::Disk => {
                "M4 6c0-1.7 3.6-3 8-3s8 1.3 8 3v12c0 1.7-3.6 3-8 3s-8-1.3-8-3Zm0 0c0 1.7 3.6 3 8 3s8-1.3 8-3m-16 6c0 1.7 3.6 3 8 3s8-1.3 8-3"
            }
            Self::Toolbar => "M3 5h18v14H3Zm0 5h18M6.5 7.5h.01m3 0h.01m3 0h.01",
            Self::Search => "M16.2 16.2 21 21M18 11a7 7 0 1 1-14 0 7 7 0 0 1 14 0Z",
            Self::Eye => {
                "M2 12s3.6-6 10-6 10 6 10 6-3.6 6-10 6S2 12 2 12Zm12.5 0a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0Z"
            }
            Self::More => "M5 12h.01m6.99 0h.01m6.99 0h.01",
            Self::NewFolder => {
                "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Zm9 3.5v6m-3-3h6"
            }
            Self::Folder | Self::ActiveFolder => {
                "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z"
            }
            Self::Home => "m3 10 9-7 9 7m-2-1v11H5V9m5 11v-6h4v6",
            Self::Desktop => "M3 5h18v12H3Zm9 12v4m-4 0h8",
            Self::Computer => "M4 4h16v10H4Zm-2 14h20l-2-4H4Zm8-2h4",
            Self::Documents => "M6 3h8l4 4v14H6Zm8 0v5h4m-9 4h6m-6 4h6",
            Self::Downloads => "M12 3v12m-4-4 4 4 4-4M5 16v4h14v-4",
            Self::Pictures => "M3 5h18v14H3Zm1 12 5-5 3 3 3-3 5 5M15 9h.01",
            Self::Music => {
                "M9 18V5l11-2v13M9 18a3 3 0 1 1-6 0 3 3 0 0 1 6 0Zm11-2a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z"
            }
            Self::Videos => "M3 5h18v14H3Zm7.5 4v6l5-3Z",
            Self::Templates => "M3 4h18v6H3Zm0 10h8v6H3Zm12 0h6v6h-6Z",
            Self::Public => {
                "M16 20v-1.5a4 4 0 0 0-4-4H7a4 4 0 0 0-4 4V20m10.5-12.5a4 4 0 1 1-8 0 4 4 0 0 1 8 0ZM21 20v-1.5a4 4 0 0 0-3-3.9m-2-11a4 4 0 0 1 0 7.8"
            }
            Self::Drive => "M5 5h14l2 10v4H3v-4Zm-2 10h18m-4 2h.01",
            Self::Trash | Self::DangerTrash => "M4 6h16M9 6V3h6v3m3 0-1 15H7L6 6m4 4v7m4-7v7",
            Self::Warning => "M12 3 2 20h20Zm0 6v5m0 3h.01",
            Self::Plus => "M12 5v14M5 12h14",
            Self::Minus => "M5 12h14",
            Self::Close => "M6 6l12 12M18 6 6 18",
            Self::Fit => "M4 9V4h5m6 0h5v5m0 6v5h-5m-6 0H4v-5",
            Self::Image => "M4 5h16v14H4Zm0 11 4.5-4.5 3.5 3.5 2.5-2.5L20 17M15.5 9h.01",
            Self::Recovery => "M3 10a9 9 0 1 1 1 7m-1-13v6h6m3-4v6l3 2",
            Self::Recent => "M12 3a9 9 0 1 0 0 18 9 9 0 1 0 0-18m0 4v5l3.5 2",
            Self::Eject => "m6 14 6-8 6 8Zm0 4h12",
            Self::Columns | Self::ActiveColumns => {
                "M3 5h4.5v14H3Zm6.75 0h4.5v14h-4.5ZM16.5 5H21v14h-4.5Z"
            }
            Self::List | Self::ActiveList => "M8 6h12M8 12h12M8 18h12M4 6h.01M4 12h.01M4 18h.01",
            Self::Grid | Self::ActiveGrid => "M4 4h6v6H4Zm10 0h6v6h-6ZM4 14h6v6H4Zm10 0h6v6h-6Z",
            Self::Edit => "m15 5 4 4M4 20l4-.8L20 7l-3-3L5 16Z",
            Self::Places => "M3 5h18v14H3Zm6 0v14M5.5 9h1m-1 3h1m-1 3h1",
            Self::Settings => {
                "M4 6h7m4 0h5M4 12h3m4 0h9M4 18h11m4 0h1M11 4h4v4h-4ZM7 10h4v4H7Zm8 6h4v4h-4Z"
            }
            Self::Activity => "M3 12h4l3-7 4 14 3-7h4",
            Self::Terminal => "M3 5h18v14H3Zm4 4 3 3-3 3m6 0h4",
            Self::Info => "M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0Zm-10-1v6m0-10h.01",
            Self::Question => {
                "M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0ZM9 9a3 3 0 1 1 4 3c-1 1-1 1-1 2m0 3h.01"
            }
            Self::Network => "M4 4h16v6H4Zm0 10h16v6H4ZM8 7h.01M8 17h.01",
            Self::Phone => "M7 2h10v20H7Zm4 16h2",
            Self::Camera => "M3 7h4l2-3h6l2 3h4v13H3Zm12 6a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z",
        };
        let weight = if self == Self::More { 3.5 } else { 1.9 };
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="{path}" fill="none" stroke="currentColor" stroke-width="{weight}" stroke-linecap="round" stroke-linejoin="round"/></svg>"#
        )
    }
}

#[cfg(test)]
mod tests {
    use super::Icon;

    #[test]
    fn arrows_and_chevrons_point_the_other_way_right_to_left() {
        if !crate::i18n::tests::isolated(
            "browser_ui::icons::tests::arrows_and_chevrons_point_the_other_way_right_to_left",
        ) {
            return;
        }
        assert_eq!(Icon::Back.directional(), Icon::Back);
        let _rtl = crate::i18n::tests::Selected::pseudo();
        assert_eq!(Icon::Back.directional(), Icon::Forward);
        assert_eq!(Icon::Forward.directional(), Icon::Back);
        assert_eq!(Icon::ActiveForward.directional(), Icon::ActiveBack);
        assert_eq!(Icon::Home.directional(), Icon::Home);
    }
}
