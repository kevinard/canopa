use canopa_panel_protocol::v1::{
    ExternalPanelDocument, ExternalPanelElement, ExternalPanelSeverity,
};
use canopa_panel_protocol::v5::Update;
use niribar::wifi_provider::backends::choose_backend;
use niribar::wifi_provider::transport::{Action, Transport};
use niribar::wifi_provider::{WifiBackendErrorCategory, WifiPanelProvider};

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("niribar-wifi-provider: {err}");
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
        _ => return Err("usage: niribar-wifi-provider [--protocol niribar-v1|canopa-v5]".into()),
    };
    let mut stdin = tokio::io::BufReader::new(tokio::io::stdin());
    let mut stdout = tokio::io::stdout();
    transport.handshake(&mut stdin).await?;

    let Some(backend) = choose_backend().await else {
        transport
            .write(
                &mut stdout,
                niribar::wifi_provider::no_control_document(),
                true,
                Update::Replace,
            )
            .await?;
        loop {
            let input = transport.read(&mut stdin).await.ok();
            if matches!(transport.action(input), Action::Close) {
                return Ok(());
            }
        }
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
    transport
        .write(&mut stdout, initial, true, Update::Replace)
        .await?;
    let result = niribar::wifi_provider::session::run_with_transport(
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
        WifiBackendErrorCategory::NoControl => "No supported Wi-Fi control backend is available.",
        WifiBackendErrorCategory::ScanFailed => "Wi-Fi scan failed.",
        WifiBackendErrorCategory::ConnectionFailed => "Connection failed.",
        WifiBackendErrorCategory::CredentialRejected => "Network credential was rejected.",
        WifiBackendErrorCategory::UnsupportedNetwork => "This Wi-Fi network is not supported.",
        WifiBackendErrorCategory::AuthorizationRequired => "Wi-Fi authorization is required.",
    }
}
