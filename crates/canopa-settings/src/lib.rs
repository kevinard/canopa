//! Settings is a frontend; compositor, wallpaper and idle policy keep their owners.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Page {
    Displays,
    Input,
    Personal,
    Power,
    Launcher,
    Shortcuts,
}
#[cfg(feature = "desktop")]
impl Page {
    pub(crate) const ALL: [Self; 6] = [
        Self::Displays,
        Self::Input,
        Self::Personal,
        Self::Power,
        Self::Launcher,
        Self::Shortcuts,
    ];
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Displays => "Displays",
            Self::Input => "Keyboard & pointer",
            Self::Personal => "Personalization",
            Self::Power => "Power & idle",
            Self::Launcher => "Launcher",
            Self::Shortcuts => "Shortcuts",
        }
    }
}
pub mod display;
pub mod input;
pub mod launcher;
pub mod niri;
pub mod preferences;
pub mod profile_watch;
pub mod profiles;
pub mod session;
pub mod shortcuts;
pub mod storage;
#[cfg(feature = "desktop")]
pub mod ui;
