#[tokio::main]
async fn main() {
    canopa_i18n::init();
    // Keep the Pulse client name the audio server showed before the shared client.
    canopa_services::audio::set_client_name("canopa-shell");
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .try_init();
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["--help"] {
        println!("Usage: canopa-audio-provider [--protocol canopa-v5]");
        return;
    }
    if !args.is_empty() && args != ["--protocol", "canopa-v5"] {
        eprintln!("Usage: canopa-audio-provider [--protocol canopa-v5]");
        std::process::exit(2);
    }
    if canopa_shell::audio_provider::run(
        tokio::io::BufReader::new(tokio::io::stdin()),
        tokio::io::stdout(),
        None,
    )
    .await
    .is_err()
    {
        tracing::error!(
            component = "audio_provider",
            "audio provider session failed"
        );
        std::process::exit(1);
    }
}
