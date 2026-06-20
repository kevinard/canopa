use crate::{AcceptedNotification, DeliveryOutcome, Urgency};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeliveryPolicyRule {
    pub app_identity: String,
    pub urgency: Option<Urgency>,
    pub outcome: DeliveryOutcome,
    pub reference: String,
}

impl DeliveryPolicyRule {
    pub fn new(
        app_identity: impl Into<String>,
        urgency: Option<Urgency>,
        outcome: DeliveryOutcome,
        reference: impl Into<String>,
    ) -> Self {
        Self {
            app_identity: app_identity.into(),
            urgency,
            outcome,
            reference: reference.into(),
        }
    }

    fn matches(&self, notification: &AcceptedNotification) -> bool {
        self.app_identity == notification.app_identity
            && self
                .urgency
                .map(|rule_urgency| rule_urgency == notification.urgency)
                .unwrap_or(true)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoadedDeliveryPolicyState {
    do_not_disturb: bool,
    user_rules: Vec<DeliveryPolicyRule>,
    config_rules: Vec<DeliveryPolicyRule>,
}

impl LoadedDeliveryPolicyState {
    pub fn builder() -> LoadedDeliveryPolicyStateBuilder {
        LoadedDeliveryPolicyStateBuilder::default()
    }
}

#[derive(Clone, Debug, Default)]
pub struct LoadedDeliveryPolicyStateBuilder {
    do_not_disturb: bool,
    user_rules: Vec<DeliveryPolicyRule>,
    config_rules: Vec<DeliveryPolicyRule>,
}

impl LoadedDeliveryPolicyStateBuilder {
    pub fn do_not_disturb(mut self, enabled: bool) -> Self {
        self.do_not_disturb = enabled;
        self
    }

    pub fn user_rule(mut self, rule: DeliveryPolicyRule) -> Self {
        self.user_rules.push(rule);
        self
    }

    pub fn user_rules<I>(mut self, rules: I) -> Self
    where
        I: IntoIterator<Item = DeliveryPolicyRule>,
    {
        self.user_rules.extend(rules);
        self
    }

    pub fn config_rule(mut self, rule: DeliveryPolicyRule) -> Self {
        self.config_rules.push(rule);
        self
    }

    pub fn config_rules<I>(mut self, rules: I) -> Self
    where
        I: IntoIterator<Item = DeliveryPolicyRule>,
    {
        self.config_rules.extend(rules);
        self
    }

    pub fn build(self) -> LoadedDeliveryPolicyState {
        LoadedDeliveryPolicyState {
            do_not_disturb: self.do_not_disturb,
            user_rules: self.user_rules,
            config_rules: self.config_rules,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeliveryPolicy {
    state: LoadedDeliveryPolicyState,
}

impl DeliveryPolicy {
    pub fn new(state: LoadedDeliveryPolicyState) -> Self {
        Self { state }
    }

    pub fn decide(&self, notification: &AcceptedNotification) -> DeliveryDecision {
        let user_rule = self.match_user_rule(notification);
        if let Some(rule) = user_rule.filter(|rule| rule.outcome == DeliveryOutcome::Dropped) {
            return DeliveryDecision::from_rule(
                notification,
                rule,
                DeliveryDecisionReason::UserRule,
            );
        }

        if self.state.do_not_disturb && notification.urgency != Urgency::Critical {
            return DeliveryDecision::new(
                notification,
                DeliveryOutcome::Silenced,
                Some("do-not-disturb".to_string()),
                DeliveryDecisionReason::DoNotDisturb,
            );
        }

        if let Some(rule) = user_rule {
            return DeliveryDecision::from_rule(
                notification,
                rule,
                DeliveryDecisionReason::UserRule,
            );
        }

        if let Some(rule) = self
            .state
            .config_rules
            .iter()
            .find(|rule| rule.matches(notification))
        {
            return DeliveryDecision::from_rule(
                notification,
                rule,
                DeliveryDecisionReason::ConfigRule,
            );
        }

        DeliveryDecision::new(
            notification,
            DeliveryOutcome::Shown,
            None,
            DeliveryDecisionReason::DefaultAllow,
        )
    }

    fn match_user_rule(&self, notification: &AcceptedNotification) -> Option<&DeliveryPolicyRule> {
        self.state
            .user_rules
            .iter()
            .find(|rule| {
                rule.app_identity == notification.app_identity
                    && rule.urgency == Some(notification.urgency)
            })
            .or_else(|| {
                self.state.user_rules.iter().find(|rule| {
                    rule.app_identity == notification.app_identity && rule.urgency.is_none()
                })
            })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeliveryDecision {
    pub outcome: DeliveryOutcome,
    pub rule_reference: Option<String>,
    pub log: DeliveryDecisionLog,
}

impl DeliveryDecision {
    fn from_rule(
        notification: &AcceptedNotification,
        rule: &DeliveryPolicyRule,
        reason: DeliveryDecisionReason,
    ) -> Self {
        Self::new(
            notification,
            rule.outcome,
            Some(rule.reference.clone()),
            reason,
        )
    }

    fn new(
        notification: &AcceptedNotification,
        outcome: DeliveryOutcome,
        rule_reference: Option<String>,
        reason: DeliveryDecisionReason,
    ) -> Self {
        Self {
            outcome,
            rule_reference: rule_reference.clone(),
            log: DeliveryDecisionLog {
                app_identity: notification.app_identity.clone(),
                urgency: notification.urgency,
                delivery_outcome: outcome,
                rule_reference,
                reason,
            },
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeliveryDecisionLog {
    pub app_identity: String,
    pub urgency: Urgency,
    pub delivery_outcome: DeliveryOutcome,
    pub rule_reference: Option<String>,
    pub reason: DeliveryDecisionReason,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryDecisionReason {
    UserRule,
    DoNotDisturb,
    ConfigRule,
    DefaultAllow,
}
