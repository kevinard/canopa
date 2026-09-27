use canopa_panel_protocol::v1::{
    ExternalPanelDocument, ExternalPanelElement, ExternalPanelSeverity, HostMessage,
    PROTOCOL_VERSION, ProviderEvent, ProviderMessage, read_frame, write_frame,
};
use niribar::wifi_provider::backends::choose_backend;
use niribar::wifi_provider::{WifiBackendErrorCategory, WifiPanelProvider};

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("niribar-wifi-provider: {err}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let HostMessage::Handshake(handshake) = read_frame::<_, HostMessage>(&mut stdin).await? else {
        return Err("expected host handshake".into());
    };
    if handshake.protocol_version != PROTOCOL_VERSION {
        return Err(format!(
            "unsupported external panel protocol version {}",
            handshake.protocol_version
        )
        .into());
    }

    let Some(backend) = choose_backend().await else {
        write_provider_message(
            &mut stdout,
            ProviderMessage::Handshake {
                protocol_version: PROTOCOL_VERSION,
                content: niribar::wifi_provider::no_control_document(),
            },
        )
        .await?;
        return drain_until_close(stdin).await;
    };

    let changes = backend.changes().await?;
    let cleanup = backend.cleanup();
    let mut provider = WifiPanelProvider::new(backend);
    let initial = match tokio::time::timeout(
        std::time::Duration::from_secs(2),
        provider.initial_content(),
    )
    .await
    {
        Ok(Ok(content)) => content,
        Ok(Err(err)) => backend_error_document("Wi-Fi unavailable", err.category()),
        Err(_) => backend_error_document("Wi-Fi unavailable", WifiBackendErrorCategory::NoControl),
    };
    write_provider_message(
        &mut stdout,
        ProviderMessage::Handshake {
            protocol_version: PROTOCOL_VERSION,
            content: initial,
        },
    )
    .await?;

    let result = niribar::wifi_provider::session::run(provider, stdin, stdout, changes).await;
    cleanup.finish().await;
    result?;
    Ok(())
}

async fn write_provider_message<W>(
    writer: &mut W,
    message: ProviderMessage,
) -> Result<(), canopa_panel_protocol::v1::ProtocolError>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    write_frame(writer, &message).await
}

async fn drain_until_close(mut stdin: tokio::io::Stdin) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        match read_frame::<_, HostMessage>(&mut stdin).await {
            Ok(HostMessage::Event(ProviderEvent::Close)) => return Ok(()),
            Ok(HostMessage::Event(_)) => {}
            Ok(HostMessage::Handshake(_)) => return Err("unexpected repeated handshake".into()),
            Err(_) => return Ok(()),
        }
    }
}

fn backend_error_document(
    title: &str,
    category: WifiBackendErrorCategory,
) -> ExternalPanelDocument {
    ExternalPanelDocument {
        elements: vec![ExternalPanelElement::Status {
            title: title.into(),
            message: category_message(category).into(),
            severity: ExternalPanelSeverity::Error,
        }],
    }
}

fn category_message(category: WifiBackendErrorCategory) -> &'static str {
    match category {
        WifiBackendErrorCategory::NoControl => "No supported Wi-Fi control backend is available.",
        WifiBackendErrorCategory::ScanFailed => "Wi-Fi scan failed.",
        WifiBackendErrorCategory::ConnectionFailed => "Connection failed.",
        WifiBackendErrorCategory::CredentialRejected => "Network credential was rejected.",
        WifiBackendErrorCategory::UnsupportedNetwork => "This Wi-Fi network is not supported.",
        WifiBackendErrorCategory::AuthorizationRequired => "Wi-Fi authorization is required.",
    }
}
