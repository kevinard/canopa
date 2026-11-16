//! Session actions and Settings pages as launcher results.
//!
//! Session rows reuse what the bar offers: logind's `Can*` answers decide which
//! rows exist, and Log out, Reboot and Power off open the configured
//! confirmation menus rather than acting at once. Settings pages come from the
//! Settings page registry, which `canopa-settings pages` prints without
//! opening a window, and open through its D-Bus `OpenPage`.
use super::{Action, NamedAction, ResultItem};
use crate::app::panel_search::{SearchField, SearchMatch, SearchText, best_match, rank_matches};
use crate::config::{ConfiguredCommandSpec, MenuConfig, MenuItemConfig, PanelAction};
use crate::session::{Capabilities, SessionAction};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionCommand {
    Lock,
    LogOut,
    Suspend,
    Hibernate,
    Reboot,
    PowerOff,
}

impl SessionCommand {
    pub const ALL: [Self; 6] = [
        Self::Lock,
        Self::LogOut,
        Self::Suspend,
        Self::Hibernate,
        Self::Reboot,
        Self::PowerOff,
    ];

    fn id(self) -> &'static str {
        match self {
            Self::Lock => "lock",
            Self::LogOut => "log-out",
            Self::Suspend => "suspend",
            Self::Hibernate => "hibernate",
            Self::Reboot => "reboot",
            Self::PowerOff => "power-off",
        }
    }

    fn label(self) -> String {
        match self {
            Self::Lock => fl!("shell-module-lock"),
            Self::LogOut => fl!("shell-module-log-out"),
            Self::Suspend => fl!("shell-module-suspend"),
            Self::Hibernate => fl!("shell-module-hibernate"),
            Self::Reboot => fl!("shell-module-reboot"),
            Self::PowerOff => fl!("shell-module-power-off"),
        }
    }

    /// English keywords, which always match, and the session language's.
    fn keywords(self) -> String {
        let (english, translated) = match self {
            Self::Lock => ("lock screen session", fl!("shell-session-lock-keywords")),
            Self::LogOut => (
                "log out sign out logout logoff exit quit session",
                fl!("shell-session-log-out-keywords"),
            ),
            Self::Suspend => (
                "suspend sleep standby",
                fl!("shell-session-suspend-keywords"),
            ),
            Self::Hibernate => (
                "hibernate sleep disk",
                fl!("shell-session-hibernate-keywords"),
            ),
            Self::Reboot => ("reboot restart", fl!("shell-session-reboot-keywords")),
            Self::PowerOff => (
                "power off shut down shutdown turn off halt",
                fl!("shell-session-power-off-keywords"),
            ),
        };
        format!("{english} {translated}")
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Lock => "system-lock-screen",
            Self::LogOut => "system-log-out",
            Self::Suspend => "system-suspend",
            Self::Hibernate => "system-hibernate",
            Self::Reboot => "system-reboot",
            Self::PowerOff => "system-shutdown",
        }
    }
}

/// One offered session row and what selecting it does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRow {
    pub command: SessionCommand,
    pub action: PanelAction,
}

fn items(items: &[MenuItemConfig]) -> Vec<&PanelAction> {
    items
        .iter()
        .flat_map(|item| item.action.iter().chain(self::items(&item.children)))
        .collect()
}

/// The first menu that `contains` an item action, as a confirmation.
fn confirmation(menus: &[MenuConfig], contains: impl Fn(&PanelAction) -> bool) -> Option<String> {
    menus
        .iter()
        .find(|menu| menu.source.is_none() && items(&menu.items).into_iter().any(&contains))
        .map(|menu| menu.name.clone())
}

/// `session "log-out"`, or a configuration's own `niri msg action quit`.
fn quits_session(action: &PanelAction) -> bool {
    let argv: Vec<&str> = match action {
        PanelAction::Compositor(crate::config::CompositorAction::LogOut) => return true,
        PanelAction::Launch(argv) => argv.iter().map(String::as_str).collect(),
        PanelAction::Command(command) => match command.spec() {
            ConfiguredCommandSpec::Argv { program, args } => std::iter::once(program.as_str())
                .chain(args.iter().map(String::as_str))
                .collect(),
            ConfiguredCommandSpec::Shell(line) => line.split_whitespace().collect(),
        },
        _ => return false,
    };
    argv.first()
        .is_some_and(|program| program.rsplit('/').next() == Some("niri"))
        && argv.get(1..4) == Some(&["msg", "action", "quit"][..])
}

