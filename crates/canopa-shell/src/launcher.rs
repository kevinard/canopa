//! Bounded launcher providers. Presentation and application lifetime stay with Shell.

pub(crate) mod applications;
pub(crate) mod bundled;
mod calculator;
pub(crate) mod characters;
pub(crate) mod clipboard;
pub(crate) mod controls;
pub(crate) mod files;
pub(crate) mod history;
mod math;
pub(crate) mod preferences;
mod process;
pub(crate) mod system;
pub(crate) mod windows;

use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};

pub(crate) use files::{FileIndex, build_index, search_files};
pub(crate) use process::{calculate, query_extension};

pub(crate) const RESULT_LIMIT: usize = 20;
pub(crate) const MAX_QUERY_BYTES: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Config {
    pub history: bool,
    pub roots: Vec<PathBuf>,
    pub excludes: Vec<String>,
    pub max_paths: usize,
    pub extensions: Vec<Extension>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            history: true,
            roots: vec![PathBuf::from("~")],
            excludes: ["target", "node_modules", "vendor", "build"]
                .map(String::from)
                .to_vec(),
            max_paths: 100_000,
            extensions: vec![],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Extension {
    pub id: String,
    pub name: String,
    pub keyword: String,
    pub global: bool,
    pub argv: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    #[serde(skip)]
    Clipboard(clipboard::Command),
    #[serde(skip)]
    FocusWindow {
        id: u64,
        application: Option<String>,
    },
    #[serde(skip)]
    Provider {
        name: String,
    },
    #[serde(skip)]
    Reveal {
        path: PathBuf,
    },
    #[serde(skip)]
    ChooseApplication {
        path: PathBuf,
    },
    #[serde(skip)]
    OpenWith {
        path: PathBuf,
        application: String,
    },
    #[serde(skip)]
    Preview {
        path: PathBuf,
    },
    #[serde(skip)]
    ClearHistory,
    #[serde(skip)]
    Application {
        id: String,
    },
    /// One `[Desktop Action]` of an installed application.
    #[serde(skip)]
    AppActionLaunch {
        application: String,
        action: String,
    },
    /// A session row: lock, suspend, or a confirmation menu.
    #[serde(skip)]
    Panel(crate::config::PanelAction),
    /// A Settings page id, opened through `org.canopa.Settings.OpenPage`.
    #[serde(skip)]
    SettingsPage {
        id: String,
    },
    #[serde(skip)]
    TogglePin {
        shortcut: canopa_launcher::Shortcut,
    },
    #[serde(skip)]
    EditAlias {
        shortcut: canopa_launcher::Shortcut,
    },
    #[serde(skip)]
    SaveAlias {
        shortcut: canopa_launcher::Shortcut,
    },
    OpenUrl {
        url: String,
    },
    Copy {
        text: String,
    },
    Open {
        path: PathBuf,
    },
    Launch {
        argv: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedAction {
    pub label: String,
    pub action: Action,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResultItem {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon: Option<String>,
    pub actions: Vec<NamedAction>,
}

#[derive(Debug, Clone, Default)]
pub struct Batch {
    pub results: Vec<ResultItem>,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Scope {
    All,
    Apps,
    Files,
    Calculator,
    Windows,
    Settings,
    Clipboard,
    Snippets,
    Characters(characters::Kind),
    Extension(usize),
}

/// A built-in scope keyword: English (`apps`, `clip`) always works, and the
/// session language adds its own (`shell-keyword-*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Keyword {
    Apps,
    Files,
    Windows,
    Settings,
    Clipboard,
    Snippets,
    Emoji,
    Symbol,
}

impl Keyword {
    fn scope(self) -> Scope {
        match self {
            Self::Apps => Scope::Apps,
            Self::Files => Scope::Files,
            Self::Windows => Scope::Windows,
            Self::Settings => Scope::Settings,
            Self::Clipboard => Scope::Clipboard,
            Self::Snippets => Scope::Snippets,
            Self::Emoji => Scope::Characters(characters::Kind::Emoji),
            Self::Symbol => Scope::Characters(characters::Kind::Symbol),
        }
    }
}

/// The session language's keywords, lowercased; each message lists
/// alternatives separated by spaces (`fenêtres fenetres`). English ones are
/// left out: they are matched first.
fn translated_keywords() -> Vec<(String, Keyword)> {
    use std::sync::Mutex;
    static CACHE: Mutex<Option<(u64, Vec<(String, Keyword)>)>> = Mutex::new(None);
    let generation = canopa_i18n::generation();
    let mut cache = CACHE.lock().unwrap_or_else(|poison| poison.into_inner());
    if let Some((cached, keywords)) = cache.as_ref()
        && *cached == generation
    {
        return keywords.clone();
    }
    const ENGLISH: [&str; 8] = [
        "apps", "files", "windows", "settings", "clip", "snip", "emoji", "symbol",
    ];
    let mut keywords = Vec::new();
    for (text, keyword) in [
        (fl!("shell-keyword-apps"), Keyword::Apps),
        (fl!("shell-keyword-files"), Keyword::Files),
        (fl!("shell-keyword-windows"), Keyword::Windows),
        (fl!("shell-keyword-settings"), Keyword::Settings),
        (fl!("shell-keyword-clipboard"), Keyword::Clipboard),
        (fl!("shell-keyword-snippets"), Keyword::Snippets),
        (fl!("shell-keyword-emoji"), Keyword::Emoji),
        (fl!("shell-keyword-symbol"), Keyword::Symbol),
    ] {
        for word in text.split_whitespace() {
            let word = word.to_lowercase();
            // Pseudo-locale framing and isolation marks are not keywords.
            if word.chars().all(|c| c.is_alphanumeric() || c == '-')
                && !ENGLISH.contains(&word.as_str())
            {
                keywords.push((word, keyword));
            }
        }
    }
    *cache = Some((generation, keywords.clone()));
    keywords
}

pub(crate) fn scope<'a>(query: &'a str, config: &Config) -> (Scope, &'a str) {
    let (scope, rest) = english_scope(query, config);
    if scope != Scope::All {
        return (scope, rest);
    }
    // Then the session language's keywords; an extension's keyword wins.
    let trimmed = query.trim_start();
    let (keyword, rest) = trimmed
        .split_once(char::is_whitespace)
        .unwrap_or((trimmed, ""));
    let keyword = keyword.to_lowercase();
    if config
        .extensions
        .iter()
        .any(|extension| extension.keyword == keyword)
    {
        return (Scope::All, query.trim());
    }
    match translated_keywords()
        .into_iter()
        .find(|(word, _)| *word == keyword)
    {
        // Like their English forms, scopes other than clipboard, snippets and
        // characters need a space after the keyword.
        Some((_, found))
            if trimmed.len() > keyword.len()
                || matches!(
                    found,
                    Keyword::Clipboard | Keyword::Snippets | Keyword::Emoji | Keyword::Symbol
                ) =>
        {
            (found.scope(), rest.trim())
        }
        _ => (Scope::All, query.trim()),
    }
}

fn english_scope<'a>(query: &'a str, config: &Config) -> (Scope, &'a str) {
    let trimmed = query.trim_start();
    if let Some(expression) = trimmed.strip_prefix('=') {
        return (Scope::Calculator, expression.trim());
    }
    let (keyword, rest) = trimmed
        .split_once(char::is_whitespace)
        .unwrap_or((trimmed, ""));
    match keyword {
        "clip" => return (Scope::Clipboard, rest.trim()),
        "snip" => return (Scope::Snippets, rest.trim()),
        _ => {}
    }
    if let Some((keyword, rest)) = trimmed.split_once(char::is_whitespace) {
        let scope = match keyword {
            "apps" => Scope::Apps,
            "files" => Scope::Files,
            _ => config
                .extensions
                .iter()
                .position(|extension| extension.keyword == keyword)
                .map(Scope::Extension)
                .unwrap_or_else(|| match keyword {
                    "windows" => Scope::Windows,
                    "settings" => Scope::Settings,
                    _ => Scope::All,
                }),
        };
        if scope != Scope::All {
            return (scope, rest.trim());
        }
    }
    let kind = match keyword {
        "emoji" => Some(characters::Kind::Emoji),
        "symbol" => Some(characters::Kind::Symbol),
        _ => None,
    };
    if let Some(kind) = kind
        && !config
            .extensions
            .iter()
            .any(|extension| extension.keyword == keyword)
    {
        return (Scope::Characters(kind), rest.trim());
    }
    (Scope::All, query.trim())
}

/// Shared cancellation for blocking scans; dropping a subscription stops its worker.
#[derive(Clone, Debug, Default)]
pub(crate) struct Cancellation(Arc<std::sync::atomic::AtomicBool>);

impl Cancellation {
    pub fn cancelled(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn cancel(&self) {
        self.0.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

pub(crate) struct CancelOnDrop(pub Cancellation);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}
