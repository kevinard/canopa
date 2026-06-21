use crate::{DeliveryOutcome, DeliveryPolicyRule, Urgency};
use anyhow::Context;
use kdl::{KdlDocument, KdlEntry, KdlNode};
use std::path::Path;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct LoadedNotificationConfiguration {
    active: NotificationConfiguration,
    last_error: Option<String>,
}

impl LoadedNotificationConfiguration {
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    pub(crate) fn reload_file(&mut self, path: &Path) -> anyhow::Result<()> {
        match NotificationConfiguration::load_file(path) {
            Ok(configuration) => {
                self.active = configuration;
                self.last_error = None;
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

    pub(crate) fn error(&self) -> Option<String> {
        self.last_error.clone()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct NotificationConfiguration {
    rules: Vec<NotificationConfigurationRule>,
}

impl NotificationConfiguration {
    fn load_file(path: &Path) -> anyhow::Result<Self> {
        let source = std::fs::read_to_string(path).with_context(|| {
            format!(
                "failed to read Notification Configuration {}",
                path.display()
            )
        })?;
        Self::parse(&source).with_context(|| {
            format!(
                "failed to parse Notification Configuration {}",
                path.display()
            )
        })
    }

    fn parse(source: &str) -> anyhow::Result<Self> {
        let doc: KdlDocument = source
            .parse()
            .context("failed to parse Notification Configuration as KDL")?;
        let mut rules = Vec::new();

        for node in doc.nodes() {
            if node.name().value() != "rule" {
                anyhow::bail!(
                    "invalid Notification Configuration node {:?}: expected rule",
                    node.name().value()
                );
            }

            rules.push(parse_rule(node, rules.len() + 1)?);
        }

        Ok(Self { rules })
    }

    fn delivery_policy_rules(&self) -> Vec<DeliveryPolicyRule> {
        self.rules
            .iter()
            .map(|rule| {
                DeliveryPolicyRule::new(
                    rule.app_identity.clone(),
                    rule.urgency,
                    rule.outcome,
                    rule.reference.clone(),
                )
            })
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NotificationConfigurationRule {
    app_identity: String,
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
    let outcome = parse_outcome(&required_property(entries.outcome, "outcome", rule_number)?)?;
    let urgency = entries
        .urgency
        .map(|value| parse_urgency(&value))
        .transpose()?;
    let reference = entries
        .rule_id
        .unwrap_or_else(|| format!("config-rule-{rule_number}"));

    Ok(NotificationConfigurationRule {
        app_identity,
        urgency,
        outcome,
        reference,
    })
}

#[derive(Default)]
struct RuleEntries {
    rule_id: Option<String>,
    app_identity: Option<String>,
    outcome: Option<String>,
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
                    set_string_property(&mut entries.app_identity, entry, "app", rule_number)?
                }
                Some("outcome") => {
                    set_string_property(&mut entries.outcome, entry, "outcome", rule_number)?
                }
                Some("urgency") => {
                    set_string_property(&mut entries.urgency, entry, "urgency", rule_number)?
                }
                Some(name) => anyhow::bail!("rule {rule_number} has unknown property {name:?}"),
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

fn set_string_property(
    slot: &mut Option<String>,
    entry: &KdlEntry,
    name: &str,
    rule_number: usize,
) -> anyhow::Result<()> {
    if slot.is_some() {
        anyhow::bail!("rule {rule_number} has duplicate {name} properties");
    }
    let Some(value) = entry.value().as_string() else {
        anyhow::bail!("rule {rule_number} property {name} must be a string");
    };
    *slot = Some(value.to_string());
    Ok(())
}

fn required_property(
    value: Option<String>,
    name: &str,
    rule_number: usize,
) -> anyhow::Result<String> {
    value.ok_or_else(|| anyhow::anyhow!("rule {rule_number} is missing {name}"))
}

fn parse_outcome(value: &str) -> anyhow::Result<DeliveryOutcome> {
    match value {
        "allow" | "shown" => Ok(DeliveryOutcome::Shown),
        "silence" | "silenced" => Ok(DeliveryOutcome::Silenced),
        "drop" | "dropped" => Ok(DeliveryOutcome::Dropped),
        _ => anyhow::bail!("unknown delivery outcome {value:?}"),
    }
}

fn parse_urgency(value: &str) -> anyhow::Result<Urgency> {
    match value {
        "low" => Ok(Urgency::Low),
        "normal" => Ok(Urgency::Normal),
        "critical" => Ok(Urgency::Critical),
        _ => anyhow::bail!("unknown notification urgency {value:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_kdl_notification_rules_into_delivery_policy_rules() {
        let configuration = NotificationConfiguration::parse(
            r#"
            rule "quiet-chat" app="Chat" outcome="silenced"
            rule app="Build" urgency="critical" outcome="drop"
            "#,
        )
        .unwrap();

        let rules = configuration.delivery_policy_rules();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].app_identity, "Chat");
        assert_eq!(rules[0].outcome, DeliveryOutcome::Silenced);
        assert_eq!(rules[0].reference, "quiet-chat");
        assert_eq!(rules[1].app_identity, "Build");
        assert_eq!(rules[1].urgency, Some(Urgency::Critical));
        assert_eq!(rules[1].outcome, DeliveryOutcome::Dropped);
        assert_eq!(rules[1].reference, "config-rule-2");
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
    }

    #[test]
    fn invalid_reload_keeps_previous_valid_configuration() {
        let mut loaded = LoadedNotificationConfiguration::empty();
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("notify.kdl");

        std::fs::write(&config, r#"rule "drop-chat" app="Chat" outcome="dropped""#).unwrap();
        loaded.reload_file(&config).unwrap();
        let valid_rules = loaded.delivery_policy_rules();

        std::fs::write(&config, "not-a-rule").unwrap();
        assert!(loaded.reload_file(&config).is_err());
        assert!(loaded.error().is_some());
        assert_eq!(loaded.delivery_policy_rules(), valid_rules);
    }
}
