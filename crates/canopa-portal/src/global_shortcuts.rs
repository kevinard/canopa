//! Shortcut definitions, compositor authority and the public session owner.
//! Preferred triggers are suggestions; only explicit user choices are committed.
use crate::requests::{Options, option};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use zbus::{fdo, zvariant::Value};

pub mod editor;
pub mod history;
mod portal;
mod session;
pub mod transport;
pub use portal::{Connector, GlobalShortcuts, Niri};

pub const INTERFACE: &str = "org.freedesktop.impl.portal.GlobalShortcuts";
pub const MAX_ACTIONS: usize = 64;
const TEXT_BUDGET: usize = 64 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub id: String,
    pub description: String,
    pub preferred_trigger: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub id: String,
    pub trigger: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Actions(pub Vec<Action>);

fn invalid(message: &str) -> fdo::Error {
    fdo::Error::InvalidArgs(message.into())
}
fn field(value: &str, limit: usize, budget: &mut usize) -> fdo::Result<()> {
    *budget = budget.saturating_add(value.len());
    if value.len() > limit || value.contains('\0') || *budget > TEXT_BUDGET {
        return Err(invalid("Shortcut text exceeds limits"));
    }
    Ok(())
}
impl Actions {
    pub fn parse(values: Vec<(String, Options)>) -> fdo::Result<Self> {
        if values.len() > MAX_ACTIONS {
            return Err(invalid("Too many shortcuts"));
        }
        let mut budget = 0;
        let mut ids = HashSet::new();
        let mut actions = Vec::with_capacity(values.len());
        for (id, mut properties) in values {
            field(&id, 256, &mut budget)?;
            if id.is_empty() || id.chars().any(char::is_control) || !ids.insert(id.clone()) {
                return Err(invalid("Invalid or repeated shortcut ID"));
            }
            if properties.len() > 16 || properties.keys().any(|key| key.len() > 128) {
                return Err(invalid("Shortcut metadata exceeds limits"));
            }
            let description: String = option(&mut properties, "description")?
                .ok_or_else(|| invalid("Missing shortcut description"))?;
            let preferred_trigger: Option<String> = option(&mut properties, "preferred_trigger")?;
            field(&description, 1024, &mut budget)?;
            if let Some(trigger) = &preferred_trigger {
                field(trigger, 256, &mut budget)?;
            }
            actions.push(Action {
                id,
                description,
                preferred_trigger,
            });
        }
        Ok(Self(actions))
    }
    /// IDs used on the private lease are local to this immutable action set.
    /// The application never chooses numeric registration IDs or generations.
    pub fn definitions(&self, selected: &[Binding]) -> fdo::Result<Vec<(u32, String)>> {
        if self.0.len() > MAX_ACTIONS || selected.len() > self.0.len() {
            return Err(invalid("Invalid shortcut selection"));
        }
        let mut ids = HashSet::new();
        let mut budget = self
            .0
            .iter()
            .map(|action| action.id.len() + action.description.len())
            .sum();
        selected
            .iter()
            .map(|binding| {
                if !ids.insert(&binding.id)
                    || binding.trigger.is_empty()
                    || binding.trigger.chars().any(char::is_control)
                {
                    return Err(invalid("Invalid shortcut selection"));
                }
                field(&binding.trigger, 256, &mut budget)?;
                let index = self
                    .0
                    .iter()
                    .position(|action| action.id == binding.id)
                    .ok_or_else(|| invalid("Unknown shortcut selection"))?;
                Ok((index as u32 + 1, binding.trigger.clone()))
            })
            .collect()
    }
    pub fn selected(&self, definitions: &BTreeMap<u32, String>) -> fdo::Result<Vec<Binding>> {
        definitions
            .iter()
            .map(|(&id, trigger)| {
                let action = id
                    .checked_sub(1)
                    .and_then(|index| self.0.get(index as usize))
                    .ok_or_else(|| invalid("Unknown compositor shortcut"))?;
                Ok(Binding {
                    id: action.id.clone(),
                    trigger: trigger.clone(),
                })
            })
            .collect()
    }
    pub fn response(&self, definitions: &BTreeMap<u32, String>) -> fdo::Result<Options> {
        let rows: Vec<_> = self
            .selected(definitions)?
            .into_iter()
            .map(|binding| {
                let action = self
                    .0
                    .iter()
                    .find(|a| a.id == binding.id)
                    .expect("validated action");
                (
                    binding.id,
                    Options::from([
                        (
                            "description".into(),
                            Value::from(action.description.clone())
                                .try_into()
                                .expect("owned text"),
                        ),
                        (
                            "trigger_description".into(),
                            Value::from(trigger_description(&binding.trigger))
                                .try_into()
                                .expect("owned text"),
                        ),
                    ]),
                )
            })
            .collect();
        Ok(Options::from([(
            "shortcuts".into(),
            Value::from(rows)
                .try_into()
                .map_err(|_| invalid("Invalid shortcut results"))?,
        )]))
    }
}
pub fn trigger_description(trigger: &str) -> String {
    trigger
        .split('+')
        .map(|part| match part {
            "CTRL" => "Ctrl".into(),
            "ALT" => "Alt".into(),
            "SHIFT" => "Shift".into(),
            "NUM" => "Num Lock".into(),
            "LOGO" => "Super".into(),
            "space" => "Space".into(),
            "Return" => "Enter".into(),
            "Control_L" => "Left Ctrl".into(),
            "Control_R" => "Right Ctrl".into(),
            "Shift_L" => "Left Shift".into(),
            "Shift_R" => "Right Shift".into(),
            _ if part.len() == 1 && part.is_ascii() => part.to_ascii_uppercase(),
            _ => part.replace('_', " "),
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn action(id: &str, preferred: Option<&str>) -> (String, Options) {
        let mut props = Options::from([(
            "description".into(),
            Value::from("An action").try_into().unwrap(),
        )]);
        if let Some(value) = preferred {
            props.insert(
                "preferred_trigger".into(),
                Value::from(value).try_into().unwrap(),
            );
        }
        (id.into(), props)
    }
    #[test]
    fn preferences_do_not_install_authority_and_selected_ids_keep_application_identity() {
        let actions =
            Actions::parse(vec![action("open", Some("CTRL+a")), action("close", None)]).unwrap();
        assert!(actions.definitions(&[]).unwrap().is_empty());
        let definitions = actions
            .definitions(&[Binding {
                id: "close".into(),
                trigger: "F9".into(),
            }])
            .unwrap();
        assert_eq!(definitions, [(2, "F9".into())]);
        assert_eq!(
            actions
                .selected(&definitions.into_iter().collect())
                .unwrap()[0]
                .id,
            "close"
        );
        assert!(
            actions
                .definitions(&[Binding {
                    id: "foreign".into(),
                    trigger: "F9".into()
                }])
                .is_err()
        );
    }
    #[test]
    fn duplicate_ids_and_wrong_property_types_are_rejected_before_presentation() {
        assert!(Actions::parse(vec![action("a", None), action("a", None)]).is_err());
        assert!(Actions::parse(vec![("a".into(), Options::new())]).is_err());
        assert!(
            Actions::parse(vec![(
                "a".into(),
                Options::from([("description".into(), 7u32.into())])
            )])
            .is_err()
        );
    }
    #[test]
    fn result_contains_descriptions_for_only_the_actual_accepted_set() {
        let actions = Actions::parse(vec![action("one", None), action("two", None)]).unwrap();
        let result = actions
            .response(&BTreeMap::from([(2, "CTRL+ALT+Return".into())]))
            .unwrap();
        let rows =
            Vec::<(String, Options)>::try_from(result.into_iter().next().unwrap().1).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "two");
        assert_eq!(
            String::try_from(rows[0].1["trigger_description"].try_clone().unwrap()).unwrap(),
            "Ctrl + Alt + Enter"
        );
        assert!(
            actions
                .response(&BTreeMap::from([(3, "F9".into())]))
                .is_err()
        );
    }
}
