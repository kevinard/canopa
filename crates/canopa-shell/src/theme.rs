//! Process appearance is replaced only when a complete configuration is installed.
use std::sync::{LazyLock, RwLock};

static INSTALLED: LazyLock<RwLock<iced::Theme>> =
    LazyLock::new(|| RwLock::new(canopa_ui::theme(Default::default())));

pub fn install(appearance: canopa_theme::Appearance) {
    *INSTALLED.write().expect("appearance lock") = canopa_ui::theme(appearance);
}

pub fn current() -> iced::Theme {
    INSTALLED.read().expect("appearance lock").clone()
}

pub fn palette() -> canopa_ui::Colors {
    canopa_ui::colors(&current())
}

/// Compact bar roles in logical pixels; Iced applies output scaling.
pub mod bar_metrics {
    pub const TEXT_SIZE: u32 = canopa_ui::metrics::LABEL as u32;
    pub const WINDOW_TITLE_SIZE: u32 = canopa_ui::metrics::TITLE as u32;
    pub const ICON_SIZE: u32 = 16;
}
