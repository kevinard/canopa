use canopa_config::{Request, SharedConfiguration};
use canopa_portal::{BUS_NAME, OBJECT_PATH, Settings};
use clap::Parser;
use futures_util::StreamExt;
use std::path::PathBuf;
use tokio::signal::unix::{SignalKind, signal};

#[derive(Parser)]
#[command(
    version,
    about = "Canopa Settings portal backend; reload shared appearance with SIGHUP"
)]
struct Args {
    /// Shared Canopa file. Otherwise discover it using XDG_CONFIG_HOME/HOME.
    #[arg(long, conflicts_with = "defaults")]
    canopa_config: Option<PathBuf>,
    /// Use embedded shared appearance without reading configuration files.
    #[arg(long)]
    defaults: bool,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run(Args::parse()).await {
        eprintln!("level=error component=settings_portal event=stopped error={error}");
        std::process::exit(1);
    }
}

async fn prepare(request: Request) -> anyhow::Result<SharedConfiguration> {
    Ok(tokio::task::spawn_blocking(move || SharedConfiguration::load(&request)).await??)
}

async fn run(args: Args) -> anyhow::Result<()> {
    let request = match args.canopa_config {
        Some(path) => Request::common(std::path::absolute(path)?),
        None if args.defaults => Request::Defaults,
        None => Request::Discover,
    };
    // Install signal handlers before acquiring the name: readiness includes reload handling.
    let mut reload = signal(SignalKind::hangup())?;
    let mut terminate = signal(SignalKind::terminate())?;
    let initial = prepare(request.clone()).await?;
    let connection = zbus::connection::Builder::session()?
        .serve_at(OBJECT_PATH, Settings::new(initial.appearance))?
        .name(BUS_NAME)?
        .allow_name_replacements(false)
        .replace_existing_names(false)
        .build()
        .await?;
    eprintln!(
        "level=info component=settings_portal event=ready fingerprint={}",
        initial.source.fingerprint
    );
    let mut messages = zbus::MessageStream::from(&connection);
    loop {
        tokio::select! {
            message = messages.next() => {
                if !matches!(message, Some(Ok(_))) { break; }
            },
            _ = terminate.recv() => break,
            result = tokio::signal::ctrl_c() => { result?; break; },
            _ = reload.recv() => {
                match prepare(request.clone()).await {
                    Ok(candidate) => {
                        let interface = connection.object_server().interface::<_, Settings>(OBJECT_PATH).await?;
                        interface.get_mut().await.install(candidate.appearance, interface.signal_emitter()).await?;
                        eprintln!("level=info component=settings_portal event=reloaded fingerprint={}", candidate.source.fingerprint);
                    }
                    Err(error) => eprintln!("level=warn component=settings_portal event=reload_rejected error={error}"),
                }
            }
        }
    }
    connection.close().await?;
    Ok(())
}
