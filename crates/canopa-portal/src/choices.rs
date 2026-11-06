//! Serialized controls shared by FileChooser and Access dialogs.
use crate::requests::{Options, option};
use zbus::fdo;

pub type Choice = (String, String, Vec<(String, String)>, String);

pub(crate) fn parse(options: &mut Options) -> fdo::Result<Vec<Choice>> {
    let mut choices: Vec<Choice> = option(options, "choices")?.unwrap_or_default();
    let mut ids = std::collections::HashSet::new();
    for (id, label, values, selected) in &mut choices {
        let mut value_ids = std::collections::HashSet::new();
        if id.is_empty()
            || label.is_empty()
            || !ids.insert(id.clone())
            || values.iter().any(|(id, label)| {
                id.is_empty() || label.is_empty() || !value_ids.insert(id.clone())
            })
        {
            return Err(fdo::Error::InvalidArgs("Invalid choices".into()));
        }
        if selected.is_empty() {
            *selected = values.first().map_or("false", |v| v.0.as_str()).to_owned();
        }
        if (values.is_empty() && !matches!(selected.as_str(), "true" | "false"))
            || (!values.is_empty() && !values.iter().any(|v| &v.0 == selected))
        {
            return Err(fdo::Error::InvalidArgs("Invalid choice selection".into()));
        }
    }
    Ok(choices)
}

pub(crate) fn valid(choices: &[Choice], selected: &[(String, String)]) -> bool {
    selected.len() == choices.len()
        && choices.iter().all(|(id, _, values, _)| {
            selected.iter().any(|(chosen_id, value)| {
                chosen_id == id
                    && if values.is_empty() {
                        matches!(value.as_str(), "true" | "false")
                    } else {
                        values.iter().any(|v| &v.0 == value)
                    }
            })
        })
}
