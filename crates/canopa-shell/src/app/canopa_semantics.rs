//! Shared body selection and unambiguous native/widget identity.
use canopa_panel_protocol::{PanelItem, PanelMetadata, RichBody, StyledText};

pub(super) fn metadata_label(metadata: &PanelMetadata) -> String {
    if metadata.value.is_empty() {
        metadata.label.clone()
    } else {
        format!("{}: {}", metadata.label, metadata.value)
    }
}

pub(super) enum Body<'a> {
    Rich(&'a RichBody),
    Styled(&'a StyledText),
    Plain(&'a str),
    None,
}

pub(super) fn body(item: &PanelItem) -> Body<'_> {
    if let Some(body) = &item.rich_body {
        Body::Rich(body)
    } else if let Some(body) = &item.styled_body {
        Body::Styled(body)
    } else if let Some(body) = &item.body {
        Body::Plain(body)
    } else {
        Body::None
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
pub enum Node {
    Root,
    Error,
    Section(String),
    Item(String),
    Attribution(String),
    Control(String, String),
    Body(String, Option<usize>),
    Metadata(String, usize),
    Action(String),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
pub struct Key {
    pub provider: String,
    pub generation: u64,
    pub node: Node,
}

impl Key {
    pub fn clips_children(&self) -> bool {
        matches!(self.node, Node::Root | Node::Section(_) | Node::Item(_))
    }
}
