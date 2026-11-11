//! Canopa's translations (Project Fluent through `i18n-embed`) and locale-aware
//! formatting (ICU4X). Iced-free, so headless profiles can use it.
//!
//! Each crate with user-visible text keeps `i18n.toml` and
//! `i18n/<language>/<crate>.ftl`, embedded at build time, and declares:
//!
//! ```ignore
//! // src/i18n.rs
//! canopa_i18n::localization!();
//! ```
//!
//! which defines `LOADER` and a crate-local `fl!` whose message IDs and
//! arguments are checked at compile time against the English file.
//!
//! Applications call [`init`] once at start. Until then, and in tests, every
//! process uses English and `en` formats, whatever the machine's locale.
pub mod format;
mod negotiate;
pub mod pseudo;

pub use i18n_embed;
pub use i18n_embed_fl::fl as checked_fl;
pub use negotiate::{LocaleSources, SHIPPED, language_of, negotiate};
pub use rust_embed;
pub use unic_langid::LanguageIdentifier;

use fluent::FluentValue;
use i18n_embed::{I18nAssets, LanguageLoader, fluent::FluentLanguageLoader};
use std::{
    borrow::Cow,
    collections::HashMap,
    sync::{
        Mutex, OnceLock, RwLock,
        atomic::{AtomicU64, Ordering},
    },
};

/// Writing direction of the selected language.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Direction {
    #[default]
    Ltr,
    Rtl,
}

impl Direction {
    pub fn is_rtl(self) -> bool {
        self == Self::Rtl
    }
}

/// What the process shows: message languages (shipped ones, ending with
/// English), the formats locales, and whether the pseudo-locale is on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    pub languages: Vec<LanguageIdentifier>,
    /// Dates and times (`LC_TIME`).
    pub time: LanguageIdentifier,
    /// Numbers and sizes (`LC_NUMERIC`).
    pub numbers: LanguageIdentifier,
    /// Accented, lengthened, right-to-left English for layout testing.
    pub pseudo: bool,
}

impl Default for Selection {
    fn default() -> Self {
        let english: LanguageIdentifier = "en".parse().expect("valid");
        Self {
            languages: vec![english.clone()],
            time: english.clone(),
            numbers: english,
            pseudo: false,
        }
    }
}

impl Selection {
    /// Negotiate from `sources`; `CANOPA_PSEUDO_LOCALE=1` selects the pseudo-locale.
    pub fn from_sources(sources: &LocaleSources) -> Self {
        let pseudo = sources.environment.iter().any(|(key, value)| {
            key == "CANOPA_PSEUDO_LOCALE" && !matches!(value.as_str(), "" | "0")
        });
        let languages = negotiate(&sources.requested_languages());
        let fallback = languages[0].clone();
        Self {
            time: sources
                .formats_locale("LC_TIME")
                .unwrap_or_else(|| fallback.clone()),
            numbers: sources.formats_locale("LC_NUMERIC").unwrap_or(fallback),
            languages,
            pseudo,
        }
    }

    /// The message language people see (English for the pseudo-locale).
    pub fn language(&self) -> &LanguageIdentifier {
        &self.languages[0]
    }

    pub fn direction(&self) -> Direction {
        if self.pseudo || is_rtl_language(self.language()) {
            Direction::Rtl
        } else {
            Direction::Ltr
        }
    }
}

/// Languages written right to left.
pub fn is_rtl_language(language: &LanguageIdentifier) -> bool {
    matches!(
        language.language.as_str(),
        "ar" | "arc" | "ckb" | "dv" | "fa" | "he" | "ks" | "ps" | "sd" | "syr" | "ug" | "ur" | "yi"
    )
}

static SELECTION: RwLock<Option<Selection>> = RwLock::new(None);
/// Bumped on every selection change; loaders and formatter caches compare it.
static GENERATION: AtomicU64 = AtomicU64::new(1);

/// The current selection (English and `en` formats before [`init`]).
pub fn selection() -> Selection {
    SELECTION
        .read()
        .unwrap_or_else(|poison| poison.into_inner())
        .clone()
        .unwrap_or_default()
}

/// A counter that changes whenever the selection changes, for caches of
/// translated or formatted text.
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Acquire)
}

/// Negotiate from the process environment and the person's `locale.conf`.
/// Applications call this once, early in `main`.
pub fn init() -> Selection {
    let selection = Selection::from_sources(&LocaleSources::current());
    select(selection.clone());
    selection
}

