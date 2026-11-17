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
    /// The action key of the failure panel's Retry, handled by the Shell.
    pub const RETRY: &'static str = "canopa-shell-retry";

    /// A provider that failed or stopped: a plain explanation, the technical
    /// message under Details, and Retry. It is Canopa content so it has the
    /// same keyboard focus and accessibility nodes as a working provider.
    pub fn failure(provider_name: impl Into<String>, message: impl Into<String>) -> Self {
        use canopa_panel_protocol::{PanelAction, PanelContent, PanelItem, PanelSection};
        let content = PanelContent {
            title: fl!("shell-provider-failed"),
            sections: vec![PanelSection {
                id: "details".into(),
                title: fl!("shell-provider-failed-details"),
                icon: None,
                items: vec![PanelItem {
                    id: "message".into(),
                    title: message.into(),
                    attribution: None,
                    body: None,
                    styled_body: None,
                    rich_body: None,
                    metadata: Vec::new(),
                    actions: Vec::new(),
                    slider: None,
                }],
                controls: Vec::new(),
            }],
            actions: vec![PanelAction {
                key: Self::RETRY.into(),
                label: fl!("shell-provider-retry"),
                value: None,
                request_activation_token: false,
                destructive: false,
                pressed: None,
            }],
            error: Some(fl!("shell-provider-failed-explanation")),
        };
        Self::from_provider_document(
            provider_name,
            ExternalPanelDocument::Canopa(Box::new(CanopaSnapshot::local(content))),
        )
    }

    pub fn elements(&self) -> &[ExternalPanelElement] {
        self.document.elements()
    }

    #[cfg(test)]
    pub fn failure_text(&self) -> String {
        if let Some(snapshot) = self.document.canopa() {
            let content = &snapshot.content;
            return std::iter::once(content.title.as_str())
                .chain(content.error.as_deref())
                .chain(
                    content
                        .sections
                        .iter()
                        .flat_map(|section| &section.items)
                        .map(|item| item.title.as_str()),
                )
                .collect::<Vec<_>>()
                .join("\n");
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

/// The widget holding a panel's icon-only Close control.
pub const PANEL_CLOSE_ID: &str = "panel-close";

/// The same host-owned heading and dismiss control in both panel presentations.
pub fn panel_header<'a>(title: String) -> iced::Element<'a, crate::app::Message> {
    panel_header_with_back(title, false)
}

pub fn panel_header_with_back<'a>(
    title: String,
    back: bool,
) -> iced::Element<'a, crate::app::Message> {
    use canopa_ui::accessibility::{Node, semantic};

    let panel = |event| Some(crate::app::Message::Panel(event));
    // Back, heading and Close in reading order (mirrored for RTL).
    let mut cells: Vec<iced::Element<'a, crate::app::Message>> = Vec::new();
    if back {
        cells.push(semantic(
            Node::button(fl!("shell-back"))
                .key("panel-back")
                .on_click(panel(crate::app::messages::PanelEvent::Back)),
            crate::theme::action(
                crate::theme::text(canopa_ui::direction::back_label())
                    .size(crate::theme::label_size()),
                panel(crate::app::messages::PanelEvent::Back),
                canopa_ui::Action::Normal,
                false,
            )
            .height((crate::theme::panel_header_height() - 24) as f32),
        ));
    }
    cells.push(semantic(
        Node::heading(title.clone(), 1).key("panel-title"),
        crate::theme::strong_text(title)
            .size(crate::theme::title_size())
            .width(iced::Length::Fill)
            .align_x(canopa_ui::direction::text_start()),
    ));
    cells.push(semantic(
        Node::button(fl!("shell-close"))
            .key("panel-close")
            .shortcut("Escape")
            .on_click(panel(crate::app::messages::PanelEvent::Close)),
        iced::widget::container(crate::theme::icon_button(
            crate::theme::CLOSE_ICON,
            panel(crate::app::messages::PanelEvent::Close),
            false,
        ))
        .id(PANEL_CLOSE_ID),
    ));
    canopa_ui::direction::row(cells)
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
