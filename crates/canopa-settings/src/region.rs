//! The Region & language page's model: the system locale and keyboard from
//! systemd-localed, and the person's language and formats.
//!
//! The person's choice is written to `~/.config/locale.conf` (`LANG` and the
//! format variables), which the login shell profile reads when greetd starts
//! the session, so it applies at the next login. AccountsService records the
//! same language (and the formats, where it supports them) for display
//! managers and other desktops. Only installed locales are offered: localed
//! refuses others, and generating one needs an administrator (`locale-gen`).
use crate::live::{LiveAction, LiveService, ServiceError};
use canopa_services::{
    ServiceState, accounts,
    locale::{self, SystemLocale},
};
use futures_util::{FutureExt, StreamExt, future::BoxFuture, stream::BoxStream};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

/// The variables "formats" covers.
pub const FORMAT_VARIABLES: [&str; 5] = [
    "LC_TIME",
    "LC_NUMERIC",
    "LC_MONETARY",
    "LC_MEASUREMENT",
    "LC_PAPER",
];
const USER_FILE: &str = "locale.conf";
/// A fixed moment for previews: 11 November 2026, 14:30 UTC.
const SAMPLE_TIME: i64 = 1_794_407_400;

/// An installed locale and its name for people.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Installed {
    /// As localed and `locale.conf` take it, such as `fr_FR.UTF-8`.
    pub name: String,
    /// "French (France)".
    pub label: String,
}

/// The person's choices in `~/.config/locale.conf`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UserLocale {
    pub language: Option<String>,
    pub formats: Option<String>,
}

/// How a formats locale writes dates, numbers and money.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Preview {
    pub date: String,
    pub time: String,
    pub number: String,
    pub currency: String,
    pub metric: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionState {
    /// `None` while localed is not available.
    pub system: Option<SystemLocale>,
    pub user: UserLocale,
    /// What the session uses now (`LANG` of this process).
    pub session: String,
    pub installed: Arc<Vec<Installed>>,
    /// The formats of the effective formats locale.
    pub preview: Option<Preview>,
}

impl RegionState {
    pub fn language(&self) -> String {
        self.user
            .language
            .clone()
            .or_else(|| self.system.as_ref()?.get("LANG").map(str::to_owned))
            .unwrap_or_else(|| self.session.clone())
    }

    pub fn formats(&self) -> String {
        self.user
            .formats
            .clone()
            .or_else(|| self.system.as_ref()?.get("LC_TIME").map(str::to_owned))
            .unwrap_or_else(|| self.language())
    }

