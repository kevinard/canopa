use crate::{
    NotificationInput, NotificationServerRequest, NotificationSound, NotificationVisualReference,
};

pub(crate) const MAX_BODY_BYTES: usize = 64 * 1024;
const MAX_SUMMARY_BYTES: usize = 8 * 1024;
const MAX_FIELD_BYTES: usize = 4 * 1024;
const MAX_TEXT_BYTES: usize = 128 * 1024;
const MAX_ACTIONS: usize = 64;
const MAX_HINTS: usize = 64;

/// A request rejected before receipt, with no sender-controlled content in the error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotificationLimitExceeded(pub(crate) &'static str);

impl std::fmt::Display for NotificationLimitExceeded {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "notification {} limit exceeded", self.0)
    }
}

impl std::error::Error for NotificationLimitExceeded {}

pub(crate) fn validate_server_request(
    request: &NotificationServerRequest,
) -> Result<(), NotificationLimitExceeded> {
    check_limit("actions", request.actions.len(), MAX_ACTIONS * 2)?;
    check_limit("hints", request.hints.len(), MAX_HINTS)?;
    let mut budget = TextBudget::new(&request.app_name, &request.summary, &request.body)?;
    budget.field(&request.app_icon)?;
    for action in &request.actions {
        budget.field(action)?;
    }
    for (key, value) in &request.hints {
        budget.field(key)?;
        if let zbus::zvariant::Value::Str(value) = &**value {
            budget.field(value.as_str())?;
        }
    }
    Ok(())
}

pub(crate) fn validate_input(input: &NotificationInput) -> Result<(), NotificationLimitExceeded> {
    check_limit("actions", input.actions.len(), MAX_ACTIONS)?;
    let mut budget = TextBudget::new(&input.app_name, &input.summary, &input.body)?;
    for value in [&input.desktop_entry, &input.category]
        .into_iter()
        .flatten()
    {
        budget.field(value)?;
    }
    for visual in [&input.app_icon, &input.image_reference]
        .into_iter()
        .flatten()
    {
        budget.visual(visual)?;
    }
    if let Some(NotificationSound::Name(value) | NotificationSound::File(value)) = &input.sound {
        budget.field(value)?;
    }
    for action in &input.actions {
        budget.field(&action.key)?;
        budget.field(&action.label)?;
        if let Some(icon) = &action.icon {
            budget.visual(icon)?;
        }
    }
    Ok(())
}

fn check_limit(
    field: &'static str,
    size: usize,
    maximum: usize,
) -> Result<(), NotificationLimitExceeded> {
    if size > maximum {
        return Err(NotificationLimitExceeded(field));
    }
    Ok(())
}

struct TextBudget {
    used: usize,
}

impl TextBudget {
    fn new(app_name: &str, summary: &str, body: &str) -> Result<Self, NotificationLimitExceeded> {
        let mut budget = Self { used: 0 };
        budget.add("body", body, MAX_BODY_BYTES)?;
        budget.add("summary", summary, MAX_SUMMARY_BYTES)?;
        budget.field(app_name)?;
        Ok(budget)
    }

    fn field(&mut self, value: &str) -> Result<(), NotificationLimitExceeded> {
        self.add("text field", value, MAX_FIELD_BYTES)
    }

    fn visual(
        &mut self,
        value: &NotificationVisualReference,
    ) -> Result<(), NotificationLimitExceeded> {
        let (NotificationVisualReference::IconName(value)
        | NotificationVisualReference::LocalPath(value)) = value;
        self.field(value)
    }

    fn add(
        &mut self,
        field: &'static str,
        value: &str,
        maximum: usize,
    ) -> Result<(), NotificationLimitExceeded> {
        check_limit(field, value.len(), maximum)?;
        // Each field is bounded before addition; used never exceeds MAX_TEXT_BYTES.
        self.used += value.len();
        check_limit("aggregate text", self.used, MAX_TEXT_BYTES)
    }
}
