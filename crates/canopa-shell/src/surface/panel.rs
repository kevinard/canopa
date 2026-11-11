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
    input_errors: std::collections::HashMap<String, String>,
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
            input_errors: Default::default(),
        }
    }

    pub(crate) fn list_search(&self, list_id: &str) -> &str {
        self.list_search
            .get(list_id)
            .map(String::as_str)
            .unwrap_or("")
    }

    pub(crate) fn input_error(&self, element_id: &str) -> Option<&str> {
        self.input_errors.get(element_id).map(String::as_str)
    }

    pub(crate) fn set_input_error(&mut self, element_id: &str, error: Option<&str>) {
        if let Some(error) = error {
            self.input_errors.insert(element_id.into(), error.into());
        } else {
            self.input_errors.remove(element_id);
        }
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
        .style(move |theme| {
            let mut frame = crate::theme::card(theme);
            if focused {
                let palette = crate::theme::palette();
                frame.background = Some(palette.accent.scale_alpha(0.14).into());
                frame.border.color = palette.focus;
                frame.border.width = frame.border.width.max(1.0);
            }
            frame
        })
        .into()
}

/// The same host-owned heading and dismiss control in both panel presentations.
pub fn panel_header<'a>(title: String) -> iced::Element<'a, crate::app::Message> {
    panel_header_with_back(title, false)
}

pub fn panel_header_with_back<'a>(
    title: String,
    back: bool,
) -> iced::Element<'a, crate::app::Message> {
    use canopa_ui::accessibility::{Node, semantic};
    use iced::widget::row;

    let panel = |event| Some(crate::app::Message::Panel(event));
    let mut header = row![];
    if back {
        header = header.push(semantic(
            Node::button("Back")
                .key("panel-back")
                .on_click(panel(crate::app::messages::PanelEvent::Back)),
            crate::theme::action(
                crate::theme::text("‹ Back").size(crate::theme::label_size()),
                panel(crate::app::messages::PanelEvent::Back),
                canopa_ui::Action::Normal,
                false,
            )
            .height((crate::theme::panel_header_height() - 24) as f32),
        ));
    }
    header
        .push(semantic(
            Node::heading(title.clone(), 1).key("panel-title"),
            crate::theme::strong_text(title)
                .size(crate::theme::title_size())
                .width(iced::Length::Fill),
        ))
        .push(semantic(
            Node::button("Close")
                .key("panel-close")
                .shortcut("Escape")
                .on_click(panel(crate::app::messages::PanelEvent::Close)),
            crate::theme::action(
                crate::theme::text("Close").size(crate::theme::label_size()),
                panel(crate::app::messages::PanelEvent::Close),
                canopa_ui::Action::Normal,
                false,
            )
            .height((crate::theme::panel_header_height() - 24) as f32),
        ))
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