/// Use `selection` from now on; every loader reloads on its next lookup, so
/// a running window shows the new language at its next redraw.
pub fn select(selection: Selection) {
    *SELECTION
        .write()
        .unwrap_or_else(|poison| poison.into_inner()) = Some(selection);
    GENERATION.fetch_add(1, Ordering::AcqRel);
}

/// Switch to the language of `locale` (such as `fr_FR.UTF-8`) and, when
/// given, the formats of `formats`, keeping the pseudo-locale setting.
pub fn select_locale(locale: &str, formats: Option<&str>) {
    let current = selection();
    let languages = negotiate(&language_of(locale).into_iter().collect::<Vec<_>>());
    let fallback = languages[0].clone();
    let formats = formats
        .and_then(language_of)
        .or_else(|| language_of(locale))
        .unwrap_or(fallback);
    select(Selection {
        languages,
        time: formats.clone(),
        numbers: formats,
        pseudo: current.pseudo,
    });
}

/// The writing direction of the current selection.
pub fn direction() -> Direction {
    selection().direction()
}

/// One crate's messages. Declared by [`localization!`]; look messages up with
/// the crate's `fl!`.
pub struct Loader {
    domain: &'static str,
    assets: &'static (dyn I18nAssets + Send + Sync),
    loader: OnceLock<FluentLanguageLoader>,
    loaded: AtomicU64,
    reload: Mutex<()>,
}

impl Loader {
    pub const fn new(
        domain: &'static str,
        assets: &'static (dyn I18nAssets + Send + Sync),
    ) -> Self {
        Self {
            domain,
            assets,
            loader: OnceLock::new(),
            loaded: AtomicU64::new(0),
            reload: Mutex::new(()),
        }
    }

    /// The crate's domain: its package name in Rust form (`canopa_ui`), which
    /// names its `.ftl` files, as `i18n-embed-fl` expects.
    pub fn domain(&self) -> String {
        self.domain.replace('-', "_")
    }

    /// Messages in the current selection.
    pub fn get(&self) -> Messages<'_> {
        let loader = self
            .loader
            .get_or_init(|| FluentLanguageLoader::new(self.domain(), "en".parse().expect("valid")));
        let generation = generation();
        if self.loaded.load(Ordering::Acquire) != generation {
            let _guard = self
                .reload
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            if self.loaded.load(Ordering::Acquire) != generation {
                self.load(loader, &selection());
                self.loaded.store(generation, Ordering::Release);
            }
        }
        Messages {
            loader,
            pseudo: selection_is_pseudo(),
        }
    }

    /// The languages this crate has a file for.
    pub fn available(&self) -> Vec<LanguageIdentifier> {
        SHIPPED
            .iter()
            .map(|tag| tag.parse::<LanguageIdentifier>().expect("valid"))
            .filter(|language| self.has_language(language))
            .collect()
    }

    fn has_language(&self, language: &LanguageIdentifier) -> bool {
        self.assets
            .get_files(&format!("{language}/{}.ftl", self.domain()))
            .into_iter()
            .any(|file| !file.is_empty())
    }

    fn load(&self, loader: &FluentLanguageLoader, selection: &Selection) {
        let mut languages: Vec<LanguageIdentifier> = if selection.pseudo {
            vec!["en".parse().expect("valid")]
        } else {
            selection
                .languages
                .iter()
                .filter(|language| self.has_language(language))
                .cloned()
                .collect()
        };
        if languages.is_empty() {
            languages.push("en".parse().expect("valid"));
        }
        if let Err(error) = loader.load_languages(self.assets, &languages) {
            eprintln!(
                "canopa-i18n: cannot load {} messages: {error}",
                self.domain()
            );
            return;
        }
        let isolate = selection.direction().is_rtl();
        let pseudo = selection.pseudo;
        loader.with_bundles_mut(|bundle| {
            bundle.set_use_isolating(isolate);
            bundle.set_formatter(Some(format::fluent_value));
            if pseudo {
                bundle.set_transform(Some(pseudo::accent));
            }
        });
    }
}

fn selection_is_pseudo() -> bool {
    SELECTION
        .read()
        .unwrap_or_else(|poison| poison.into_inner())
        .as_ref()
        .is_some_and(|selection| selection.pseudo)
}

