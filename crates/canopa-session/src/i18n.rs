//! This crate's messages, in `i18n/<language>/canopa_session.ftl`.
canopa_i18n::localization!();

#[cfg(test)]
mod tests {
    #[test]
    fn catalog_is_complete() {
        canopa_i18n::check_catalog(&super::LOADER).unwrap();
    }

    /// Changes the process-wide selection; restores English even on panic.
    struct French(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);
    impl French {
        fn select() -> Self {
            static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
            let guard = SERIAL.lock().unwrap_or_else(|poison| poison.into_inner());
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
    impl Drop for French {
        fn drop(&mut self) {
            canopa_i18n::select(canopa_i18n::Selection::default());
        }
    }

    #[test]
    fn lock_screen_actions_are_translated() {
        let _french = French::select();
        let settings = canopa_session_ui::settings::Settings::load(
            &canopa_config::Request::Defaults,
            canopa_config::Component::Lockscreen,
        )
        .unwrap()
        .0;
        let mut ui = crate::lock_ui::LockUi::new(&settings, "alice".into()).unwrap();
        crate::lock_ui::configure(&mut ui);
        crate::lock_ui::update_actions(&mut ui, true);
        assert_eq!(ui.controls[0].label, "Réessayer");
        let panel = ui.authentication_panel.as_ref().unwrap();
        assert_eq!(panel.heading, "Déverrouiller la session");
        assert_eq!(fl!("lock-title"), "Verrouillage Canopa");
    }
}
