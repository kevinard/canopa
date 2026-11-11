//! This crate's messages, in `i18n/<language>/canopa_session_ui.ftl`.
canopa_i18n::localization!();

/// Serializes tests that change the process-wide language selection and
/// restores English when dropped, even after a failed assertion.
#[cfg(test)]
pub(crate) struct Selected(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

#[cfg(test)]
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Holds English for a test that asserts English text.
#[cfg(test)]
pub(crate) fn english() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|poison| poison.into_inner())
}

#[cfg(test)]
impl Selected {
    pub(crate) fn new(language: &str, pseudo: bool) -> Self {
        let guard = english();
        let tag: canopa_i18n::LanguageIdentifier = language.parse().unwrap();
        canopa_i18n::select(canopa_i18n::Selection {
            languages: canopa_i18n::negotiate(std::slice::from_ref(&tag)),
            time: tag.clone(),
            numbers: tag,
            pseudo,
        });
        Self(guard)
    }
}

#[cfg(test)]
impl Drop for Selected {
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

    #[test]
    fn shared_labels_and_default_clock_follow_french() {
        let _french = super::Selected::new("fr", false);
        assert_eq!(crate::view::account_label("Alice"), "Compte\u{a0}: Alice");
        let snapshot = canopa_notify_client::LockScreenSnapshot {
            entries: (0..4)
                .map(|_| canopa_notify_client::LockScreenEntry {
                    app: "A".into(),
                    text: "B".into(),
                    critical: true,
                })
                .collect(),
            more: 2,
        };
        let lines = crate::widgets::notification_lines(&snapshot).unwrap();
        assert!(lines.ends_with("+3 autres"), "{lines}");
        let settings = crate::settings::Settings::load(
            &canopa_config::Request::Defaults,
            canopa_config::Component::Lockscreen,
        )
        .unwrap()
        .0;
        let now = chrono::NaiveDate::from_ymd_opt(2026, 11, 11)
            .unwrap()
            .and_hms_opt(14, 30, 0)
            .unwrap()
            .and_utc();
        assert_eq!(settings.clock_text(&now), "14:30");
        assert_eq!(settings.date_text(&now), "mercredi 11 novembre");
        let mut explicit = settings.clone();
        explicit.date_format = "%Y-%m-%d".into();
        assert_eq!(explicit.date_text(&now), "2026-11-11");
    }
}