/// A crate's loaded messages, with the methods `i18n_embed_fl::fl!` expands to.
pub struct Messages<'a> {
    loader: &'a FluentLanguageLoader,
    pseudo: bool,
}

impl Messages<'_> {
    fn finish(&self, text: String) -> String {
        if self.pseudo {
            pseudo::frame(&text)
        } else {
            text
        }
    }

    pub fn has(&self, id: &str) -> bool {
        self.loader.has(id)
    }

    pub fn get(&self, id: &str) -> String {
        self.finish(self.loader.get(id))
    }

    pub fn get_args<'a, S, V>(&self, id: &str, args: HashMap<S, V>) -> String
    where
        S: Into<Cow<'a, str>> + Clone,
        V: Into<FluentValue<'a>> + Clone,
    {
        self.finish(self.loader.get_args(id, args))
    }

    pub fn get_args_concrete<'args>(
        &self,
        id: &str,
        args: HashMap<&'args str, FluentValue<'args>>,
    ) -> String {
        self.finish(self.loader.get_args_concrete(id, args))
    }

    pub fn get_attr(&self, id: &str, attribute: &str) -> String {
        self.finish(self.loader.get_attr(id, attribute))
    }

    pub fn get_attr_args<'a, S, V>(&self, id: &str, attribute: &str, args: HashMap<S, V>) -> String
    where
        S: Into<Cow<'a, str>> + Clone,
        V: Into<FluentValue<'a>> + Clone,
    {
        self.finish(self.loader.get_attr_args(id, attribute, args))
    }

    pub fn get_attr_args_concrete<'args>(
        &self,
        id: &str,
        attribute: &str,
        args: HashMap<&'args str, FluentValue<'args>>,
    ) -> String {
        self.finish(self.loader.get_attr_args_concrete(id, attribute, args))
    }

    /// Every message ID in English, for completeness checks.
    pub fn ids(&self) -> Vec<String> {
        self.ids_in(&"en".parse().expect("valid"))
    }

    /// The message IDs in `language`'s file.
    pub fn ids_in(&self, language: &LanguageIdentifier) -> Vec<String> {
        self.loader.with_message_iter(language, |messages| {
            messages.map(|message| message.id.name.to_owned()).collect()
        })
    }
}

/// Declare this crate's embedded messages: a `Localizations` asset type, a
/// `LOADER`, and a crate-local `fl!` macro. Put it in `src/i18n.rs` and declare
/// `#[macro_use] mod i18n;` before the modules that use `fl!`.
#[macro_export]
macro_rules! localization {
    () => {
        $crate::__localization!($);
    };
}

#[doc(hidden)]
#[macro_export]
#[allow(clippy::crate_in_macro_def)] // `crate` is the declaring crate, on purpose.
macro_rules! __localization {
    ($d:tt) => {
        #[derive($crate::rust_embed::RustEmbed)]
        #[folder = "i18n/"]
        #[crate_path = "canopa_i18n::rust_embed"]
        pub struct Localizations;

        pub static LOADER: $crate::Loader =
            $crate::Loader::new(env!("CARGO_PKG_NAME"), &Localizations);

        /// A translated message from this crate's `i18n/<language>/<crate>.ftl`,
        /// checked at compile time against the English file.
        #[allow(unused_macros)]
        macro_rules! fl { ($d($d args:tt)*) => { canopa_i18n::checked_fl!(crate::i18n::LOADER.get(), $d($d args)*) }; }
    };
}

