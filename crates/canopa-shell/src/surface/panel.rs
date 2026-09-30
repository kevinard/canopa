use crate::modules::ModuleId;

pub use canopa_panel_protocol::v1::{
    ExternalPanelElement, ExternalPanelSeverity, ExternalSelectableEntry,
};
use canopa_shell::external_panel::canopa::CanopaSnapshot;

/// Prepared host content is never serialized back onto either provider wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalPanelDocument {
    Elements { elements: Vec<ExternalPanelElement> },
    Canopa(Box<CanopaSnapshot>),
}

impl From<canopa_panel_protocol::v1::ExternalPanelDocument> for ExternalPanelDocument {
    fn from(document: canopa_panel_protocol::v1::ExternalPanelDocument) -> Self {
        Self::Elements {
            elements: document.elements,
        }
    }
}

impl ExternalPanelDocument {
    pub fn elements(&self) -> &[ExternalPanelElement] {
        match self {
            Self::Elements { elements } => elements,
            Self::Canopa(_) => &[],
        }
    }

    pub fn canopa(&self) -> Option<&CanopaSnapshot> {
        match self {
            Self::Canopa(snapshot) => Some(snapshot),
            Self::Elements { .. } => None,
        }
    }

    pub fn canopa_mut(&mut self) -> Option<&mut CanopaSnapshot> {
        match self {
            Self::Canopa(snapshot) => Some(snapshot),
            Self::Elements { .. } => None,
        }
    }
}

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
    pub document: ExternalPanelDocument,
    // Search is host state, not provider content. Never send it on the wire.
    list_search: std::collections::HashMap<String, String>,
}

impl ExternalPanelContent {
    pub fn failure(provider_name: impl Into<String>, message: impl Into<String>) -> Self {
        Self::from_provider_document(
            provider_name,
            ExternalPanelDocument::Elements {
                elements: vec![ExternalPanelElement::Status {
                    title: "Provider failed".into(),
                    message: message.into(),
                    severity: ExternalPanelSeverity::Error,
                }],
            },
        )
    }

    pub fn elements(&self) -> &[ExternalPanelElement] {
        self.document.elements()
    }

    #[cfg(test)]
    pub fn failure_text(&self) -> String {
        if let Some(snapshot) = self.document.canopa() {
            return snapshot.content.title.clone();
        }
        self.elements()
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
            document,
            list_search: Default::default(),
        }
    }

    pub(crate) fn list_search(&self, list_id: &str) -> &str {
        self.list_search
            .get(list_id)
            .map(String::as_str)
            .unwrap_or("")
    }

    pub(crate) fn with_list_search(&self, list_id: &str, query: String) -> Self {
        let mut next = self.clone();
        if next.elements().iter().any(|element| {
            matches!(element,
                ExternalPanelElement::SelectableList { id, .. } if id == list_id
            )
        }) {
            next.list_search.insert(list_id.to_owned(), query);
        }
        next
    }

    pub(crate) fn with_text_input_value(&self, element_id: &str, value: String) -> Self {
        let mut next = self.clone();
        if let ExternalPanelDocument::Elements { elements } = &mut next.document {
            for element in elements {
                if let ExternalPanelElement::TextInput {
                    id, value: current, ..
                } = element
                    && id == element_id
                {
                    *current = value.clone();
                }
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

/// The same host-owned heading and dismiss control in both panel presentations.
pub fn panel_header<'a>(title: String) -> iced::Element<'a, crate::app::Message> {
    use iced::widget::{row, text};

    row![
        text(title)
            .size(canopa_ui::metrics::TITLE)
            .width(iced::Length::Fill),
        canopa_ui::action(
            text("Close").size(canopa_ui::metrics::LABEL),
            Some(crate::app::Message::Panel(
                crate::app::messages::PanelEvent::Close
            )),
            canopa_ui::Action::Quiet,
            false,
        )
        .height(canopa_ui::panel::HEADER_CONTROL_HEIGHT),
    ]
    .spacing(canopa_ui::metrics::GAP)
    .align_y(iced::Alignment::Center)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_panel_document_rejects_empty_interactive_labels() {
        let document = canopa_panel_protocol::v1::ExternalPanelDocument {
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
