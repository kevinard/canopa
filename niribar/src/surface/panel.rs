use crate::modules::ModuleId;

pub use niribar::external_panel::protocol::{
    ExternalPanelDocument, ExternalPanelElement, ExternalPanelSeverity, ExternalSelectableEntry,
};

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
                ExternalPanelElement::Canopa { snapshot } => snapshot.content.title.as_str(),
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
                Some(iced::Background::Color(crate::theme::palette().background))
            } else {
                Some(iced::Background::Color(crate::theme::palette().surface))
            },
            border: iced::Border {
                color: if focused {
                    crate::theme::palette().accent
                } else {
                    crate::theme::palette().border
                },
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
