//! This crate's messages, in `i18n/<language>/canopa_portal.ftl`.
canopa_i18n::localization!();

#[cfg(test)]
mod tests {
    use crate::file_chooser::{ChooserRequest, Mode};
    use canopa_i18n::{Selection, negotiate, select};
    use std::sync::{Mutex, MutexGuard};

    /// The selection is process-global: tests that change it hold this lock
    /// and restore English when the guard drops, even on panic.
    static SERIAL: Mutex<()> = Mutex::new(());

    struct Language(#[allow(dead_code)] MutexGuard<'static, ()>);

    impl Language {
        fn select(selection: Selection) -> Self {
            let guard = SERIAL.lock().unwrap_or_else(|poison| poison.into_inner());
            select(selection);
            Self(guard)
        }

        fn french() -> Self {
            Self::select(Selection {
                languages: negotiate(&["fr".parse().unwrap()]),
                time: "fr".parse().unwrap(),
                numbers: "fr".parse().unwrap(),
                pseudo: false,
            })
        }

        fn pseudo() -> Self {
            Self::select(Selection {
                pseudo: true,
                ..Selection::default()
            })
        }
    }

    impl Drop for Language {
        fn drop(&mut self) {
            select(Selection::default());
        }
    }

    #[test]
    fn catalog_is_complete() {
        canopa_i18n::check_catalog(&super::LOADER).unwrap();
    }

    #[test]
    fn default_labels_and_counts_follow_the_language() {
        let _french = Language::french();
        let request = ChooserRequest::parse(
            Mode::Save,
            "org.example.App".into(),
            String::new(),
            "Export".into(),
            Default::default(),
        )
        .unwrap();
        assert_eq!(request.accept_label, "Enregistrer");
        assert_eq!(
            request.title, "Export",
            "the app's title is shown as passed"
        );
        let access = crate::access::AccessRequest::parse(
            "org.example.App".into(),
            String::new(),
            "Camera".into(),
            String::new(),
            String::new(),
            Default::default(),
        )
        .unwrap();
        assert_eq!(
            (access.deny_label.as_str(), access.grant_label.as_str()),
            ("Refuser", "Autoriser")
        );
        assert_eq!(fl!("portal-chooser-item-count", count = 1), "1 élément");
        assert_eq!(fl!("portal-chooser-item-count", count = 3), "3 éléments");
        assert_eq!(
            fl!("portal-window-title", title = "Export"),
            "Export — Canopa"
        );
        assert_eq!(
            fl!("portal-screencast-action", action = "share-clipboard"),
            "Partager le presse-papiers"
        );
        let limit = fl!(
            "portal-recovery-size-limit",
            size = canopa_i18n::format::size(128 << 20)
        );
        assert!(limit.starts_with("Les copies jusqu’à 128") && limit.contains("Mio"));
    }

    #[cfg(feature = "desktop")]
    #[test]
    fn folder_rows_point_toward_the_reading_end() {
        assert_eq!(crate::chooser_ui::entry_marker(true), "▸");
        let _pseudo = Language::pseudo();
        assert_eq!(crate::chooser_ui::entry_marker(true), "◂");
        assert_eq!(crate::chooser_ui::entry_marker(false), " ");
    }
}