/// The rows to offer. Suspend and Hibernate act at once, like the bar's
/// buttons; Hibernate needs a plain "yes" from logind. Log out, Reboot and
/// Power off are offered only through a configured confirmation menu.
pub fn offered_session_rows(
    menus: &[MenuConfig],
    capabilities: &Capabilities,
    lock: Option<Vec<String>>,
) -> Vec<SessionRow> {
    SessionCommand::ALL
        .into_iter()
        .filter_map(|command| {
            let logind = match command {
                SessionCommand::Suspend => Some(SessionAction::Suspend),
                SessionCommand::Hibernate => Some(SessionAction::Hibernate),
                SessionCommand::Reboot => Some(SessionAction::Reboot),
                SessionCommand::PowerOff => Some(SessionAction::PowerOff),
                SessionCommand::Lock | SessionCommand::LogOut => None,
            };
            if logind.is_some_and(|action| !capabilities.offers(action)) {
                return None;
            }
            let action = match command {
                SessionCommand::Lock => PanelAction::Launch(lock.clone()?),
                SessionCommand::LogOut => {
                    PanelAction::OpenMenu(confirmation(menus, quits_session)?)
                }
                SessionCommand::Suspend | SessionCommand::Hibernate => {
                    PanelAction::Session(logind?)
                }
                SessionCommand::Reboot | SessionCommand::PowerOff => {
                    let logind = logind?;
                    PanelAction::OpenMenu(confirmation(menus, |action| {
                        *action == PanelAction::Session(logind)
                    })?)
                }
            };
            Some(SessionRow { command, action })
        })
        .collect()
}

/// The lock command the bar's Lock button uses, else logind's `lock-session`.
pub fn lock_command(search_path: &[std::path::PathBuf]) -> Option<Vec<String>> {
    [&["canopa-lock"][..], &["loginctl", "lock-session"][..]]
        .into_iter()
        .find(|argv| crate::desktop_apps::find_program(argv[0], search_path).is_some())
        .map(|argv| argv.iter().map(|word| (*word).to_owned()).collect())
}

/// [`lock_command`] on this process's `PATH`, looked up once.
pub fn installed_lock_command() -> Option<Vec<String>> {
    static LOCK: std::sync::OnceLock<Option<Vec<String>>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| lock_command(&crate::desktop_apps::process_search_path()))
        .clone()
}

/// Session rows matching a nonempty query.
pub fn session_results(rows: &[SessionRow], query: &str) -> Vec<ResultItem> {
    if query.trim().is_empty() {
        return Vec::new();
    }
    let matches = rows
        .iter()
        .enumerate()
        .filter_map(|(source_index, row)| {
            let command = row.command;
            let (label, keywords) = (command.label(), command.keywords());
            let score = best_match(
                &[
                    SearchText {
                        field: SearchField::Label,
                        text: &label,
                    },
                    SearchText {
                        field: SearchField::Secondary,
                        text: &keywords,
                    },
                ],
                query,
            )?;
            let confirms = matches!(row.action, PanelAction::OpenMenu(_));
            Some(SearchMatch {
                source_index,
                score,
                item: ResultItem {
                    id: format!("session:{}", command.id()),
                    label: if confirms {
                        fl!("shell-session-confirmed-label", action = label.clone())
                    } else {
                        label.clone()
                    },
                    description: if confirms {
                        fl!("shell-session-row-confirms")
                    } else {
                        fl!("shell-session-row")
                    },
                    icon: Some(command.icon().into()),
                    actions: vec![NamedAction {
                        label,
                        action: Action::Panel(row.action.clone()),
                    }],
                },
            })
        })
        .collect();
    rank_matches(matches)
}

/// One visible Settings page, as the registry declares it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct SettingsPage {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
}

const PAGES_BYTES: usize = 64 * 1024;
const MAX_PAGES: usize = 64;

