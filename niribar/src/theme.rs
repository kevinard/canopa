//! The shell installs immutable appearance before constructing modules or surfaces.
//! Shell reload is unsupported; validators never install process state.
use std::sync::{LazyLock, OnceLock};

static INSTALLED: OnceLock<iced::Theme> = OnceLock::new();
static DEFAULT: LazyLock<iced::Theme> = LazyLock::new(|| canopa_ui::theme(Default::default()));

pub fn install(appearance: canopa_theme::Appearance) {
    INSTALLED
        .set(canopa_ui::theme(appearance))
        .expect("shell appearance installed once at startup");
}

pub fn current() -> &'static iced::Theme {
    INSTALLED.get().unwrap_or(&DEFAULT)
}
pub fn palette() -> canopa_ui::Colors {
    canopa_ui::colors(current())
}