/// Check, in a crate's tests, that every shipped language file parses, has no
/// message missing from English's IDs, and (for French) has every English ID.
pub fn check_catalog(loader: &Loader) -> Result<(), String> {
    let mut problems = Vec::new();
    for language in SHIPPED {
        let path = format!("{language}/{}.ftl", loader.domain());
        let files = loader.assets.get_files(&path);
        let Some(file) = files.first() else {
            problems.push(format!("{path} is missing"));
            continue;
        };
        let text = String::from_utf8(file.to_vec()).map_err(|error| error.to_string())?;
        if let Err((_, errors)) = fluent::FluentResource::try_new(text) {
            problems.push(format!("{path} does not parse: {errors:?}"));
        }
    }
    let english = message_ids(loader, "en");
    let english_blocks = message_blocks(&file_text(loader, "en"));
    for language in SHIPPED.iter().filter(|language| **language != "en") {
        let translated = message_ids(loader, language);
        for id in &english {
            if !translated.contains(id) {
                problems.push(format!("{language}: {id} is not translated"));
            }
        }
        for id in &translated {
            if !english.contains(id) {
                problems.push(format!("{language}: {id} is not an English message"));
            }
        }
        let text = file_text(loader, language);
        if !text.contains("machine-assisted, needs review") && !text.contains("reviewed") {
            problems.push(format!(
                "{language}: the header must say whether the translation is reviewed"
            ));
        }
        for (id, block) in message_blocks(&text) {
            let Some((_, source)) = english_blocks.iter().find(|(english, _)| *english == id)
            else {
                continue;
            };
            let (mut wanted, mut found) = (variables(source), variables(&block));
            wanted.sort();
            wanted.dedup();
            found.sort();
            found.dedup();
            if wanted != found {
                problems.push(format!(
                    "{language}: {id} uses {found:?}, English uses {wanted:?}"
                ));
            }
            if *language == "fr" {
                for problem in french_typography(&block) {
                    problems.push(format!("fr: {id}: {problem}"));
                }
            }
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

fn file_text(loader: &Loader, language: &str) -> String {
    let path = format!("{language}/{}.ftl", loader.domain());
    loader
        .assets
        .get_files(&path)
        .into_iter()
        .next()
        .map(|file| String::from_utf8_lossy(&file).into_owned())
        .unwrap_or_default()
}

/// Each message or term ID with its source lines (value, variants and
/// attributes), comments excluded.
fn message_blocks(text: &str) -> Vec<(String, String)> {
    let mut blocks: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let starts_entry = line
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '-');
        match line.split_once('=') {
            Some((id, value)) if starts_entry => {
                blocks.push((id.trim().to_owned(), value.to_owned()));
            }
            _ => {
                if let Some((_, block)) = blocks.last_mut() {
                    block.push('\n');
                    block.push_str(line);
                }
            }
        }
    }
    blocks
}

/// The `$variables` and `-terms` a message refers to.
fn variables(block: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut chars = block.char_indices().peekable();
    while let Some((start, c)) = chars.next() {
        if c != '$' {
            continue;
        }
        let mut end = start + 1;
        while let Some((index, next)) = chars.peek().copied() {
            if next.is_ascii_alphanumeric() || next == '_' || next == '-' {
                end = index + next.len_utf8();
                chars.next();
            } else {
                break;
            }
        }
        if end > start + 1 {
            found.push(block[start..end].to_owned());
        }
    }
    found
}

/// French puts a no-break space before `:`, `;`, `!`, `?` and `»` and after
/// `«`, and uses `’` rather than `'`.
fn french_typography(block: &str) -> Vec<String> {
    let mut problems = Vec::new();
    for line in block.lines() {
        let text = line.trim_start();
        // Variant keys and selectors are syntax.
        let text = match text.strip_prefix('*').unwrap_or(text).strip_prefix('[') {
            Some(rest) => rest.split_once(']').map_or("", |(_, rest)| rest),
            None => text,
        };
        if text.trim_start().starts_with('.') {
            // An attribute: check its value only.
        }
        for mark in [':', ';', '!', '?', '»'] {
            if text.contains(&format!(" {mark}")) {
                problems.push(format!(
                    "a plain space before {mark:?}; use a no-break space"
                ));
            }
        }
        if text.contains("« ") {
            problems.push("a plain space after «; use a no-break space".to_owned());
        }
        if text.contains('\'') {
            problems.push("a straight apostrophe; use ’".to_owned());
        }
        if text.contains('"') && !text.contains("{\"") {
            problems.push("straight quotes; use « »".to_owned());
        }
    }
    problems
}

fn message_ids(loader: &Loader, language: &str) -> Vec<String> {
    let path = format!("{language}/{}.ftl", loader.domain());
    let Some(file) = loader.assets.get_files(&path).into_iter().next() else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&file).into_owned();
    let resource = match fluent::FluentResource::try_new(text) {
        Ok(resource) | Err((resource, _)) => resource,
    };
    resource
        .entries()
        .filter_map(|entry| match entry {
            fluent_syntax::ast::Entry::Message(message) => Some(message.id.name.to_owned()),
            fluent_syntax::ast::Entry::Term(term) => Some(format!("-{}", term.id.name)),
            _ => None,
        })
        .collect()
}

extern crate self as canopa_i18n;

#[macro_use]
mod i18n {
    crate::localization!();
}

#[cfg(test)]
mod tests;