/// The output of `canopa-settings pages`, bounded and checked.
pub fn parse_pages(bytes: &[u8]) -> Result<Vec<SettingsPage>, String> {
    #[derive(serde::Deserialize)]
    struct Catalog {
        pages: Vec<SettingsPage>,
    }
    if bytes.len() > PAGES_BYTES {
        return Err(fl!("shell-settings-pages-too-large"));
    }
    let catalog: Catalog =
        serde_json::from_slice(bytes).map_err(|_| fl!("shell-settings-pages-invalid"))?;
    let short =
        |text: &str, limit: usize| text.len() <= limit && !text.chars().any(char::is_control);
    let valid = catalog.pages.len() <= MAX_PAGES
        && catalog.pages.iter().all(|page| {
            !page.id.is_empty()
                && page.id.len() <= 64
                && page
                    .id
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
                && !page.title.is_empty()
                && short(&page.title, 128)
                && short(&page.description, 512)
                && page.icon.as_deref().is_none_or(|icon| {
                    short(icon, 128) && crate::icon_theme::is_valid_icon_name(icon)
                })
                && page.keywords.len() <= 32
                && page.keywords.iter().all(|keyword| short(keyword, 64))
        });
    if !valid {
        return Err(fl!("shell-settings-pages-invalid"));
    }
    Ok(catalog.pages)
}

/// Runs `canopa-settings pages`, which reads the registry and exits.
#[cfg_attr(test, allow(dead_code))]
pub async fn fetch_pages() -> Result<Vec<SettingsPage>, String> {
    use tokio::io::AsyncReadExt;
    let mut child = tokio::process::Command::new("canopa-settings")
        .arg("pages")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| fl!("shell-settings-not-installed"))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| fl!("shell-settings-pages-failed"))?;
    let mut bytes = Vec::new();
    let read = async {
        (&mut stdout)
            .take(PAGES_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .await?;
        child.wait().await
    };
    let status = tokio::time::timeout(std::time::Duration::from_secs(3), read)
        .await
        .map_err(|_| fl!("shell-settings-pages-timed-out"))?
        .map_err(|error| error.to_string())?;
    if !status.success() {
        return Err(fl!("shell-settings-pages-failed"));
    }
    parse_pages(&bytes)
}

/// Pages whose title, description or keywords match. A blank query lists
/// every page only in the `settings` scope.
pub fn page_results(pages: &[SettingsPage], query: &str, scoped: bool) -> Vec<ResultItem> {
    if query.trim().is_empty() && !scoped {
        return Vec::new();
    }
    let matches = pages
        .iter()
        .enumerate()
        .filter_map(|(source_index, page)| {
            let keywords = page.keywords.join(" ");
            let score = best_match(
                &[
                    SearchText {
                        field: SearchField::Label,
                        text: &page.title,
                    },
                    SearchText {
                        field: SearchField::Secondary,
                        text: &keywords,
                    },
                    SearchText {
                        field: SearchField::Breadcrumb,
                        text: &page.description,
                    },
                ],
                query,
            )?;
            Some(SearchMatch {
                source_index,
                score,
                item: ResultItem {
                    id: format!("settings-page:{}", page.id),
                    label: fl!("shell-settings-page-label", page = page.title.clone()),
                    description: fl!("shell-settings-page"),
                    icon: page
                        .icon
                        .clone()
                        .or_else(|| Some("preferences-system".into())),
                    actions: vec![NamedAction {
                        label: fl!("shell-launcher-open"),
                        action: Action::SettingsPage {
                            id: page.id.clone(),
                        },
                    }],
                },
            })
        })
        .collect();
    rank_matches(matches)
}

