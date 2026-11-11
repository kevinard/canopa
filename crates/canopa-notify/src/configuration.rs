use crate::{
    BannerIconSettings, DeliveryOutcome, DeliveryPolicyRule, NotificationSoundSettings, Urgency,
    normalize_notification_category,
};
use anyhow::Context;
use kdl::{KdlDocument, KdlEntry, KdlNode};
#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;

use canopa_config::{Component, Document, Request, SourceInfo};

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ConfigurationStatus {
    pub generation: u64,
    pub path: Option<PathBuf>,
    pub error: Option<String>,
    #[serde(default)]
    pub source: Option<SourceInfo>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct LoadedNotificationConfiguration {
    active: NotificationConfiguration,
    appearance: canopa_theme::Appearance,
    last_error: Option<String>,
    generation: u64,
    path: Option<PathBuf>,
    source: Option<SourceInfo>,
    request: Request,
}

impl LoadedNotificationConfiguration {
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    #[cfg(test)]
    pub(crate) fn reload_file(&mut self, path: &Path) -> anyhow::Result<()> {
        self.reload_source(Request::legacy(path))
    }

    pub(crate) fn request(&self) -> Request {
        self.request.clone()
    }

    pub(crate) fn reload_source(&mut self, request: Request) -> anyhow::Result<()> {
        let result = (|| {
            let document = Document::load(&request, Component::Notifications, "")?;
            let configuration = NotificationConfiguration::parse_document(document.selected())?;
            let mut appearance = document.appearance();
            std::sync::Arc::make_mut(&mut appearance.customization).ui
                [canopa_theme::UiKind::Notification as usize]
                .overlay(configuration.banner_ui);
            Ok::<_, anyhow::Error>((configuration, document.info().clone(), appearance))
        })();
        match result {
            Ok((configuration, source, appearance)) => {
                self.appearance = appearance;
                self.active = configuration;
                self.last_error = None;
                self.generation = self.generation.saturating_add(1);
                self.path = source.path.clone();
                self.source = Some(source);
                self.request = match request {
                    Request::File { format, .. } => Request::File {
                        path: self.path.clone().expect("file source has a path"),
                        format,
                    },
                    other => other,
                };
                Ok(())
            }
            Err(error) => {
                self.last_error = Some(format!("{error:#}"));
                Err(error)
            }
        }
    }

    pub(crate) fn delivery_policy_rules(&self) -> Vec<DeliveryPolicyRule> {
        self.active.delivery_policy_rules()
    }

    pub(crate) fn sound_settings(&self) -> NotificationSoundSettings {
        self.active.sound_settings()
    }

    pub(crate) fn configured_preferences(&self) -> crate::runtime::ConfiguredPreferences {
        crate::runtime::ConfiguredPreferences {
            do_not_disturb: self.active.do_not_disturb,
            apps: self.active.apps.clone(),
        }
    }

    pub(crate) fn appearance(&self) -> canopa_theme::Appearance {
        self.appearance.clone()
    }

    pub(crate) fn icon_settings(&self) -> BannerIconSettings {
        self.active.icons.clone()
    }

    pub(crate) fn error(&self) -> Option<String> {
        self.last_error.clone()
    }

    pub(crate) fn record_error(&mut self, error: &anyhow::Error) {
        self.last_error = Some(format!("{error:#}"));
    }

    pub(crate) fn status(&self) -> ConfigurationStatus {
        ConfigurationStatus {
            generation: self.generation,
            path: self.path.clone(),
            error: self.error(),
            source: self.source.clone(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct NotificationConfiguration {
    rules: Vec<NotificationConfigurationRule>,
    do_not_disturb: Option<crate::DoNotDisturbPreferences>,
    apps: std::collections::BTreeMap<String, crate::AppPreferences>,
    sound: NotificationSoundSettings,
    icons: BannerIconSettings,
    banner_ui: canopa_theme::UiStyle,
}

impl NotificationConfiguration {
    #[cfg(test)]
    fn parse(source: &str) -> anyhow::Result<Self> {
        Self::parse_document(&canopa_config::parse_kdl(source)?)
    }

    fn parse_document(doc: &KdlDocument) -> anyhow::Result<Self> {
        let mut rules = Vec::new();
        let mut sound = None;
        let mut icons = None;
        let mut banner_ui = None;
        let mut do_not_disturb = None;
        let mut apps = std::collections::BTreeMap::new();

        for node in doc.nodes() {
            anyhow::ensure!(
                node.ty().is_none() && node.entries().iter().all(|entry| entry.ty().is_none()),
                "Notification Configuration does not support type annotations"
            );
            match node.name().value() {
                "rule" => rules.push(parse_rule(node, rules.len() + 1)?),
                "do-not-disturb" => {
                    anyhow::ensure!(do_not_disturb.is_none(), "duplicate do-not-disturb blocks");
                    do_not_disturb = Some(parse_do_not_disturb(node)?);
                }
                "app" => {
                    let (identity, preferences) = parse_app(node)?;
                    anyhow::ensure!(
                        apps.len() < crate::MAX_APP_PREFERENCES,
                        "too many app nodes"
                    );
                    anyhow::ensure!(
                        apps.insert(identity.clone(), preferences).is_none(),
                        "duplicate app node {identity:?}"
                    );
                }
                "sound" => {
                    if sound.is_some() {
                        anyhow::bail!("Notification Configuration has duplicate sound nodes");
                    }
                    sound = Some(parse_sound(node)?);
                }
                "icons" => {
                    anyhow::ensure!(icons.is_none(), "duplicate icons nodes");
                    icons = Some(parse_icons(node)?);
                }
                "banners" => {
                    anyhow::ensure!(
                        banner_ui.is_none() && node.entries().is_empty(),
                        "invalid or duplicate banners block"
                    );
                    let children = node
                        .children()
                        .ok_or_else(|| anyhow::anyhow!("banners requires a block"))?;
                    anyhow::ensure!(
                        children.nodes().len() <= 1
                            && children.nodes().iter().all(|n| n.name().value() == "ui"),
                        "banners accepts one ui block"
                    );
                    banner_ui = Some(match children.get("ui") {
                        Some(ui) => {
                            canopa_config::ui_style(ui, canopa_theme::UiKind::Notification)?
                        }
                        None => Default::default(),
                    });
                }
                _ => {
                    anyhow::bail!(
                        "invalid Notification Configuration node: expected rule, app, do-not-disturb, sound, icons or banners"
                    );
                }
            }
        }

        Ok(Self {
            rules,
            do_not_disturb,
            apps,
            sound: sound.unwrap_or_default(),
            icons: icons.unwrap_or_default(),
            banner_ui: banner_ui.unwrap_or_default(),
        })
    }

    fn delivery_policy_rules(&self) -> Vec<DeliveryPolicyRule> {
        self.rules
            .iter()
            .map(|rule| match &rule.category {
                Some(category) => DeliveryPolicyRule::for_category(
                    rule.app_identity.clone(),
                    category.clone(),
                    rule.urgency,
                    rule.outcome,
                    rule.reference.clone(),
                ),
                None => DeliveryPolicyRule::new(
                    rule.app_identity.clone(),
                    rule.urgency,
                    rule.outcome,
                    rule.reference.clone(),
                ),
            })
            .collect()
    }

    fn sound_settings(&self) -> NotificationSoundSettings {
        self.sound.clone()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NotificationConfigurationRule {
    app_identity: String,
    category: Option<String>,
    urgency: Option<Urgency>,
    outcome: DeliveryOutcome,
    reference: String,
}

fn parse_rule(node: &KdlNode, rule_number: usize) -> anyhow::Result<NotificationConfigurationRule> {
    let entries = RuleEntries::from_node(node, rule_number)?;
    let app_identity = required_property(entries.app_identity, "app", rule_number)?;
    if app_identity.trim().is_empty() {
        anyhow::bail!("rule {rule_number} app must not be empty");
    }
    let outcome = DeliveryOutcome::parse_config_value(&required_property(
        entries.outcome,
        "outcome",
        rule_number,
    )?)
    .map_err(|_| {
        anyhow::anyhow!(
            "rule {rule_number} outcome must be allow/shown, silence/silenced or drop/dropped"
        )
    })?;
    let urgency = entries
        .urgency
        .map(|value| {
            Urgency::parse_config_value(&value).map_err(|_| {
                anyhow::anyhow!("rule {rule_number} urgency must be low, normal or critical")
            })
        })
        .transpose()?;
    let category = entries
        .category
        .map(|value| {
            normalize_notification_category(&value)
                .ok_or_else(|| anyhow::anyhow!("rule {rule_number} has invalid category"))
        })
        .transpose()?;
    let reference = entries
        .rule_id
        .unwrap_or_else(|| format!("config-rule-{rule_number}"));

    Ok(NotificationConfigurationRule {
        app_identity,
        category,
        urgency,
        outcome,
        reference,
    })
}

fn bool_property(entry: &KdlEntry, owner: &str) -> anyhow::Result<bool> {
    entry
        .value()
        .as_bool()
        .ok_or_else(|| anyhow::anyhow!("{owner} properties must be booleans"))
}

/// `do-not-disturb { schedule from="22:00" to="07:00" days="mon tue";
/// break-through alarms=#true calls=#true; suppress fullscreen=#true screen-sharing=#true }`
fn parse_do_not_disturb(node: &KdlNode) -> anyhow::Result<crate::DoNotDisturbPreferences> {
    anyhow::ensure!(
        node.entries().is_empty(),
        "do-not-disturb takes only child nodes"
    );
    let mut preferences = crate::DoNotDisturbPreferences::default();
    let mut seen = std::collections::HashSet::new();
    for child in node.children().map(|c| c.nodes()).unwrap_or_default() {
        let name = child.name().value();
        anyhow::ensure!(
            child.children().is_none() && seen.insert(name.to_owned()),
            "invalid or duplicate do-not-disturb {name}"
        );
        match name {
            "schedule" => {
                let mut start = None;
                let mut end = None;
                let mut days = crate::Weekdays::ALL;
                for entry in child.entries() {
                    let value = entry.value().as_string();
                    match (entry.name().map(|n| n.value()), value) {
                        (Some("from"), Some(value)) => start = crate::ClockTime::parse(value),
                        (Some("to"), Some(value)) => end = crate::ClockTime::parse(value),
                        (Some("days"), Some(value)) => {
                            days = crate::Weekdays::parse(value.split_whitespace())
                                .context("schedule days must name mon..sun")?
                        }
                        _ => anyhow::bail!("schedule accepts from=, to= and days= strings"),
                    }
                }
                preferences.schedule = Some(crate::DoNotDisturbSchedule {
                    start: start.context("schedule from must be HH:MM")?,
                    end: end.context("schedule to must be HH:MM")?,
                    days,
                });
            }
            "break-through" => {
                for entry in child.entries() {
                    match entry.name().map(|n| n.value()) {
                        Some("alarms") => {
                            preferences.break_through.alarms =
                                bool_property(entry, "break-through")?
                        }
                        Some("calls") => {
                            preferences.break_through.calls = bool_property(entry, "break-through")?
                        }
                        _ => anyhow::bail!("break-through accepts alarms and calls"),
                    }
                }
            }
            "suppress" => {
                for entry in child.entries() {
                    match entry.name().map(|n| n.value()) {
                        Some("fullscreen") => {
                            preferences.suppress_fullscreen = bool_property(entry, "suppress")?
                        }
                        Some("screen-sharing") => {
                            preferences.suppress_screen_sharing = bool_property(entry, "suppress")?
                        }
                        _ => anyhow::bail!("suppress accepts fullscreen and screen-sharing"),
                    }
                }
            }
            _ => anyhow::bail!("do-not-disturb accepts schedule, break-through and suppress"),
        }
    }
    Ok(preferences)
}

/// `app "org.example.App" sound=#false lock-screen="content"`
fn parse_app(node: &KdlNode) -> anyhow::Result<(String, crate::AppPreferences)> {
    anyhow::ensure!(node.children().is_none(), "app must not have child nodes");
    let mut identity = None;
    let mut preferences = crate::AppPreferences::default();
    for entry in node.entries() {
        match entry.name().map(|n| n.value()) {
            None => {
                let value = entry
                    .value()
                    .as_string()
                    .context("app identity must be a string")?;
                anyhow::ensure!(
                    identity.is_none() && !value.trim().is_empty() && value.len() <= 512,
                    "app needs one non-empty identity"
                );
                identity = Some(value.to_owned());
            }
            Some("sound") => preferences.sound = Some(bool_property(entry, "app")?),
            Some("lock-screen") => {
                preferences.lock_screen = Some(
                    entry
                        .value()
                        .as_string()
                        .and_then(crate::LockScreenVisibility::parse)
                        .context("app lock-screen must be hidden, name-only or content")?,
                )
            }
            Some(_) => anyhow::bail!("app accepts sound and lock-screen"),
        }
    }
    Ok((identity.context("app needs an identity")?, preferences))
}

fn parse_sound(node: &KdlNode) -> anyhow::Result<NotificationSoundSettings> {
    let entries = SoundEntries::from_node(node)?;
    NotificationSoundSettings::new(
        entries.enabled.unwrap_or(true),
        entries
            .theme
            .unwrap_or_else(|| NotificationSoundSettings::default().theme),
        entries
            .default_sound
            .unwrap_or_else(|| NotificationSoundSettings::default().default_sound),
    )
}

fn parse_icons(node: &KdlNode) -> anyhow::Result<BannerIconSettings> {
    anyhow::ensure!(node.children().is_none(), "icons must not have child nodes");
    let mut theme = None;
    let mut scale = None;
    for entry in node.entries() {
        match entry.name().map(|name| name.value()) {
            Some("theme") => set_string_property(&mut theme, entry, "theme", "icons")?,
            Some("scale") => {
                anyhow::ensure!(scale.is_none(), "duplicate icon scale");
                scale = Some(
                    entry
                        .value()
                        .as_integer()
                        .and_then(|value| u16::try_from(value).ok())
                        .context("icon scale must be an integer between 1 and 8")?,
                );
            }
            _ => anyhow::bail!("icons accepts only theme and scale properties"),
        }
    }
    match theme {
        Some(theme) => BannerIconSettings::new(theme, scale.unwrap_or(1)),
        None => BannerIconSettings::desktop(scale.unwrap_or(1)),
    }
}

#[derive(Default)]
struct SoundEntries {
    enabled: Option<bool>,
    theme: Option<String>,
    default_sound: Option<String>,
}

impl SoundEntries {
    fn from_node(node: &KdlNode) -> anyhow::Result<Self> {
        if node.children().is_some() {
            anyhow::bail!("sound must not have child nodes");
        }

        let mut entries = Self::default();
        for entry in node.entries() {
            match entry.name().map(|name| name.value()) {
                None => anyhow::bail!("sound must not have positional values"),
                Some("enabled") => {
                    set_bool_property(&mut entries.enabled, entry, "enabled", "sound")?
                }
                Some("theme") => set_string_property(&mut entries.theme, entry, "theme", "sound")?,
                Some("default") => {
                    set_string_property(&mut entries.default_sound, entry, "default", "sound")?
                }
                Some(_) => {
                    anyhow::bail!("sound has unknown property; expected enabled, theme or default")
                }
            }
        }

        Ok(entries)
    }
}

#[derive(Default)]
struct RuleEntries {
    rule_id: Option<String>,
    app_identity: Option<String>,
    outcome: Option<String>,
    category: Option<String>,
    urgency: Option<String>,
}

impl RuleEntries {
    fn from_node(node: &KdlNode, rule_number: usize) -> anyhow::Result<Self> {
        if node.children().is_some() {
            anyhow::bail!("rule {rule_number} must not have child nodes");
        }

        let mut entries = Self::default();
        for entry in node.entries() {
            match entry.name().map(|name| name.value()) {
                None => entries.set_rule_id(entry, rule_number)?,
                Some("app") => {
                    set_rule_string_property(&mut entries.app_identity, entry, "app", rule_number)?
                }
                Some("outcome") => {
                    set_rule_string_property(&mut entries.outcome, entry, "outcome", rule_number)?
                }
                Some("category") => {
                    set_rule_string_property(&mut entries.category, entry, "category", rule_number)?
                }
                Some("urgency") => {
                    set_rule_string_property(&mut entries.urgency, entry, "urgency", rule_number)?
                }
                Some(_) => anyhow::bail!(
                    "rule {rule_number} has unknown property; expected app, outcome, category or urgency"
                ),
            }
        }

        Ok(entries)
    }

    fn set_rule_id(&mut self, entry: &KdlEntry, rule_number: usize) -> anyhow::Result<()> {
        if self.rule_id.is_some() {
            anyhow::bail!("rule {rule_number} has too many positional values");
        }
        let Some(value) = entry.value().as_string() else {
            anyhow::bail!("rule {rule_number} Config Rule ID must be a string");
        };
        if value.trim().is_empty() {
            anyhow::bail!("rule {rule_number} Config Rule ID must not be empty");
        }

        self.rule_id = Some(value.to_string());
        Ok(())
    }
}

fn set_rule_string_property(
    slot: &mut Option<String>,
    entry: &KdlEntry,
    name: &str,
    rule_number: usize,
) -> anyhow::Result<()> {
    set_string_property(slot, entry, name, &format!("rule {rule_number}"))
}

fn set_string_property(
    slot: &mut Option<String>,
    entry: &KdlEntry,
    name: &str,
    owner: &str,
) -> anyhow::Result<()> {
    if slot.is_some() {
        anyhow::bail!("{owner} has duplicate {name} properties");
    }
    let Some(value) = entry.value().as_string() else {
        anyhow::bail!("{owner} property {name} must be a string");
    };
    *slot = Some(value.to_string());
    Ok(())
}

fn set_bool_property(
    slot: &mut Option<bool>,
    entry: &KdlEntry,
    name: &str,
    owner: &str,
) -> anyhow::Result<()> {
    if slot.is_some() {
        anyhow::bail!("{owner} has duplicate {name} properties");
    }
    let Some(value) = entry.value().as_bool() else {
        anyhow::bail!("{owner} property {name} must be a boolean");
    };
    *slot = Some(value);
    Ok(())
}

fn required_property(
    value: Option<String>,
    name: &str,
    rule_number: usize,
) -> anyhow::Result<String> {
    value.ok_or_else(|| anyhow::anyhow!("rule {rule_number} is missing {name}"))
}

/// Validate using the same intake and schema as startup/reload, without opening a service.
pub fn check_configuration(request: &Request) -> anyhow::Result<SourceInfo> {
    let document = Document::load(request, Component::Notifications, "")?;
    check_configuration_document(&document)
}

/// Validate an already admitted immutable source, for local diagnostic clients.
pub fn check_configuration_document(document: &Document) -> anyhow::Result<SourceInfo> {
    NotificationConfiguration::parse_document(document.selected())?;
    Ok(document.info().clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_kdl_notification_rules_into_delivery_policy_rules() {
        let configuration = NotificationConfiguration::parse(
            r#"
            rule "quiet-chat" app="Chat" category="im.received" outcome="silenced"
            rule app="Build" urgency="critical" outcome="drop"
            "#,
        )
        .unwrap();

        let rules = configuration.delivery_policy_rules();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].app_identity, "Chat");
        assert_eq!(rules[0].category.as_deref(), Some("im.received"));
        assert_eq!(rules[0].outcome, DeliveryOutcome::Silenced);
        assert_eq!(rules[0].reference, "quiet-chat");
        assert_eq!(rules[1].app_identity, "Build");
        assert_eq!(rules[1].urgency, Some(Urgency::Critical));
        assert_eq!(rules[1].outcome, DeliveryOutcome::Dropped);
        assert_eq!(rules[1].reference, "config-rule-2");
    }

    #[test]
    fn parses_top_level_sound_settings() {
        let configuration = NotificationConfiguration::parse(
            r#"
            sound enabled=#false theme="freedesktop" default="message-new-instant"
            rule "quiet-chat" app="Chat" outcome="silenced"
            "#,
        )
        .unwrap();

        let sound = configuration.sound_settings();
        assert!(!sound.enabled);
        assert_eq!(sound.theme, "freedesktop");
        assert_eq!(sound.default_sound, "message-new-instant");
    }

    #[test]
    fn rejects_invalid_sound_settings() {
        let duplicate_error = NotificationConfiguration::parse(
            r#"sound enabled=#true
            sound enabled=#false"#,
        )
        .unwrap_err()
        .to_string();
        assert!(duplicate_error.contains("duplicate sound"));

        let empty_theme_error = NotificationConfiguration::parse(r#"sound theme="""#)
            .unwrap_err()
            .to_string();
        assert!(empty_theme_error.contains("sound theme"));

        let path_default_error = NotificationConfiguration::parse(r#"sound default="/tmp/a.oga""#)
            .unwrap_err()
            .to_string();
        assert!(path_default_error.contains("default notification sound"));
    }

    #[test]
    fn rejects_unknown_nodes_and_properties() {
        let node_error =
            NotificationConfiguration::parse(r#"rules { rule app="Chat" outcome="drop" }"#)
                .unwrap_err()
                .to_string();
        assert!(node_error.contains("expected rule"));

        let property_error =
            NotificationConfiguration::parse(r#"rule app="Chat" outcome="drop" sound="on""#)
                .unwrap_err()
                .to_string();
        assert!(property_error.contains("unknown property"));
    }

    #[test]
    fn rejects_invalid_rule_entry_shapes() {
        let child_error =
            NotificationConfiguration::parse(r#"rule app="Chat" outcome="drop" { nested }"#)
                .unwrap_err()
                .to_string();
        assert!(child_error.contains("must not have child nodes"));

        let duplicate_error =
            NotificationConfiguration::parse(r#"rule app="Chat" app="Other" outcome="drop""#)
                .unwrap_err()
                .to_string();
        assert!(duplicate_error.contains("duplicate app properties"));

        let type_error = NotificationConfiguration::parse(r#"rule app=123 outcome="drop""#)
            .unwrap_err()
            .to_string();
        assert!(type_error.contains("property app must be a string"));

        let category_error = NotificationConfiguration::parse(
            r#"rule app="Chat" category="IM Received" outcome="drop""#,
        )
        .unwrap_err()
        .to_string();
        assert!(category_error.contains("invalid category"));
    }

    #[test]
    fn parses_do_not_disturb_and_app_preferences() {
        let configuration = NotificationConfiguration::parse(
            r#"
            do-not-disturb {
                schedule from="22:00" to="7:30" days="mon tue"
                break-through alarms=#true
                suppress fullscreen=#false
            }
            app "org.gnome.Fractal" sound=#false lock-screen="content"
            app "Mail" lock-screen="hidden"
            "#,
        )
        .unwrap();
        let dnd = configuration.do_not_disturb.unwrap();
        let schedule = dnd.schedule.unwrap();
        assert_eq!(
            (schedule.start.to_string(), schedule.end.to_string()),
            ("22:00".into(), "07:30".into())
        );
        assert!(schedule.days.contains(chrono::Weekday::Tue));
        assert!(!schedule.days.contains(chrono::Weekday::Wed));
        assert!(dnd.break_through.alarms && !dnd.break_through.calls);
        assert!(!dnd.suppress_fullscreen && dnd.suppress_screen_sharing);
        assert_eq!(configuration.apps["org.gnome.Fractal"].sound, Some(false));
        assert_eq!(
            configuration.apps["Mail"].lock_screen,
            Some(crate::LockScreenVisibility::Hidden)
        );
        for invalid in [
            r#"do-not-disturb { schedule from="25:00" to="07:00"; }"#,
            r#"do-not-disturb { schedule from="22:00"; }"#,
            r#"do-not-disturb { schedule from="22:00" to="07:00" days="noday"; }"#,
            r#"do-not-disturb { break-through pagers=#true; }"#,
            r#"do-not-disturb { suppress fullscreen="yes"; }"#,
            r#"do-not-disturb { }
               do-not-disturb { }"#,
            r#"app "Mail" lock-screen="maybe""#,
            r#"app sound=#false"#,
            r#"app "Mail" sound=#true
               app "Mail" sound=#false"#,
        ] {
            assert!(
                NotificationConfiguration::parse(invalid).is_err(),
                "{invalid}"
            );
        }
    }

    #[test]
    fn invalid_reload_keeps_previous_valid_configuration() {
        let mut loaded = LoadedNotificationConfiguration::empty();
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("notify.kdl");

        std::fs::write(&config, r#"rule "drop-chat" app="Chat" outcome="dropped""#).unwrap();
        loaded.reload_file(&config).unwrap();
        let valid_rules = loaded.delivery_policy_rules();

        std::fs::write(
            &config,
            r#"rule app="Chat" category="bad category" outcome="drop""#,
        )
        .unwrap();
        assert!(loaded.reload_file(&config).is_err());
        assert!(loaded.error().is_some());
        assert_eq!(loaded.delivery_policy_rules(), valid_rules);
    }
}
