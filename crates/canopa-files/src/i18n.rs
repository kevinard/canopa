//! This crate's messages, in `i18n/<language>/canopa_files.ftl`.
canopa_i18n::localization!();

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::{Mutex, MutexGuard};

    /// The language selection is process-global; tests that change it hold this.
    static SERIAL: Mutex<()> = Mutex::new(());
    const CHILD: &str = "CANOPA_FILES_LANGUAGE_TEST_CHILD";

    /// Whether the test `name` (its full path in this test binary) should run
    /// its body here. Tests that select another language call this first: the
    /// parent re-runs the test alone in a child process, so the process-global
    /// selection never reaches tests that assert English text concurrently.
    pub(crate) fn isolated(name: &str) -> bool {
        if std::env::var_os(CHILD).is_some() {
            return true;
        }
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([name, "--exact", "--test-threads=1"])
            .env(CHILD, "1")
            .output()
            .unwrap();
        let report = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && report.contains("1 passed"),
            "{name} did not pass in its child process:\n{report}{}",
            String::from_utf8_lossy(&output.stderr)
        );
        false
    }

    /// A language selection until dropped, even when the test panics.
    pub(crate) struct Selected(#[allow(dead_code)] MutexGuard<'static, ()>);

    impl Selected {
        pub(crate) fn french() -> Self {
            let guard = SERIAL.lock().unwrap_or_else(|poison| poison.into_inner());
            let french: canopa_i18n::LanguageIdentifier = "fr".parse().unwrap();
            canopa_i18n::select(canopa_i18n::Selection {
                languages: canopa_i18n::negotiate(std::slice::from_ref(&french)),
                time: french.clone(),
                numbers: french,
                pseudo: false,
            });
            Self(guard)
        }

        /// The pseudo-locale (right to left) until dropped.
        #[cfg_attr(not(feature = "desktop"), allow(dead_code))]
        pub(crate) fn pseudo() -> Self {
            let guard = SERIAL.lock().unwrap_or_else(|poison| poison.into_inner());
            canopa_i18n::select(canopa_i18n::Selection {
                pseudo: true,
                ..Default::default()
            });
            Self(guard)
        }
    }

    impl Drop for Selected {
        fn drop(&mut self) {
            canopa_i18n::select(canopa_i18n::Selection::default());
        }
    }

    #[test]
    fn catalog_is_complete() {
        canopa_i18n::check_catalog(&super::LOADER).unwrap();
    }

    #[test]
    fn commands_and_sizes_follow_the_selected_language() {
        use crate::interface::Command;
        if !isolated("i18n::tests::commands_and_sizes_follow_the_selected_language") {
            return;
        }
        {
            let _french = Selected::french();
            assert_eq!(Command::Trash.label(), "Mettre à la corbeille");
            assert_eq!(
                Command::ConnectToServer.label(),
                "Se connecter à un serveur…"
            );
            assert_eq!(canopa_i18n::format::size(1536), "1,5\u{a0}Kio");
        }
        assert_eq!(Command::Trash.label(), "Move to Trash");
        assert_eq!(canopa_i18n::format::size(1536), "1.5 KiB");
    }
}
