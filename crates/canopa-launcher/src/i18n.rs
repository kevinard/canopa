//! This crate's messages, in `i18n/<language>/canopa_launcher.ftl`.
canopa_i18n::localization!();

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    /// Held by tests that change the process-wide language selection.
    static SERIAL: Mutex<()> = Mutex::new(());

    /// Restores English even if the test panics.
    struct English;

    impl Drop for English {
        fn drop(&mut self) {
            canopa_i18n::select(canopa_i18n::Selection::default());
        }
    }

    #[test]
    fn catalog_is_complete() {
        canopa_i18n::check_catalog(&super::LOADER).unwrap();
    }

    #[test]
    fn snippet_errors_follow_the_language() {
        let _serial = SERIAL.lock().unwrap_or_else(|poison| poison.into_inner());
        canopa_i18n::select(canopa_i18n::Selection {
            languages: canopa_i18n::negotiate(&["fr".parse().unwrap()]),
            time: "fr".parse().unwrap(),
            numbers: "fr".parse().unwrap(),
            pseudo: false,
        });
        let _english = English;
        let mut saved = crate::clipboard::Saved::default();
        let error = saved
            .edit(crate::clipboard::Edit::Rename {
                id: 1,
                name: String::new(),
            })
            .unwrap_err();
        assert_eq!(error, "Saisissez un nom d’extrait de 256 octets au maximum");
    }
}
