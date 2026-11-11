//! Test helpers for the process-wide language selection.

use std::sync::{Mutex, MutexGuard};

/// Held by every test that changes the selection or depends on it.
static SERIAL: Mutex<()> = Mutex::new(());

/// The selection in effect until dropped; restores English even on panic.
pub(crate) struct Selected {
    _serial: MutexGuard<'static, ()>,
}

impl Drop for Selected {
    fn drop(&mut self) {
        canopa_i18n::select(canopa_i18n::Selection::default());
    }
}

/// Select `tag` for messages and formats (`"fr"`), or the pseudo-locale.
pub(crate) fn select(tag: &str, pseudo: bool) -> Selected {
    let serial = SERIAL.lock().unwrap_or_else(|poison| poison.into_inner());
    let language: canopa_i18n::LanguageIdentifier = tag.parse().expect("valid tag");
    canopa_i18n::select(canopa_i18n::Selection {
        languages: canopa_i18n::negotiate(std::slice::from_ref(&language)),
        time: language.clone(),
        numbers: language,
        pseudo,
    });
    Selected { _serial: serial }
}

/// English, held against concurrent selection changes.
pub(crate) fn english() -> Selected {
    select("en", false)
}
