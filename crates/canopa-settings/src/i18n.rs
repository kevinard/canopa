//! This crate's messages, in `i18n/<language>/canopa_settings.ftl`.
canopa_i18n::localization!();

/// Tests that change the process-wide language hold this guard: it
/// serializes them, and restores English and `en` formats when dropped, even
/// after a failed assertion. Tests asserting English text run without it.
#[cfg(test)]
pub(crate) struct Language(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

#[cfg(test)]
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
impl Language {
    /// Select `tag` (`fr`) for messages and formats.
    pub(crate) fn select(tag: &str) -> Self {
        let guard = SERIAL.lock().unwrap_or_else(|poison| poison.into_inner());
        let language: canopa_i18n::LanguageIdentifier = tag.parse().expect("a language tag");
        canopa_i18n::select(canopa_i18n::Selection {
            languages: canopa_i18n::negotiate(std::slice::from_ref(&language)),
            time: language.clone(),
            numbers: language,
            pseudo: false,
        });
        Self(guard)
    }

    /// Hold the lock without changing the language (English), for tests
    /// that switch it themselves.
    pub(crate) fn english() -> Self {
        let guard = SERIAL.lock().unwrap_or_else(|poison| poison.into_inner());
        canopa_i18n::select(canopa_i18n::Selection::default());
        Self(guard)
    }
}

/// Run `body` in a child process that runs only the test `path` (from
/// `concat!(module_path!(), "::name")`), so that switching the process-wide
/// language never reaches the English tests running beside it.
#[cfg(test)]
pub(crate) fn isolated(path: &str, body: impl FnOnce()) {
    const CHILD: &str = "CANOPA_SETTINGS_ISOLATED_TEST";
    let name = path.split_once("::").map_or(path, |(_, name)| name);
    if std::env::var(CHILD).is_ok_and(|running| running == name) {
        body();
        return;
    }
    let output = std::process::Command::new(std::env::current_exe().expect("the test binary"))
        .args([name, "--exact", "--test-threads=1", "--nocapture"])
        .env(CHILD, name)
        .output()
        .expect("the test binary runs");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && stdout.contains("1 passed"),
        "{name} failed in its own process:\n{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(test)]
impl Drop for Language {
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
