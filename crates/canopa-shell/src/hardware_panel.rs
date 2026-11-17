//! Short-lived Quick settings providers. System services retain device ownership.
use canopa_panel_protocol::{
    InteractionEvent, InteractionTarget, PanelContent, PanelItem, PanelSection,
    v5::{Content, HostMessage, Interaction, Session, Update},
};
use tokio::io::{AsyncBufRead, AsyncWrite};

mod bluetooth;
mod power;

/// How long a hardware provider waits for its service before publishing its
/// first frame; the Shell keeps the panel closed until that frame arrives.
const FIRST_READ: std::time::Duration = std::time::Duration::from_millis(1500);

pub async fn run(
    name: &str,
    input: impl AsyncBufRead + Unpin + Send + 'static,
    mut output: impl AsyncWrite + Unpin,
) -> anyhow::Result<()> {
    let input = crate::provider_io::Input::v5(input);
    match name {
        "bluetooth" => bluetooth::run(input, &mut output).await,
        "power" => power::run(input, &mut output).await,
        _ => anyhow::bail!("unknown hardware panel"),
    }
}

fn section(id: &str, title: &str, items: Vec<PanelItem>) -> PanelSection {
    PanelSection {
        id: id.into(),
        title: title.into(),
        icon: None,
        items,
        controls: Vec::new(),
    }
}

fn item(id: impl Into<String>, title: impl Into<String>) -> PanelItem {
    PanelItem {
        id: id.into(),
        title: title.into(),
        body: None,
        attribution: None,
        styled_body: None,
        rich_body: None,
        metadata: Vec::new(),
        actions: Vec::new(),
        slider: None,
    }
}

async fn publish(
    session: &mut Session,
    output: &mut (impl AsyncWrite + Unpin),
    content: PanelContent,
) -> anyhow::Result<()> {
    let update = session.replace(Content::Rich(content), Update::Replace)?;
    crate::provider_io::write_v5(output, update).await?;
    Ok(())
}

fn action<'a>(
    session: &Session,
    message: &'a HostMessage,
) -> Option<(&'a str, &'a str, Option<&'a str>)> {
    if !session.accepts(message) {
        return None;
    }
    let HostMessage::Event {
        interaction:
            Interaction::Rich(InteractionEvent::Invoke {
                target: InteractionTarget::Item { id },
                action_key,
                value,
            }),
        ..
    } = message
    else {
        return None;
    };
    Some((id, action_key, value.as_deref()))
}