    pub fn label(&self, name: &str) -> String {
        self.installed
            .iter()
            .find(|locale| same_locale(&locale.name, name))
            .map_or_else(|| name.to_owned(), |locale| locale.label.clone())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegionAction {
    UserLanguage(String),
    UserFormats(String),
    /// The login screen's and new users' language; formats follow it.
    SystemLanguage(String),
    /// XKB layouts, variants and options for the login screen and console.
    SystemKeyboard {
        layouts: String,
        variants: String,
        options: String,
    },
}

impl LiveAction for RegionAction {
    fn control(&self) -> String {
        match self {
            Self::UserLanguage(_) => "user-language",
            Self::UserFormats(_) => "user-formats",
            Self::SystemLanguage(_) => "system-language",
            Self::SystemKeyboard { .. } => "system-keyboard",
        }
        .into()
    }
}

/// `en_US.utf8` and `en_US.UTF-8` are the same locale.
pub fn same_locale(a: &str, b: &str) -> bool {
    normalize(a) == normalize(b)
}

/// `locale -a` spells the codeset `utf8`; localed and files use `UTF-8`.
pub fn normalize(name: &str) -> String {
    match name.split_once('.') {
        Some((base, codeset)) => {
            let (codeset, modifier) = match codeset.split_once('@') {
                Some((codeset, modifier)) => (codeset, Some(modifier)),
                None => (codeset, None),
            };
            let codeset =
                if codeset.eq_ignore_ascii_case("utf8") || codeset.eq_ignore_ascii_case("utf-8") {
                    "UTF-8".to_owned()
                } else {
                    codeset.to_owned()
                };
            match modifier {
                Some(modifier) => format!("{base}.{codeset}@{modifier}"),
                None => format!("{base}.{codeset}"),
            }
        }
        None => name.to_owned(),
    }
}

/// `locale.conf` assignments, unquoted.
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

pub fn read_user(text: &str) -> UserLocale {
    let values = assignments(text);
    let get = |name: &str| {
        values
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
            .filter(|value| locale::valid_locale(value))
    };
    UserLocale {
        language: get("LANG"),
        formats: get("LC_TIME"),
    }
}

/// `text` with these variables set; comments and other lines are kept.
pub fn with_user_values(text: &str, values: &[(&str, &str)]) -> String {
    let mut lines: Vec<String> = text
        .lines()
        .filter(|line| {
            let name = line.split('=').next().unwrap_or("").trim();
            !values.iter().any(|(variable, _)| *variable == name)
        })
        .map(str::to_owned)
        .collect();
    for (name, value) in values {
        lines.push(format!("{name}={value}"));
    }
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

/// Group `digits` by `grouping` (glibc's `3;3`, or `3;2` for lakh) with `separator`.
fn group(digits: &str, grouping: &[usize], separator: &str) -> String {
    if separator.is_empty() || grouping.is_empty() || grouping[0] == 0 {
        return digits.to_owned();
    }
    let mut groups = Vec::new();
    let mut rest = digits;
    let mut index = 0;
    while !rest.is_empty() {
        let size = grouping[index.min(grouping.len() - 1)];
        if size == 0 || size >= rest.len() {
            groups.push(rest);
            break;
        }
        let (head, tail) = rest.split_at(rest.len() - size);
        groups.push(tail);
        rest = head;
        index += 1;
    }
    groups.reverse();
    groups.join(separator)
}

/// A preview from `locale -k LC_NUMERIC LC_MONETARY LC_MEASUREMENT` output
/// and `date` lines (date, time).
pub fn preview_from(keys: &str, date: &str) -> Preview {
    let values = assignments(keys);
    let get = |name: &str| {
        values
            .iter()
            .find(|(key, _)| key == name)
            .map_or("", |(_, value)| value.as_str())
    };
    let grouping = |name: &str| -> Vec<usize> {
        get(name)
            .split(';')
            .filter_map(|part| part.parse::<i64>().ok())
            .filter(|size| *size > 0)
            .map(|size| size as usize)
            .collect()
    };
    let decimal = match get("decimal_point") {
        "" => ".",
        point => point,
    };
    let number = format!(
        "{}{decimal}89",
        group("1234567", &grouping("grouping"), get("thousands_sep"))
    );
    let mon_decimal = match get("mon_decimal_point") {
        "" => decimal,
        point => point,
    };
    let amount = format!(
        "{}{mon_decimal}50",
        group("1234", &grouping("mon_grouping"), get("mon_thousands_sep"))
    );
    let symbol = match get("currency_symbol") {
        "" => get("int_curr_symbol").trim(),
        symbol => symbol,
    };
    let space = if get("p_sep_by_space") == "1" {
        " "
    } else {
        ""
    };
    let currency = if symbol.is_empty() {
        amount
    } else if get("p_cs_precedes") == "0" {
        format!("{amount}{space}{symbol}")
    } else {
        format!("{symbol}{space}{amount}")
    };
    let mut lines = date.lines();
    Preview {
        date: lines.next().unwrap_or("").trim().to_owned(),
        time: lines.next().unwrap_or("").trim().to_owned(),
        number,
        currency,
        metric: get("measurement") != "2",
    }
}

/// The programs this page runs (tests use their own).
#[derive(Clone, Debug)]
pub struct Tools {
    pub locale: PathBuf,
    pub date: PathBuf,
}

impl Default for Tools {
    fn default() -> Self {
        Self {
            locale: "locale".into(),
            date: "date".into(),
        }
    }
}

async fn run(program: &Path, args: &[&str], lc_all: Option<&str>) -> Option<String> {
    let mut command = tokio::process::Command::new(program);
    command
        .args(args)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    if let Some(locale) = lc_all {
        command.env("LC_ALL", locale);
    }
    let output = tokio::time::timeout(Duration::from_secs(5), command.output())
        .await
        .ok()?
        .ok()?;
    let mut text = String::from_utf8(output.stdout).ok()?;
    text.truncate(64 * 1024);
    output.status.success().then_some(text)
}

impl Tools {
    async fn installed(&self) -> Vec<Installed> {
        let Some(list) = run(&self.locale, &["-a"], None).await else {
            return Vec::new();
        };
        let mut installed = Vec::new();
        for name in list.lines().map(str::trim).take(400) {
            if !locale::valid_locale(name)
                || matches!(name, "C" | "POSIX")
                || !name.to_lowercase().contains("utf")
            {
                continue;
            }
            let name = normalize(name);
            if installed.iter().any(|known: &Installed| known.name == name) {
                continue;
            }
            let identity = run(&self.locale, &["-k", "LC_IDENTIFICATION"], Some(&name))
                .await
                .map(|text| assignments(&text))
                .unwrap_or_default();
            let get = |key: &str| {
                identity
                    .iter()
                    .find(|(name, _)| name == key)
                    .map(|(_, value)| value.clone())
                    .unwrap_or_default()
            };
            let (language, territory) = (get("language"), get("territory"));
            let label = match (language.is_empty(), territory.is_empty()) {
                _ if name.starts_with("C.") => "Unspecified (C)".to_owned(),
                (false, false) => format!("{language} ({territory})"),
                (false, true) => language,
                _ => name.clone(),
            };
            installed.push(Installed { name, label });
        }
        installed.sort_by_key(|a| a.label.to_lowercase());
        installed
    }

    async fn preview(&self, name: &str) -> Option<Preview> {
        let keys = run(
            &self.locale,
            &["-k", "LC_NUMERIC", "LC_MONETARY", "LC_MEASUREMENT"],
            Some(name),
        )
        .await?;
        let sample = format!("@{SAMPLE_TIME}");
        let date = run(&self.date, &["-d", &sample, "+%x%n%X"], Some(name))
            .await
            .unwrap_or_default();
        Some(preview_from(&keys, &date))
    }
}

/// localed, AccountsService and `locale.conf` for the page.
#[derive(Clone)]
pub struct RegionService {
    system: Arc<tokio::sync::OnceCell<zbus::Connection>>,
    installed: Arc<tokio::sync::OnceCell<Arc<Vec<Installed>>>>,
    config_home: Option<PathBuf>,
    uid: u64,
    tools: Tools,
}

impl Default for RegionService {
    fn default() -> Self {
        Self {
            system: Arc::default(),
            installed: Arc::default(),
            config_home: canopa_config::config_home().ok(),
            // SAFETY: getuid has no preconditions.
            uid: u64::from(unsafe { libc::getuid() }),
            tools: Tools::default(),
        }
    }
}

fn failure(error: canopa_services::Error) -> ServiceError {
    use canopa_services::Error;
    ServiceError::Failed(match error {
        Error::Unavailable => "systemd-localed is not available.".into(),
        Error::NotAuthorized => "The system did not allow this change.".into(),
        Error::TimedOut => "The system did not answer in time.".into(),
        Error::Changed => "The setting changed. Try again.".into(),
        Error::Invalid(what) => format!("That is not a valid {what}."),
        Error::Refused(message) => message,
    })
}

impl RegionService {
    /// A service on an existing connection, with `locale.conf` under
    /// `config_home`, for user `uid`, running `tools` (tests).
    pub fn with(system: zbus::Connection, config_home: PathBuf, uid: u64, tools: Tools) -> Self {
        Self {
            system: Arc::new(tokio::sync::OnceCell::new_with(Some(system))),
            installed: Arc::default(),
            config_home: Some(config_home),
            uid,
            tools,
        }
    }

    fn user_file(&self) -> Option<PathBuf> {
        Some(self.config_home.as_ref()?.join(USER_FILE))
    }

    fn user(&self) -> UserLocale {
        self.user_file()
            .and_then(|path| canopa_file::read_bounded(&path, 64 * 1024).ok())
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .map(|text| read_user(&text))
            .unwrap_or_default()
    }

    async fn connection(&self) -> Option<zbus::Connection> {
        self.system
            .get_or_try_init(canopa_services::bus::system)
            .await
            .ok()
            .cloned()
    }

    async fn state(&self, system: Option<SystemLocale>) -> RegionState {
        let tools = self.tools.clone();
        let installed = self
            .installed
            .get_or_init(|| async move { Arc::new(tools.installed().await) })
            .await
            .clone();
        let mut state = RegionState {
            system,
            user: self.user(),
            session: std::env::var("LANG").unwrap_or_default(),
            installed,
            preview: None,
        };
        let formats = state.formats();
        if !formats.is_empty() {
            state.preview = self.tools.preview(&formats).await;
        }
        state
    }

    async fn read(&self) -> RegionState {
        let system = match self.connection().await {
            Some(connection) => locale::read(&connection).await.ok(),
            None => None,
        };
        self.state(system).await
    }

    fn write_user(&self, values: &[(&str, &str)]) -> Result<(), ServiceError> {
        let path = self
            .user_file()
            .ok_or_else(|| ServiceError::Failed("There is no configuration folder.".into()))?;
        let text = canopa_file::read_bounded(&path, 64 * 1024)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .unwrap_or_default();
        let save = || -> std::io::Result<()> {
            use std::io::Write;
            let parent = path.parent().ok_or(std::io::ErrorKind::InvalidInput)?;
            std::fs::create_dir_all(parent)?;
            let mut file = tempfile::NamedTempFile::new_in(parent)?;
            file.write_all(with_user_values(&text, values).as_bytes())?;
            file.as_file().sync_all()?;
            file.persist(&path).map_err(|error| error.error)?;
            Ok(())
        };
        save().map_err(|error| {
            ServiceError::Failed(format!("locale.conf could not be saved: {error}"))
        })
    }

    /// Tell AccountsService too, where it is running; it is not required.
    async fn record(&self, language: Option<&str>, formats: Option<&str>) {
        let Some(connection) = self.connection().await else {
            return;
        };
        let Ok(user) = accounts::find_by_id(&connection, self.uid).await else {
            return;
        };
        if let Some(language) = language {
            let _ = accounts::set_language(&connection, user.path.as_str(), language).await;
        }
        if let (Some(formats), Some(_)) = (formats, &user.formats_locale) {
            let _ = accounts::set_formats_locale(&connection, user.path.as_str(), formats).await;
        }
    }

    fn installed_name(&self, state: &RegionState, name: &str) -> Result<String, ServiceError> {
        state
            .installed
            .iter()
            .find(|locale| same_locale(&locale.name, name))
            .map(|locale| locale.name.clone())
            .ok_or_else(|| ServiceError::Failed("That language is not installed.".into()))
    }

    async fn run(&self, action: RegionAction) -> Result<(), ServiceError> {
        let state = self.read().await;
        match action {
            RegionAction::UserLanguage(name) => {
                let name = self.installed_name(&state, &name)?;
                self.write_user(&[("LANG", &name)])?;
                self.record(Some(&name), None).await;
                Ok(())
            }
            RegionAction::UserFormats(name) => {
                let name = self.installed_name(&state, &name)?;
                let values: Vec<(&str, &str)> = FORMAT_VARIABLES
                    .iter()
                    .map(|variable| (*variable, name.as_str()))
                    .collect();
                self.write_user(&values)?;
                self.record(None, Some(&name)).await;
                Ok(())
            }
            RegionAction::SystemLanguage(name) => {
                let name = self.installed_name(&state, &name)?;
                let connection = self
                    .connection()
                    .await
                    .ok_or_else(|| failure(canopa_services::Error::Unavailable))?;
                let current = state.system.unwrap_or_default();
                // Keep the other variables; the language replaces LANG.
                let mut assignments: Vec<(String, String)> = current
                    .assignments
                    .into_iter()
                    .filter(|(variable, _)| variable != "LANG" && variable != "LANGUAGE")
                    .collect();
                assignments.insert(0, ("LANG".into(), name));
                locale::set_locale(&connection, &assignments)
                    .await
                    .map_err(failure)
            }
            RegionAction::SystemKeyboard {
                layouts,
                variants,
                options,
            } => {
                let connection = self
                    .connection()
                    .await
                    .ok_or_else(|| failure(canopa_services::Error::Unavailable))?;
                let model = state
                    .system
                    .map(|system| system.x11_model)
                    .unwrap_or_default();
                locale::set_x11_keyboard(&connection, &layouts, &model, &variants, &options)
                    .await
                    .map_err(failure)
            }
        }
    }
}

impl LiveService for RegionService {
    type State = RegionState;
    type Action = RegionAction;

    fn name(&self) -> &str {
        "systemd-localed"
    }

    fn snapshot(&self) -> BoxFuture<'static, Result<RegionState, ServiceError>> {
        let service = self.clone();
        async move { Ok(service.read().await) }.boxed()
    }

    fn updates(&self) -> BoxStream<'static, Result<RegionState, ServiceError>> {
        let service = self.clone();
        futures_util::stream::once(async move {
            let Some(connection) = service.connection().await else {
                return futures_util::stream::pending().boxed();
            };
            locale::watch(connection)
                .then(move |system| {
                    let service = service.clone();
                    async move {
                        Ok(service
                            .state(match system {
                                ServiceState::Available(system) => Some(system),
                                ServiceState::Unavailable => None,
                            })
                            .await)
                    }
                })
                .boxed()
        })
        .flatten()
        .boxed()
    }

    fn apply(
        &self,
        action: RegionAction,
    ) -> BoxFuture<'static, Result<Option<RegionState>, ServiceError>> {
        let service = self.clone();
        async move {
            service.run(action).await?;
            Ok(Some(service.read().await))
        }
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use canopa_services::testing::{TestBus, accounts::FakeAccounts, locale::FakeLocaled};

    #[test]
    fn locale_names_files_and_previews() {
        assert_eq!(normalize("fr_FR.utf8"), "fr_FR.UTF-8");
        assert_eq!(normalize("ca_ES.utf8@valencia"), "ca_ES.UTF-8@valencia");
        assert!(same_locale("en_US.UTF-8", "en_US.utf8"));
        let text = "# mine\nLANG=en_US.UTF-8\nLC_TIME=\"fr_FR.UTF-8\"\nLC_PAPER=bad value\n";
        assert_eq!(
            read_user(text),
            UserLocale {
                language: Some("en_US.UTF-8".into()),
                formats: Some("fr_FR.UTF-8".into())
            }
        );
        let written = with_user_values(
            text,
            &[("LANG", "de_DE.UTF-8"), ("LC_PAPER", "de_DE.UTF-8")],
        );
        assert_eq!(
            written,
            "# mine\nLC_TIME=\"fr_FR.UTF-8\"\nLANG=de_DE.UTF-8\nLC_PAPER=de_DE.UTF-8\n"
        );
        let french = preview_from(
            "decimal_point=\",\"\nthousands_sep=\" \"\ngrouping=3;3\nmon_decimal_point=\",\"\nmon_thousands_sep=\" \"\nmon_grouping=3;3\ncurrency_symbol=\"€\"\np_cs_precedes=0\np_sep_by_space=1\nmeasurement=1\n",
            "11/11/2026\n15:30:00\n",
        );
        assert_eq!(french.number, "1 234 567,89");
        assert_eq!(french.currency, "1 234,50 €");
        assert_eq!(
            (french.date.as_str(), french.time.as_str()),
            ("11/11/2026", "15:30:00")
        );
        assert!(french.metric);
        let american = preview_from(
            "decimal_point=\".\"\nthousands_sep=\",\"\ngrouping=3;3\nmon_decimal_point=\".\"\nmon_thousands_sep=\",\"\nmon_grouping=3;3\ncurrency_symbol=\"$\"\np_cs_precedes=1\np_sep_by_space=0\nmeasurement=2\n",
            "",
        );
        assert_eq!(
            (american.number.as_str(), american.currency.as_str()),
            ("1,234,567.89", "$1,234.50")
        );
        assert!(!american.metric);
        assert_eq!(group("12345678", &[3, 2], ","), "1,23,45,678");
    }

    fn tools(dir: &Path) -> Tools {
        use std::os::unix::fs::PermissionsExt;
        let script = |name: &str, body: &str| {
            let path = dir.join(name);
            std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            path
        };
        Tools {
            locale: script(
                "locale",
                r#"if [ "$1" = -a ]; then printf 'C\nC.utf8\nPOSIX\nen_US.utf8\nfr_FR.utf8\nen_GB.utf8\n'; exit 0; fi
case "$LC_ALL" in
  fr_FR*) printf 'language="French"\nterritory="France"\ndecimal_point=","\nthousands_sep=" "\ngrouping=3;3\n';;
  en_GB*) printf 'language="English"\nterritory="United Kingdom"\ndecimal_point="."\n';;
  *) printf 'language="American English"\nterritory="United States"\ndecimal_point="."\nthousands_sep=","\ngrouping=3;3\n';;
esac"#,
            ),
            date: script("date", r#"printf '%s\n12:00\n' "$LC_ALL""#),
        }
    }

    #[tokio::test]
    async fn the_page_writes_the_user_locale_and_asks_localed_for_the_system() {
        let bus = TestBus::new();
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config");
        let service =
            RegionService::with(bus.connect().await, config.clone(), 1000, tools(dir.path()));
        // Without localed or AccountsService the person's choice still works.
        let state = service.snapshot().await.unwrap();
        assert!(state.system.is_none());
        let labels: Vec<_> = state.installed.iter().map(|l| l.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "American English (United States)",
                "English (United Kingdom)",
                "French (France)",
                "Unspecified (C)"
            ]
        );
        let state = service
            .apply(RegionAction::UserFormats("fr_FR.utf8".into()))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(state.formats(), "fr_FR.UTF-8");
        assert_eq!(state.preview.as_ref().unwrap().number, "1 234 567,89");
        assert_eq!(state.preview.as_ref().unwrap().date, "fr_FR.UTF-8");
        let file = std::fs::read_to_string(config.join("locale.conf")).unwrap();
        assert!(file.contains("LC_TIME=fr_FR.UTF-8") && file.contains("LC_PAPER=fr_FR.UTF-8"));
        assert!(matches!(
            service
                .apply(RegionAction::UserLanguage("xx_XX.UTF-8".into()))
                .await,
            Err(ServiceError::Failed(_))
        ));

        let _localed = FakeLocaled::serve(&bus).await;
        let accounts = FakeAccounts::serve(&bus).await;
        let state = service
            .apply(RegionAction::UserLanguage("en_GB.UTF-8".into()))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(state.language(), "en_GB.UTF-8");
        assert_eq!(
            state.system.as_ref().unwrap().get("LANG"),
            Some("en_US.UTF-8")
        );
        assert_eq!(
            accounts.state.lock().unwrap().users[&1000].language,
            "en_GB.UTF-8"
        );
        let state = service
            .apply(RegionAction::SystemLanguage("fr_FR.UTF-8".into()))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            state.system.as_ref().unwrap().get("LANG"),
            Some("fr_FR.UTF-8")
        );
        service
            .apply(RegionAction::SystemKeyboard {
                layouts: "fr".into(),
                variants: String::new(),
                options: String::new(),
            })
            .await
            .unwrap();
        assert_eq!(_localed.keyboard.lock().unwrap().0, "fr");
    }
}
