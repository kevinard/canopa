use crate::preferences::{
    DoNotDisturbStatus, DoNotDisturbTimer, PreferenceLayers, SuppressionState, evaluate,
};
use crate::{AcceptedNotification, DeliveryOutcome, Urgency, UserRule};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeliveryPolicyRule {
    pub app_identity: String,
    pub category: Option<String>,
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
            category: None,
            urgency,
            outcome,
            reference: reference.into(),
        }
    }

    pub fn for_category(
        app_identity: impl Into<String>,
        category: impl Into<String>,
        urgency: Option<Urgency>,
        outcome: DeliveryOutcome,
        reference: impl Into<String>,
    ) -> Self {
        Self {
            app_identity: app_identity.into(),
            category: Some(category.into()),
            urgency,
            outcome,
            reference: reference.into(),
        }
    }

    fn matches(&self, notification: &AcceptedNotification) -> bool {
        self.app_identity == notification.app_identity
            && self
                .category
                .as_ref()
                .map(|category| notification.category.as_ref() == Some(category))
                .unwrap_or(true)
            && self
                .urgency
                .map(|rule_urgency| rule_urgency == notification.urgency)
                .unwrap_or(true)
    }
}

impl From<UserRule> for DeliveryPolicyRule {
    fn from(rule: UserRule) -> Self {
        Self {
            app_identity: rule.app_identity,
            category: rule.category,
            urgency: rule.urgency,
            outcome: rule.outcome,
            reference: rule.reference,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoadedDeliveryPolicyState {
    dnd_timer: DoNotDisturbTimer,
    user_rules: Vec<DeliveryPolicyRule>,
    config_rules: Vec<DeliveryPolicyRule>,
    preferences: PreferenceLayers,
    suppression: SuppressionState,
}

impl LoadedDeliveryPolicyState {
    pub fn builder() -> LoadedDeliveryPolicyStateBuilder {
        LoadedDeliveryPolicyStateBuilder::default()
    }

    pub(crate) fn do_not_disturb_status<Tz: chrono::TimeZone>(
        &self,
        now: &chrono::DateTime<Tz>,
    ) -> DoNotDisturbStatus {
        evaluate(
            &self.dnd_timer,
            self.preferences.dnd().schedule.as_ref(),
            now,
        )
    }

    pub(crate) fn dnd_timer(&self) -> DoNotDisturbTimer {
        self.dnd_timer
    }

    pub(crate) fn preferences(&self) -> &PreferenceLayers {
        &self.preferences
    }

    pub(crate) fn suppression(&self) -> SuppressionState {
        self.suppression
    }

    /// The automatic suppression in effect, honouring the preferences.
    pub(crate) fn active_suppression(&self) -> Option<crate::Suppression> {
        let preferences = self.preferences.dnd();
        if self.suppression.screen_sharing && preferences.suppress_screen_sharing {
            Some(crate::Suppression::ScreenSharing)
        } else if self.suppression.fullscreen && preferences.suppress_fullscreen {
            Some(crate::Suppression::Fullscreen)
        } else {
            None
        }
    }

    pub(crate) fn with_dnd_timer(mut self, timer: DoNotDisturbTimer) -> Self {
        self.dnd_timer = timer;
        self
    }

    pub(crate) fn with_preferences(mut self, preferences: PreferenceLayers) -> Self {
        self.preferences = preferences;
        self
    }

    pub(crate) fn with_suppression(mut self, suppression: SuppressionState) -> Self {
        self.suppression = suppression;
        self
    }

    pub(crate) fn user_rules(&self) -> &[DeliveryPolicyRule] {
        &self.user_rules
    }

    pub(crate) fn with_user_rule(mut self, rule: DeliveryPolicyRule) -> Self {
        match self.user_rules.iter_mut().find(|existing| {
            existing.app_identity == rule.app_identity
                && existing.category == rule.category
                && existing.urgency == rule.urgency
        }) {
            Some(existing) => *existing = rule,
            None => self.user_rules.push(rule),
        }
        self.sort_user_rules();
        self
    }

    pub(crate) fn without_user_rule(
        mut self,
        app_identity: &str,
        category: Option<&str>,
        urgency: Option<Urgency>,
    ) -> Self {
        self.user_rules.retain(|rule| {
            rule.app_identity != app_identity
                || rule.category.as_deref() != category
                || rule.urgency != urgency
        });
        self
    }

    pub(crate) fn with_config_rules(mut self, rules: Vec<DeliveryPolicyRule>) -> Self {
        self.config_rules = rules;
        self
    }

    fn sort_user_rules(&mut self) {
        self.user_rules.sort_by(|left, right| {
            left.app_identity
                .cmp(&right.app_identity)
                .then_with(|| left.category.cmp(&right.category))
                .then_with(|| urgency_sort_key(left.urgency).cmp(urgency_sort_key(right.urgency)))
        });
    }
}

fn urgency_sort_key(urgency: Option<Urgency>) -> &'static str {
    urgency.map(Urgency::as_str).unwrap_or("*")
}

#[derive(Clone, Debug, Default)]
pub struct LoadedDeliveryPolicyStateBuilder {
    do_not_disturb: bool,
    user_rules: Vec<DeliveryPolicyRule>,
    config_rules: Vec<DeliveryPolicyRule>,
    preferences: PreferenceLayers,
    suppression: SuppressionState,
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

    /// UI-managed Do Not Disturb preferences (schedule, break-through, suppression).
    pub fn do_not_disturb_preferences(
        mut self,
        preferences: crate::DoNotDisturbPreferences,
    ) -> Self {
        self.preferences.user_dnd = Some(preferences);
        self
    }

    pub fn suppression(mut self, suppression: SuppressionState) -> Self {
        self.suppression = suppression;
        self
    }

    pub fn build(self) -> LoadedDeliveryPolicyState {
        LoadedDeliveryPolicyState {
            dnd_timer: DoNotDisturbTimer {
                manual: self.do_not_disturb,
                ..Default::default()
            },
            user_rules: self.user_rules,
            config_rules: self.config_rules,
            preferences: self.preferences,
            suppression: self.suppression,
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
        self.decide_at(notification, &chrono::Local::now())
    }

    /// Decide at an explicit instant: explicit drop rules first, then Do Not
    /// Disturb and automatic suppression (critical and break-through categories
    /// pass), then user, configuration and default rules.
    pub fn decide_at<Tz: chrono::TimeZone>(
        &self,
        notification: &AcceptedNotification,
        now: &chrono::DateTime<Tz>,
    ) -> DeliveryDecision {
        let user_rule = self.match_user_rule(notification);
        if let Some(rule) = user_rule.filter(|rule| rule.outcome == DeliveryOutcome::Dropped) {
            return DeliveryDecision::from_rule(
                notification,
                rule,
                DeliveryDecisionReason::UserRule,
            );
        }

        let preferences = self.state.preferences.dnd();
        let breaks_through = notification.urgency == Urgency::Critical
            || preferences
                .break_through
                .matches(notification.category.as_deref());
        if !breaks_through {
            if self.state.do_not_disturb_status(now).active {
                return DeliveryDecision::new(
                    notification,
                    DeliveryOutcome::Silenced,
                    Some("do-not-disturb".to_string()),
                    DeliveryDecisionReason::DoNotDisturb,
                );
            }
            match self.state.active_suppression() {
                Some(crate::Suppression::ScreenSharing) => {
                    return DeliveryDecision::new(
                        notification,
                        DeliveryOutcome::Silenced,
                        Some("screen-sharing".to_string()),
                        DeliveryDecisionReason::ScreenSharing,
                    );
                }
                Some(crate::Suppression::Fullscreen) => {
                    return DeliveryDecision::new(
                        notification,
                        DeliveryOutcome::Silenced,
                        Some("fullscreen".to_string()),
                        DeliveryDecisionReason::Fullscreen,
                    );
                }
                None => {}
            }
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
        let category = notification.category.as_deref();
        [
            (category, Some(notification.urgency)),
            (category, None),
            (None, Some(notification.urgency)),
            (None, None),
        ]
        .into_iter()
        .filter(|(rule_category, _)| category.is_some() || rule_category.is_none())
        .find_map(|(rule_category, rule_urgency)| {
            self.state.user_rules.iter().find(|rule| {
                rule.app_identity == notification.app_identity
                    && rule.category.as_deref() == rule_category
                    && rule.urgency == rule_urgency
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
                category: notification.category.clone(),
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
    pub category: Option<String>,
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
    /// An application was fullscreen on the focused output.
    Fullscreen,
    /// The screen was being shared or recorded.
    ScreenSharing,
    /// A portal `tray` display hint asked for Center-only delivery.
    DisplayHint,
}
