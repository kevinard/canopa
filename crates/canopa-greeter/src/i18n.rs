//! This crate's messages, in `i18n/<language>/canopa_greeter.ftl`.
canopa_i18n::localization!();

/// Selects French for one test, serialized, and restores English when
/// dropped, even after a failed assertion.
#[cfg(test)]
pub(crate) struct French(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

#[cfg(test)]
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Holds English for a test that asserts English text.
#[cfg(test)]
pub(crate) fn english() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|poison| poison.into_inner())
}

#[cfg(test)]
impl French {
    pub(crate) fn select() -> Self {
        let guard = english();
        let fr: canopa_i18n::LanguageIdentifier = "fr".parse().unwrap();
        canopa_i18n::select(canopa_i18n::Selection {
            languages: canopa_i18n::negotiate(std::slice::from_ref(&fr)),
            time: fr.clone(),
            numbers: fr,
            pseudo: false,
        });
        Self(guard)
    }
}

#[cfg(test)]
impl Drop for French {
    fn drop(&mut self) {
        canopa_i18n::select(canopa_i18n::Selection::default());
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn catalog_is_complete() {
        canopa_i18n::check_catalog(&super::LOADER).unwrap();
    }
}
