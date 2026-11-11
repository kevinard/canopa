//! This crate's messages, in `i18n/<language>/canopa_notify.ftl`.
canopa_i18n::localization!();

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::{Mutex, MutexGuard};

    /// Tests that change the process-wide language hold this lock.
    static SERIAL: Mutex<()> = Mutex::new(());

    /// French messages and formats until dropped, then the default again.
    pub(crate) struct French(#[allow(dead_code)] MutexGuard<'static, ()>);

    impl French {
        pub(crate) fn select() -> Self {
            let guard = SERIAL.lock().unwrap_or_else(|poison| poison.into_inner());
            canopa_i18n::select(canopa_i18n::Selection {
                languages: canopa_i18n::negotiate(&["fr".parse().unwrap()]),
                time: "fr".parse().unwrap(),
                numbers: "fr".parse().unwrap(),
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
    fn catalog_is_complete() {
        canopa_i18n::check_catalog(&super::LOADER).unwrap();
    }

    #[test]
    fn notify_labels_follow_the_language() {
        let (label, hidden) = {
            let _french = French::select();
            (
                crate::AppPermissionControl::Silence.label(),
                crate::hidden_content_summary(),
            )
        };
        assert_eq!(label, "Silencieux");
        assert_eq!(hidden, "Nouvelle notification");
        assert_eq!(crate::hidden_content_summary(), "New notification");
    }
}
