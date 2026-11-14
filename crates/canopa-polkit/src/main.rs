use anyhow::{Result, ensure};
use canopa_config::Request;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about = "Canopa graphical polkit authentication agent")]
struct Cli {
    #[arg(long, global = true, conflicts_with = "defaults")]
    canopa_config: Option<PathBuf>,
    #[arg(long, global = true)]
    defaults: bool,
    /// Select an owned graphical logind session when several are available.
    #[arg(long)]
    session: Option<String>,
    #[command(subcommand)]
    command: Option<Operation>,
}
#[derive(Subcommand)]
enum Operation {
    #[command(hide = true)]
    InternalIdentity {
        uid: u32,
    },
    CheckConfig {
        #[arg(long)]
        show_source: bool,
    },
    Preview {
        output: PathBuf,
    },
}
fn main() {
    canopa_i18n::init();
    if let Err(error) = run(Cli::parse()) {
        eprintln!("canopa-polkit: {error:#}");
        std::process::exit(1);
    }
}
fn run(cli: Cli) -> Result<()> {
    if let Some(Operation::InternalIdentity { uid }) = cli.command {
        return canopa_polkit::identity::helper(uid);
    }
    let notify = std::env::var_os("NOTIFY_SOCKET").map(PathBuf::from);
    // Before any threads/native library calls: polkit debugging can expose cookies.
    unsafe {
        libc::setlocale(libc::LC_ALL, c"".as_ptr());
        std::env::remove_var("POLKIT_DEBUG");
        std::env::remove_var("NOTIFY_SOCKET");
        std::env::remove_var("GREETD_SOCK");
        std::env::set_var("GIO_USE_VFS", "local");
    }
    let request = if cli.defaults {
        Request::Defaults
    } else {
        cli.canopa_config
            .map(Request::common)
            .unwrap_or(Request::Discover)
    };
    let loaded = canopa_polkit::config::load(&request);
    if let Some(Operation::CheckConfig { show_source }) = cli.command {
        return match loaded {
            Ok((_, document)) => {
                println!(
                    "{}",
                    serde_json::json!({"component":"polkit","source":document.info()})
                );
                Ok(())
            }
            Err(error) => {
                if show_source {
                    eprintln!("{error:#}");
                }
                let diagnostic=error.downcast_ref::<canopa_config::Diagnostic>().map(serde_json::to_value).transpose()?.unwrap_or_else(||serde_json::json!({"code":"polkit-schema","message":"invalid polkit configuration"}));
                println!(
                    "{}",
                    serde_json::json!({"component":"polkit","error":diagnostic})
                );
                Err(error)
            }
        };
    }
    let (settings, _) = loaded?;
    if let Some(Operation::Preview { output }) = cli.command {
        let controller = canopa_polkit::controller::Controller::preview(&settings)?;
        let width = canopa_session_ui::view::AUTHORIZATION_DIALOG_SIZE.0;
        let height = controller.ui.dialog_height(
            canopa_session_ui::view::DIALOG_MIN_HEIGHT,
            canopa_session_ui::view::DIALOG_MAX_HEIGHT,
        );
        controller
            .ui
            .render_dialog(&settings, width, height, 1.)?
            .0
            .save_with_format(output, image::ImageFormat::Png)?;
        return Ok(());
    }
    ensure!(
        unsafe { libc::geteuid() } != 0,
        "run the authentication agent as an unprivileged desktop user"
    );
    canopa_session_ui::secret::protect_process()?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let session = runtime.block_on(canopa_polkit::session::discover(cli.session.as_deref()))?;
    drop(runtime);
    canopa_polkit::wayland::run(settings, &session, notify)
}
