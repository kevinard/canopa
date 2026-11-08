use canopa_panel_protocol::v1::{
    ExternalPanelDocument, ExternalPanelElement, ExternalPanelSeverity, ProviderEvent,
};
use canopa_panel_protocol::v5::Update;
use canopa_shell::wifi_provider::backends::choose_backend;
use canopa_shell::wifi_provider::transport::{Action, Transport};
use canopa_shell::wifi_provider::{WifiBackendErrorCategory, WifiPanelProvider};

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("canopa-wifi-provider: {err}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let transport = match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] | ["--protocol", "niribar-v1"] => Transport::V1,
        ["--protocol", "canopa-v5"] => Transport::v5(),
        _ => return Err("usage: canopa-wifi-provider [--protocol niribar-v1|canopa-v5]".into()),
    };
    let mut stdin = tokio::io::BufReader::new(tokio::io::stdin());
    let mut stdout = tokio::io::stdout();
    transport.handshake(&mut stdin).await?;

    let mut initial = true;
    let backend = loop {
        if let Some(backend) = choose_backend().await {
            break backend;
        }
        transport
            .write(
                &mut stdout,
                canopa_shell::wifi_provider::no_control_document(),
                initial,
                Update::Replace,
            )
            .await?;
        initial = false;
        loop {
            let input = transport.read(&mut stdin).await.ok();
            match transport.action(input) {
                Action::Close => return Ok(()),
                Action::Event(ProviderEvent::ButtonActivated { element_id })
                    if element_id == "refresh" =>
                {
                    break;
                }
                _ => {}
            }
        }
    };

    let changes = backend.changes().await?;
    let cleanup = backend.cleanup();
    let mut provider = WifiPanelProvider::new(backend);
    let document = match tokio::time::timeout(
        std::time::Duration::from_secs(2),
        provider.initial_content(),
    )
    .await
    {
        Ok(Ok(content)) => content,
        Ok(Err(err)) => backend_error_document("Wi-Fi unavailable", err.category()),
        Err(_) => backend_error_document("Wi-Fi unavailable", WifiBackendErrorCategory::NoControl),
    };
    transport
        .write(&mut stdout, document, initial, Update::Replace)
        .await?;
    let result = canopa_shell::wifi_provider::session::run_with_transport(
        provider, stdin, stdout, changes, transport,
    )
    .await;
    cleanup.finish().await;
    result?;
    Ok(())
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
        WifiBackendErrorCategory::TargetChanged => "The selected network changed. Select it again.",
        WifiBackendErrorCategory::NoControl => "No supported Wi-Fi control backend is available.",
        WifiBackendErrorCategory::ScanFailed => "Wi-Fi scan failed.",
        WifiBackendErrorCategory::ConnectionFailed => "Connection failed.",
        WifiBackendErrorCategory::CredentialRejected => "Network credential was rejected.",
        WifiBackendErrorCategory::UnsupportedNetwork => "This Wi-Fi network is not supported.",
        WifiBackendErrorCategory::AuthorizationRequired => "Wi-Fi authorization is required.",
    }
}
