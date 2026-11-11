//! Which languages and formats a Canopa process uses.
//!
//! Sources, first match wins:
//! 1. `CANOPA_LANGUAGE` (a `:` or `,` list), for development and tests;
//! 2. `LC_ALL`;
//! 3. the person's `~/.config/locale.conf` (`LC_MESSAGES`, `LANG`, with its
//!    `LANGUAGE` list), which Settings writes, so a restarted Canopa app follows
//!    a new choice without waiting for the next login;
//! 4. the inherited `LC_MESSAGES` and `LANG`, with the inherited `LANGUAGE`.
//!
//! `LANGUAGE` is honoured only with a locale that is not `C` or `POSIX`, as
//! gettext does. Formats follow the same order with `LC_TIME` (dates and
//! times) or `LC_NUMERIC` (numbers) in place of `LC_MESSAGES`.
use std::path::PathBuf;
use unic_langid::LanguageIdentifier;

/// The languages Canopa ships messages for. English is the source and the
/// fallback for every message.
pub const SHIPPED: &[&str] = &["en", "fr"];

/// Where a locale value came from; used to pair `LANGUAGE` with its locale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Override,
    Environment,
    File,
}

/// The process environment and the person's `locale.conf`, so negotiation can
/// be tested without touching the real environment.
#[derive(Clone, Debug, Default)]
pub struct LocaleSources {
    pub environment: Vec<(String, String)>,
    /// The text of `~/.config/locale.conf`, when it exists.
    pub user_file: Option<String>,
}

impl LocaleSources {
    /// The current process environment and `$XDG_CONFIG_HOME/locale.conf`.
    pub fn current() -> Self {
        let environment: Vec<(String, String)> = std::env::vars().collect();
        let user_file = user_file_path(&environment).and_then(|path| {
            std::fs::read_to_string(path).ok().map(|mut text| {
                text.truncate(64 * 1024);
                text
            })
        });
        Self {
            environment,
            user_file,
        }
    }

    fn env(&self, name: &str) -> Option<&str> {
        self.environment
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
            .filter(|value| !value.trim().is_empty())
    }

    fn file(&self, name: &str) -> Option<String> {
        assignments(self.user_file.as_deref()?)
            .into_iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value)
            .filter(|value| !value.trim().is_empty())
    }

    /// The locale for `category` (`LC_MESSAGES`, `LC_TIME`, `LC_NUMERIC`) and
    /// its source.
    fn locale(&self, category: &str) -> Option<(String, Source)> {
        if let Some(value) = self.env("LC_ALL") {
            return Some((value.to_owned(), Source::Environment));
        }
        if let Some(value) = self.file(category).or_else(|| self.file("LANG")) {
            return Some((value, Source::File));
        }
        self.env(category)
            .or_else(|| self.env("LANG"))
            .map(|value| (value.to_owned(), Source::Environment))
    }

    /// The person's requested message languages, most preferred first.
    pub fn requested_languages(&self) -> Vec<LanguageIdentifier> {
        if let Some(list) = self.env("CANOPA_LANGUAGE") {
            return parse_list(list, Source::Override);
        }
        let Some((locale, source)) = self.locale("LC_MESSAGES") else {
            return Vec::new();
        };
        if is_posix(&locale) {
            return Vec::new();
        }
        let list = match source {
            Source::File => self.file("LANGUAGE"),
            _ => self.env("LANGUAGE").map(str::to_owned),
        };
        let mut requested = list
            .map(|list| parse_list(&list, source))
            .unwrap_or_default();
        requested.extend(language_of(&locale));
        dedup(requested)
    }

    /// The locale for formatting `category` (`LC_TIME` or `LC_NUMERIC`), such
    /// as `fr-FR`; `None` for `C`, `POSIX` or nothing set.
    pub fn formats_locale(&self, category: &str) -> Option<LanguageIdentifier> {
        if let Some(list) = self.env("CANOPA_LANGUAGE") {
            return parse_list(list, Source::Override).into_iter().next();
        }
        let (locale, _) = self.locale(category)?;
        language_of(&locale)
    }
}

fn user_file_path(environment: &[(String, String)]) -> Option<PathBuf> {
    let get = |name: &str| {
        environment
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| PathBuf::from(value))
            .filter(|path| path.is_absolute())
    };
    get("XDG_CONFIG_HOME")
        .or_else(|| get("HOME").map(|home| home.join(".config")))
        .map(|config| config.join("locale.conf"))
}

/// `locale.conf` assignments with quotes removed.
fn assignments(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| {
            let (name, value) = line.split_once('=')?;
            let value = value.trim().trim_matches('"').trim_matches('\'');
            Some((name.trim().to_owned(), value.to_owned()))
        })
        .collect()
}

fn is_posix(locale: &str) -> bool {
    let base = locale.split(['.', '@']).next().unwrap_or("");
    matches!(base, "C" | "POSIX" | "")
}

/// `fr_FR.UTF-8@euro` → `fr-FR`; `C` and unparsable names → `None`.
pub fn language_of(locale: &str) -> Option<LanguageIdentifier> {
    if is_posix(locale) {
        return None;
    }
    let base = locale.split(['.', '@']).next()?.replace('_', "-");
    base.parse::<LanguageIdentifier>()
        .ok()
        .filter(|id| id.language.as_str() != "und")
}

