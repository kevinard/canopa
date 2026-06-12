use crate::modules::ModuleId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PanelOrigin {
    Bar,
    Module(ModuleId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PanelContent {
    BarPanel,
    ModulePanel(ModuleId),
    Menu(String),
    DmenuRequest,
    ExternalProvider(ExternalPanelContent),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalPanelContent {
    pub provider_name: String,
    pub elements: Vec<ExternalPanelElement>,
}

impl ExternalPanelContent {
    pub fn failure(provider_name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            provider_name: provider_name.into(),
            elements: vec![ExternalPanelElement::Status {
                title: "Provider failed".into(),
                message: message.into(),
                severity: ExternalPanelSeverity::Error,
            }],
        }
    }

    #[cfg(test)]
    pub fn failure_text(&self) -> String {
        self.elements
            .iter()
            .map(|element| match element {
                ExternalPanelElement::Text { text } => text.as_str(),
                ExternalPanelElement::Button { label, .. } => label.as_str(),
                ExternalPanelElement::SelectableList { id, .. } => id.as_str(),
                ExternalPanelElement::TextInput { label, .. } => label.as_str(),
                ExternalPanelElement::Status { message, .. } => message.as_str(),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub(crate) fn from_provider_document(
        provider_name: impl Into<String>,
        document: ExternalPanelDocument,
    ) -> Self {
        Self {
            provider_name: provider_name.into(),
            elements: document.elements,
        }
    }

    pub(crate) fn with_list_search(&self, list_id: &str, query: String) -> Self {
        let mut next = self.clone();
        for element in &mut next.elements {
            if let ExternalPanelElement::SelectableList {
                id, search_query, ..
            } = element
                && id == list_id
            {
                *search_query = query.clone();
            }
        }
        next
    }

    pub(crate) fn with_text_input_value(&self, element_id: &str, value: String) -> Self {
        let mut next = self.clone();
        for element in &mut next.elements {
            if let ExternalPanelElement::TextInput {
                id, value: current, ..
            } = element
                && id == element_id
            {
                *current = value.clone();
            }
        }
        next
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExternalPanelDocument {
    pub elements: Vec<ExternalPanelElement>,
}

impl ExternalPanelDocument {
    pub(crate) fn validate(&self) -> Result<(), ExternalPanelValidationError> {
        let mut ids = std::collections::HashSet::new();
        for element in &self.elements {
            if let Some(id) = element.interactive_id() {
                if id.is_empty() {
                    return Err(ExternalPanelValidationError(
                        "interactive element id must not be empty".into(),
                    ));
                }
                if !ids.insert(id) {
                    return Err(ExternalPanelValidationError(format!(
                        "duplicate external panel element id {id:?}"
                    )));
                }
            }
            if let ExternalPanelElement::SelectableList { entries, .. } = element {
                validate_selectable_entries(entries)?;
            }
            validate_element_definition(element)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExternalPanelValidationError(String);

impl std::fmt::Display for ExternalPanelValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ExternalPanelValidationError {}

fn validate_selectable_entries(
    entries: &[ExternalSelectableEntry],
) -> Result<(), ExternalPanelValidationError> {
    let mut ids = std::collections::HashSet::new();
    for entry in entries {
        if entry.id.is_empty() {
            return Err(ExternalPanelValidationError(
                "external selectable entry id must not be empty".into(),
            ));
        }
        if entry.label.is_empty() {
            return Err(ExternalPanelValidationError(
                "external selectable entry label must not be empty".into(),
            ));
        }
        if !ids.insert(entry.id.as_str()) {
            return Err(ExternalPanelValidationError(format!(
                "duplicate external selectable entry id {:?}",
                entry.id
            )));
        }
    }
    Ok(())
}

fn validate_element_definition(
    element: &ExternalPanelElement,
) -> Result<(), ExternalPanelValidationError> {
    match element {
        ExternalPanelElement::Text { .. } | ExternalPanelElement::SelectableList { .. } => Ok(()),
        ExternalPanelElement::Button { label, .. } if label.is_empty() => Err(
            ExternalPanelValidationError("external button label must not be empty".into()),
        ),
        ExternalPanelElement::TextInput { label, .. } if label.is_empty() => Err(
            ExternalPanelValidationError("external text input label must not be empty".into()),
        ),
        ExternalPanelElement::Status { title, message, .. }
            if title.is_empty() || message.is_empty() =>
        {
            Err(ExternalPanelValidationError(
                "external status title and message must not be empty".into(),
            ))
        }
        ExternalPanelElement::Button { .. }
        | ExternalPanelElement::TextInput { .. }
        | ExternalPanelElement::Status { .. } => Ok(()),
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ExternalPanelElement {
    Text {
        text: String,
    },
    Button {
        id: String,
        label: String,
    },
    SelectableList {
        id: String,
        entries: Vec<ExternalSelectableEntry>,
        #[serde(default, skip)]
        search_query: String,
    },
    TextInput {
        id: String,
        label: String,
        value: String,
        #[serde(default)]
        secret: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    Status {
        title: String,
        message: String,
        severity: ExternalPanelSeverity,
    },
}

impl ExternalPanelElement {
    fn interactive_id(&self) -> Option<&str> {
        match self {
            ExternalPanelElement::Button { id, .. }
            | ExternalPanelElement::SelectableList { id, .. }
            | ExternalPanelElement::TextInput { id, .. } => Some(id.as_str()),
            ExternalPanelElement::Text { .. } | ExternalPanelElement::Status { .. } => None,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExternalSelectableEntry {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub active: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ExternalPanelSeverity {
    Info,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelModuleEntry {
    pub id: ModuleId,
    pub zone: crate::modules::Zone,
    pub focused: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelPreferredSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PanelTree {
    pub origin: PanelOrigin,
    pub content: PanelContent,
    pub origin_window: iced::window::Id,
    pub origin_bar_name: String,
    pub origin_output_name: String,
    pub anchor: Option<iced::Rectangle>,
    pub screen_size: (u32, u32),
}

pub fn panel_module_frame<'a>(
    content: iced::Element<'a, crate::app::Message>,
    focused: bool,
) -> iced::Element<'a, crate::app::Message> {
    use iced::widget::container;

    container(content)
        .padding([8, 10])
        .width(iced::Length::Fill)
        .style(move |_| iced::widget::container::Style {
            background: if focused {
                Some(iced::Background::Color(crate::theme::SURFACE0))
            } else {
                Some(iced::Background::Color(crate::theme::SURFACE1))
            },
            border: iced::Border {
                color: crate::theme::BORDER,
                width: 1.0,
                radius: iced::border::Radius::from(4.0),
            },
            ..Default::default()
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_panel_document_rejects_empty_interactive_labels() {
        let document = ExternalPanelDocument {
            elements: vec![ExternalPanelElement::Button {
                id: "scan".into(),
                label: String::new(),
            }],
        };

        let err = document.validate().unwrap_err();

        assert!(err.to_string().contains("label"));
    }

    #[test]
    fn panel_tree_stores_origin_metadata() {
        let window = iced::window::Id::unique();
        let tree = PanelTree {
            origin: PanelOrigin::Module(ModuleId::new("clock")),
            content: PanelContent::ModulePanel(ModuleId::new("clock")),
            origin_window: window,
            origin_bar_name: "main".into(),
            origin_output_name: "DP-1".into(),
            anchor: Some(iced::Rectangle {
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0,
            }),
            screen_size: (1920, 1080),
        };

        assert_eq!(tree.origin_window, window);
        assert_eq!(tree.origin_bar_name, "main");
        assert_eq!(tree.origin_output_name, "DP-1");
        assert_eq!(tree.screen_size, (1920, 1080));
    }
}