/// `OpenPage` on the running (or D-Bus-activated) Settings window, falling
/// back to `canopa-settings --page` when the bus has no Settings service.
#[cfg_attr(test, allow(dead_code))]
pub async fn open_page(id: &str) -> std::io::Result<()> {
    let call = async {
        let connection = zbus::Connection::session().await?;
        let options = std::collections::HashMap::<&str, zbus::zvariant::Value<'_>>::new();
        connection
            .call_method(
                Some("org.canopa.Settings"),
                "/org/canopa/Settings",
                Some("org.canopa.Settings"),
                "OpenPage",
                &(id, options),
            )
            .await
            .map(|_| ())
    };
    match tokio::time::timeout(std::time::Duration::from_secs(10), call).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(zbus::Error::MethodError(name, _, _)))
            if name.as_str() == "org.freedesktop.DBus.Error.InvalidArgs" =>
        {
            Err(std::io::Error::other(fl!("shell-settings-no-such-page")))
        }
        _ => crate::command::launch_application(&[
            "canopa-settings".into(),
            "--page".into(),
            id.into(),
        ])
        .await
        .map(|_| ()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Availability;

    fn item(label: &str, action: PanelAction) -> MenuItemConfig {
        MenuItemConfig {
            label: label.into(),
            icon: None,
            keywords: vec![],
            action: Some(action),
            children: vec![],
        }
    }

    fn menu(name: &str, action: PanelAction) -> MenuConfig {
        MenuConfig {
            name: name.into(),
            title: None,
            show_icons: false,
            prompt: None,
            source: None,
            items: vec![
                item("Cancel", PanelAction::OpenBarPanel),
                item(name, action),
            ],
        }
    }

    fn menus() -> Vec<MenuConfig> {
        vec![
            menu(
                "Log out?",
                PanelAction::Compositor(crate::config::CompositorAction::LogOut),
            ),
            menu("Reboot?", PanelAction::Session(SessionAction::Reboot)),
            menu("Power off?", PanelAction::Session(SessionAction::PowerOff)),
        ]
    }

    fn commands(rows: &[SessionRow]) -> Vec<SessionCommand> {
        rows.iter().map(|row| row.command).collect()
    }

    #[test]
    fn session_rows_follow_logind_capabilities_and_confirmation_menus() {
        let lock = Some(vec!["canopa-lock".to_owned()]);
        let unknown = offered_session_rows(&menus(), &Capabilities::default(), lock.clone());
        // Unknown answers still offer everything except hibernate.
        assert_eq!(
            commands(&unknown),
            [
                SessionCommand::Lock,
                SessionCommand::LogOut,
                SessionCommand::Suspend,
                SessionCommand::Reboot,
                SessionCommand::PowerOff
            ]
        );
        assert_eq!(unknown[1].action, PanelAction::OpenMenu("Log out?".into()));
        assert_eq!(
            unknown[2].action,
            PanelAction::Session(SessionAction::Suspend)
        );
        assert_eq!(unknown[3].action, PanelAction::OpenMenu("Reboot?".into()));

        let capabilities = Capabilities {
            suspend: Availability::NotApplicable,
            hibernate: Availability::Yes,
            reboot: Availability::Challenge,
            power_off: Availability::No,
        };
        assert_eq!(
            commands(&offered_session_rows(&menus(), &capabilities, None)),
            [
                SessionCommand::LogOut,
                SessionCommand::Hibernate,
                SessionCommand::Reboot
            ]
        );
        // Without a confirmation menu, ending the session is not offered.
        assert_eq!(
            commands(&offered_session_rows(&[], &Capabilities::default(), lock)),
            [SessionCommand::Lock, SessionCommand::Suspend]
        );
    }

    #[test]
    fn session_results_match_names_and_synonyms_and_name_confirmations() {
        let rows = offered_session_rows(&menus(), &Capabilities::default(), None);
        assert!(session_results(&rows, "").is_empty());
        let shut_down = session_results(&rows, "shut down");
        assert_eq!(shut_down[0].label, "Power off…");
        assert!(matches!(
            &shut_down[0].actions[0].action,
            Action::Panel(PanelAction::OpenMenu(name)) if name == "Power off?"
        ));
        assert_eq!(session_results(&rows, "restart")[0].label, "Reboot…");
        assert_eq!(session_results(&rows, "sleep")[0].label, "Suspend");
        assert_eq!(session_results(&rows, "sign out")[0].label, "Log out…");
        assert!(session_results(&rows, "lock").is_empty());
    }

    #[test]
    fn settings_pages_are_validated_and_searched_by_title_and_keywords() {
        let json = br#"{"pages":[
            {"id":"displays","title":"Displays","description":"Arrange monitors","icon":"video-display","keywords":["monitor","resolution"]},
            {"id":"night-light","title":"Night light","keywords":["blue light","warm"]}
        ]}"#;
        let pages = parse_pages(json).unwrap();
        assert_eq!(
            page_results(&pages, "monitor", false)[0].label,
            "Displays settings"
        );
        let warm = page_results(&pages, "warm", false);
        assert_eq!(warm.len(), 1);
        assert!(
            matches!(&warm[0].actions[0].action, Action::SettingsPage { id } if id == "night-light")
        );
        assert_eq!(warm[0].icon.as_deref(), Some("preferences-system"));
        assert!(page_results(&pages, "", false).is_empty());
        assert_eq!(page_results(&pages, "", true).len(), 2);
        for bad in [
            br#"{"pages":[{"id":"Bad Id","title":"X"}]}"#.as_slice(),
            br#"{"pages":[{"id":"x","title":""}]}"#,
            br#"{"pages":[{"id":"x","title":"X","icon":"../escape"}]}"#,
            br#"not json"#,
        ] {
            assert!(parse_pages(bad).is_err());
        }
    }
}
