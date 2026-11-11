//! This crate's messages, in `i18n/<language>/canopa_services.ftl`.
canopa_i18n::localization!();

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    /// Tests that change the process-wide language hold this lock.
    static SERIAL: Mutex<()> = Mutex::new(());

    /// Run `f` in French, restoring the default even if it panics.
    fn in_french<R>(f: impl FnOnce() -> R) -> R {
        struct Restore;
        impl Drop for Restore {
            fn drop(&mut self) {
                canopa_i18n::select(canopa_i18n::Selection::default());
            }
        }
        let _guard = SERIAL.lock().unwrap_or_else(|poison| poison.into_inner());
        canopa_i18n::select(canopa_i18n::Selection {
            languages: canopa_i18n::negotiate(&["fr".parse().unwrap()]),
            time: "fr".parse().unwrap(),
            numbers: "fr".parse().unwrap(),
            pseudo: false,
        });
        let _restore = Restore;
        f()
    }

    #[test]
    fn catalog_is_complete() {
        canopa_i18n::check_catalog(&super::LOADER).unwrap();
    }

    #[test]
    fn errors_people_see_follow_the_language() {
        use crate::{Error, logind::Failure, mpris::MprisError};
        let french = in_french(|| {
            [
                Error::TimedOut.to_string(),
                MprisError::Unsupported.to_string(),
                MprisError::Invalid("Invalid playback state").to_string(),
                Failure::Unavailable.to_string(),
            ]
        });
        assert_eq!(french[0], "le service n’a pas répondu à temps");
        assert_eq!(
            french[1],
            "Ce lecteur ne prend pas en charge cette commande"
        );
        assert_eq!(
            french[2],
            "Le lecteur a envoyé des informations invalides (Invalid playback state)"
        );
        assert_eq!(
            french[3],
            "le gestionnaire de connexion n’est pas disponible"
        );
        // Programmatic matching is unchanged; English comes back afterwards.
        assert!(matches!(
            Error::Invalid("user name"),
            Error::Invalid("user name")
        ));
        assert_eq!(
            MprisError::Changed.to_string(),
            "Player or track changed; try again"
        );
    }

    #[cfg(feature = "audio")]
    #[test]
    fn audio_errors_follow_the_language() {
        let french = in_french(|| crate::audio::AudioError::Unavailable.to_string());
        assert_eq!(french, "L’audio est indisponible.");
    }
}