fn parse_list(list: &str, _source: Source) -> Vec<LanguageIdentifier> {
    dedup(list.split([':', ',']).filter_map(language_of).collect())
}

fn dedup(list: Vec<LanguageIdentifier>) -> Vec<LanguageIdentifier> {
    let mut unique: Vec<LanguageIdentifier> = Vec::new();
    for id in list {
        if !unique.contains(&id) {
            unique.push(id);
        }
    }
    unique
}

/// The shipped languages matching `requested`, most preferred first, always
/// ending with English.
pub fn negotiate(requested: &[LanguageIdentifier]) -> Vec<LanguageIdentifier> {
    let available: Vec<LanguageIdentifier> = SHIPPED
        .iter()
        .map(|tag| tag.parse().expect("shipped language tags are valid"))
        .collect();
    let english: LanguageIdentifier = "en".parse().expect("valid");
    let mut chosen: Vec<LanguageIdentifier> = fluent_langneg::negotiate_languages(
        requested,
        &available,
        Some(&english),
        fluent_langneg::NegotiationStrategy::Filtering,
    )
    .into_iter()
    .cloned()
    .collect();
    if !chosen.contains(&english) {
        chosen.push(english);
    }
    chosen
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sources(env: &[(&str, &str)], file: Option<&str>) -> LocaleSources {
        LocaleSources {
            environment: env
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
            user_file: file.map(str::to_owned),
        }
    }

    fn tags(list: &[LanguageIdentifier]) -> Vec<String> {
        list.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn posix_names_become_language_tags() {
        assert_eq!(language_of("fr_FR.UTF-8").unwrap().to_string(), "fr-FR");
        assert_eq!(language_of("de_DE@euro").unwrap().to_string(), "de-DE");
        assert_eq!(language_of("pt_BR.utf8").unwrap().to_string(), "pt-BR");
        assert_eq!(language_of("sr_RS@latin").unwrap().to_string(), "sr-RS");
        assert!(language_of("C.UTF-8").is_none());
        assert!(language_of("POSIX").is_none());
    }

    #[test]
    fn environment_order_follows_gettext() {
        let env = sources(
            &[("LANG", "en_US.UTF-8"), ("LC_MESSAGES", "fr_FR.UTF-8")],
            None,
        );
        assert_eq!(tags(&env.requested_languages()), ["fr-FR"]);
        let env = sources(&[("LANG", "fr_FR.UTF-8"), ("LC_ALL", "de_DE.UTF-8")], None);
        assert_eq!(tags(&env.requested_languages()), ["de-DE"]);
        let env = sources(&[("LANG", "de_DE.UTF-8"), ("LANGUAGE", "fr:en")], None);
        assert_eq!(tags(&env.requested_languages()), ["fr", "en", "de-DE"]);
    }

    #[test]
    fn language_list_is_ignored_with_the_c_locale() {
        let env = sources(&[("LANG", "C.UTF-8"), ("LANGUAGE", "fr")], None);
        assert!(env.requested_languages().is_empty());
        assert_eq!(tags(&negotiate(&env.requested_languages())), ["en"]);
    }

    #[test]
    fn the_user_file_wins_over_the_inherited_session_language() {
        let file = "# written by Canopa Settings\nLANG=\"fr_FR.UTF-8\"\nLC_TIME=en_GB.UTF-8\n";
        let env = sources(
            &[("LANG", "en_US.UTF-8"), ("LANGUAGE", "en_US")],
            Some(file),
        );
        assert_eq!(tags(&env.requested_languages()), ["fr-FR"]);
        assert_eq!(env.formats_locale("LC_TIME").unwrap().to_string(), "en-GB");
        assert_eq!(
            env.formats_locale("LC_NUMERIC").unwrap().to_string(),
            "fr-FR"
        );
        let env = sources(&[("LC_ALL", "de_DE.UTF-8")], Some(file));
        assert_eq!(tags(&env.requested_languages()), ["de-DE"]);
    }

    #[test]
    fn the_override_wins_over_everything() {
        let env = sources(
            &[("LC_ALL", "de_DE.UTF-8"), ("CANOPA_LANGUAGE", "fr")],
            Some("LANG=en_US.UTF-8"),
        );
        assert_eq!(tags(&env.requested_languages()), ["fr"]);
        assert_eq!(env.formats_locale("LC_TIME").unwrap().to_string(), "fr");
    }

    #[test]
    fn negotiation_falls_back_to_english() {
        let fr_ca: LanguageIdentifier = "fr-CA".parse().unwrap();
        let de: LanguageIdentifier = "de-DE".parse().unwrap();
        assert_eq!(tags(&negotiate(std::slice::from_ref(&fr_ca))), ["fr", "en"]);
        assert_eq!(tags(&negotiate(std::slice::from_ref(&de))), ["en"]);
        assert_eq!(tags(&negotiate(&[de, fr_ca])), ["fr", "en"]);
        assert_eq!(tags(&negotiate(&[])), ["en"]);
    }
}
