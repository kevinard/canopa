use canopa_config::{Request, SharedConfiguration};
use canopa_portal::{
    BUS_NAME, OBJECT_PATH, Settings,
    access::Access,
    account::Account,
    app_chooser::AppChooser,
    background::{Background, NotifyPresenter, Observation, Observer, Unavailable},
    dynamic_launcher::DynamicLauncher,
    file_chooser::{FileChooser, ProcessPresenter},
    lockdown::{Lockdown, Source as PolicySource},
    requests::Requests,
    screencast::{ScreenCast, remote::RemoteDesktop},
    screenshot::{Production as ScreenshotPresenter, Screenshot},
    wallpaper::{Production as WallpaperPresenter, Wallpaper},
};
use clap::Parser;
use futures_util::StreamExt;
use std::path::PathBuf;
use tokio::signal::unix::{SignalKind, signal};

#[derive(Parser)]
#[command(version, about = "Canopa desktop portal backend")]
struct Args {
    /// Shared Canopa file. Otherwise discover it using XDG_CONFIG_HOME/HOME.
    #[arg(long, conflicts_with = "defaults")]
    canopa_config: Option<PathBuf>,
    /// Use embedded shared appearance without reading configuration files.
    #[arg(long)]
    defaults: bool,
    /// Do not remember per-application FileChooser folders.
    #[arg(long)]
    no_file_chooser_history: bool,
    /// Runtime privacy policy database. Otherwise use XDG_STATE_HOME/HOME.
    #[arg(long)]
    lockdown_policy: Option<PathBuf>,
    /// Publish the selected runtime policy while denying property writes.
    #[arg(long)]
    lockdown_read_only: bool,
    /// Enable the experimental Print interface for explicit portal routing.
    #[arg(long, conflicts_with_all = ["file_chooser", "app_chooser", "access_dialog", "account_dialog", "background_dialog", "dynamic_launcher_dialog", "wallpaper_dialog", "screenshot_dialog", "screencast_worker", "screencast_dialog", "screencast_status", "remote_input_worker", "remote_desktop_dialog", "input_capture_worker", "input_capture_dialog", "usb_dialog", "global_shortcuts_dialog", "print_worker", "print_dialog", "print_layout_worker", "recover_print", "print_recovery_dialog"])]
    experimental_print: bool,
    /// Review live printer jobs through the opt-in running Canopa backend.
    #[arg(long, conflicts_with_all = ["experimental_print", "file_chooser", "app_chooser", "access_dialog", "account_dialog", "background_dialog", "dynamic_launcher_dialog", "wallpaper_dialog", "screenshot_dialog", "screencast_worker", "screencast_dialog", "screencast_status", "remote_input_worker", "remote_desktop_dialog", "input_capture_worker", "input_capture_dialog", "usb_dialog", "global_shortcuts_dialog", "print_worker", "print_dialog", "print_authentication_dialog", "print_layout_worker", "recover_print", "print_recovery_dialog", "print_jobs_dialog"])]
    printer_jobs: bool,
    #[arg(long, hide = true, conflicts_with_all = ["experimental_print", "file_chooser", "app_chooser", "access_dialog", "account_dialog", "background_dialog", "dynamic_launcher_dialog", "wallpaper_dialog", "screenshot_dialog", "screencast_worker", "screencast_dialog", "screencast_status", "remote_input_worker", "remote_desktop_dialog", "input_capture_worker", "input_capture_dialog", "usb_dialog", "global_shortcuts_dialog", "print_worker", "print_dialog", "print_authentication_dialog", "print_layout_worker", "recover_print", "print_recovery_dialog"])]
    print_jobs_dialog: bool,

    #[arg(long, hide = true)]
    file_chooser: bool,
    #[arg(long, hide = true, conflicts_with = "file_chooser")]
    app_chooser: bool,
    #[arg(long, hide = true, conflicts_with_all = ["file_chooser", "app_chooser"])]
    access_dialog: bool,
    #[arg(long, hide = true, conflicts_with_all = ["file_chooser", "app_chooser", "access_dialog"])]
    account_dialog: bool,
    #[arg(long, hide = true, conflicts_with_all = ["file_chooser", "app_chooser", "access_dialog", "account_dialog"])]
    background_dialog: bool,
    #[arg(long, hide = true, conflicts_with_all = ["file_chooser", "app_chooser", "access_dialog", "account_dialog", "background_dialog"])]
    dynamic_launcher_dialog: bool,
    #[arg(long, hide = true, conflicts_with_all = ["file_chooser", "app_chooser", "access_dialog", "account_dialog", "background_dialog", "dynamic_launcher_dialog"])]
    wallpaper_dialog: bool,
    #[arg(long, hide = true, conflicts_with_all = ["file_chooser", "app_chooser", "access_dialog", "account_dialog", "background_dialog", "dynamic_launcher_dialog", "wallpaper_dialog"])]
    screenshot_dialog: bool,
    #[arg(long, hide = true, conflicts_with_all = ["experimental_print", "file_chooser", "app_chooser", "access_dialog", "account_dialog", "background_dialog", "dynamic_launcher_dialog", "wallpaper_dialog", "screenshot_dialog", "screencast_worker", "screencast_dialog", "screencast_status", "remote_input_worker", "remote_desktop_dialog", "input_capture_worker", "input_capture_dialog", "usb_dialog", "global_shortcuts_dialog", "print_worker", "print_dialog", "print_authentication_dialog", "print_layout_worker", "recover_print", "print_recovery_dialog", "print_jobs_dialog", "printer_jobs"])]
    screenshot_window_picker: bool,
    #[arg(long, hide = true, conflicts_with_all = ["file_chooser", "app_chooser", "access_dialog", "account_dialog", "background_dialog", "dynamic_launcher_dialog", "wallpaper_dialog", "screenshot_dialog"])]
    screencast_worker: bool,
    #[arg(long, hide = true, conflicts_with_all = ["file_chooser", "app_chooser", "access_dialog", "account_dialog", "background_dialog", "dynamic_launcher_dialog", "wallpaper_dialog", "screenshot_dialog", "screencast_worker"])]
    screencast_dialog: bool,
    #[arg(long, hide = true, conflicts_with_all = ["file_chooser", "app_chooser", "access_dialog", "account_dialog", "background_dialog", "dynamic_launcher_dialog", "wallpaper_dialog", "screenshot_dialog", "screencast_worker", "screencast_dialog"])]
    screencast_status: bool,
    #[arg(long, hide = true, conflicts_with_all = ["file_chooser", "app_chooser", "access_dialog", "account_dialog", "background_dialog", "dynamic_launcher_dialog", "wallpaper_dialog", "screenshot_dialog", "screencast_worker", "screencast_dialog", "screencast_status"])]
    remote_input_worker: bool,
    #[arg(long, hide = true, conflicts_with_all = ["file_chooser", "app_chooser", "access_dialog", "account_dialog", "background_dialog", "dynamic_launcher_dialog", "wallpaper_dialog", "screenshot_dialog", "screencast_worker", "screencast_dialog", "screencast_status", "remote_input_worker"])]
    remote_desktop_dialog: bool,
    #[arg(long, hide = true, conflicts_with_all = ["file_chooser", "app_chooser", "access_dialog", "account_dialog", "background_dialog", "dynamic_launcher_dialog", "wallpaper_dialog", "screenshot_dialog", "screencast_worker", "screencast_dialog", "screencast_status", "remote_input_worker", "remote_desktop_dialog"])]
    input_capture_worker: bool,
    #[arg(long, hide = true, conflicts_with_all = ["file_chooser", "app_chooser", "access_dialog", "account_dialog", "background_dialog", "dynamic_launcher_dialog", "wallpaper_dialog", "screenshot_dialog", "screencast_worker", "screencast_dialog", "screencast_status", "remote_input_worker", "remote_desktop_dialog", "input_capture_worker"])]
    input_capture_dialog: bool,
    #[arg(long, hide = true, conflicts_with_all = ["file_chooser", "app_chooser", "access_dialog", "account_dialog", "background_dialog", "dynamic_launcher_dialog", "wallpaper_dialog", "screenshot_dialog", "screencast_worker", "screencast_dialog", "screencast_status", "remote_input_worker", "remote_desktop_dialog", "input_capture_worker", "input_capture_dialog"])]
    usb_dialog: bool,
    #[arg(long, hide = true, conflicts_with_all = ["file_chooser", "app_chooser", "access_dialog", "account_dialog", "background_dialog", "dynamic_launcher_dialog", "wallpaper_dialog", "screenshot_dialog", "screencast_worker", "screencast_dialog", "screencast_status", "remote_input_worker", "remote_desktop_dialog", "input_capture_worker", "input_capture_dialog", "usb_dialog"])]
    global_shortcuts_dialog: bool,
    #[arg(long, hide = true, conflicts_with_all = ["file_chooser", "app_chooser", "access_dialog", "account_dialog", "background_dialog", "dynamic_launcher_dialog", "wallpaper_dialog", "screenshot_dialog", "screencast_worker", "screencast_dialog", "screencast_status", "remote_input_worker", "remote_desktop_dialog", "input_capture_worker", "input_capture_dialog", "usb_dialog", "global_shortcuts_dialog"])]
    print_worker: bool,
    #[arg(long, hide = true, conflicts_with_all = ["file_chooser", "app_chooser", "access_dialog", "account_dialog", "background_dialog", "dynamic_launcher_dialog", "wallpaper_dialog", "screenshot_dialog", "screencast_worker", "screencast_dialog", "screencast_status", "remote_input_worker", "remote_desktop_dialog", "input_capture_worker", "input_capture_dialog", "usb_dialog", "global_shortcuts_dialog", "print_worker"])]
    print_dialog: bool,
    #[arg(long, hide = true, conflicts_with_all = ["file_chooser", "app_chooser", "access_dialog", "account_dialog", "background_dialog", "dynamic_launcher_dialog", "wallpaper_dialog", "screenshot_dialog", "screencast_worker", "screencast_dialog", "screencast_status", "remote_input_worker", "remote_desktop_dialog", "input_capture_worker", "input_capture_dialog", "usb_dialog", "global_shortcuts_dialog", "print_worker", "print_dialog", "print_layout_worker", "recover_print", "print_recovery_dialog", "experimental_print"])]
    print_authentication_dialog: bool,
    #[arg(long, hide = true, conflicts_with_all = ["file_chooser", "app_chooser", "access_dialog", "account_dialog", "background_dialog", "dynamic_launcher_dialog", "wallpaper_dialog", "screenshot_dialog", "screencast_worker", "screencast_dialog", "screencast_status", "remote_input_worker", "remote_desktop_dialog", "input_capture_worker", "input_capture_dialog", "usb_dialog", "global_shortcuts_dialog", "print_worker", "print_dialog"])]
    print_layout_worker: bool,
    /// Review retained print files and unresolved printer jobs.
    #[arg(long, conflicts_with_all = ["file_chooser", "app_chooser", "access_dialog", "account_dialog", "background_dialog", "dynamic_launcher_dialog", "wallpaper_dialog", "screenshot_dialog", "screencast_worker", "screencast_dialog", "screencast_status", "remote_input_worker", "remote_desktop_dialog", "input_capture_worker", "input_capture_dialog", "usb_dialog", "global_shortcuts_dialog", "print_worker", "print_dialog", "print_layout_worker", "print_recovery_dialog"])]
    recover_print: bool,
    /// Override the private print recovery store (also used for startup review).
    #[arg(long, conflicts_with = "print_recovery_dialog")]
    print_recovery_store: Option<PathBuf>,
    #[arg(long, hide = true, conflicts_with_all = ["file_chooser", "app_chooser", "access_dialog", "account_dialog", "background_dialog", "dynamic_launcher_dialog", "wallpaper_dialog", "screenshot_dialog", "screencast_worker", "screencast_dialog", "screencast_status", "remote_input_worker", "remote_desktop_dialog", "input_capture_worker", "input_capture_dialog", "usb_dialog", "global_shortcuts_dialog", "print_worker", "print_dialog", "print_layout_worker", "recover_print"])]
    print_recovery_dialog: bool,
}

fn main() {
    canopa_i18n::init();
    let args = Args::parse();
    let result = if args.printer_jobs {
        tokio::runtime::Runtime::new()
            .and_then(|runtime| {
                runtime
                    .block_on(review_printer_jobs())
                    .map_err(std::io::Error::other)
            })
            .map_err(anyhow::Error::from)
    } else if args.print_jobs_dialog {
        print_jobs_dialog(args)
    } else if args.recover_print || args.print_recovery_dialog {
        print_recovery(args)
    } else if args.file_chooser {
        chooser(args)
    } else if args.app_chooser {
        app_chooser(args)
    } else if args.access_dialog {
        access_dialog(args)
    } else if args.account_dialog {
        account_dialog(args)
    } else if args.background_dialog {
        background_dialog(args)
    } else if args.dynamic_launcher_dialog {
        dynamic_launcher_dialog(args)
    } else if args.wallpaper_dialog {
        wallpaper_dialog(args)
    } else if args.screenshot_dialog {
        screenshot_dialog(args)
    } else if args.screencast_worker {
        canopa_portal::screencast_capture::run_worker()
    } else if args.screencast_dialog || args.remote_desktop_dialog || args.screenshot_window_picker
    {
        screencast_dialog(args)
    } else if args.screencast_status {
        screencast_status(args)
    } else if args.remote_input_worker {
        remote_input_worker()
    } else if args.input_capture_worker {
        input_capture_worker()
    } else if args.input_capture_dialog {
        input_capture_dialog(args)
    } else if args.usb_dialog {
        usb_dialog(args)
    } else if args.global_shortcuts_dialog {
        global_shortcuts_dialog(args)
    } else if args.print_layout_worker {
        #[cfg(feature = "desktop")]
        {
            canopa_portal::print::render::run_layout_worker()
        }
        #[cfg(not(feature = "desktop"))]
        {
            anyhow::bail!("Print layout requires the desktop feature")
        }
    } else if args.print_worker {
        print_worker()
    } else if args.print_dialog {
        print_dialog(args)
    } else if args.print_authentication_dialog {
        print_authentication_dialog(args)
    } else {
        tokio::runtime::Runtime::new()
            .and_then(|runtime| runtime.block_on(run(args)).map_err(std::io::Error::other))
            .map_err(anyhow::Error::from)
    };
    if let Err(error) = result {
        eprintln!("level=error component=portal event=stopped error={error}");
        std::process::exit(1);
    }
}

async fn review_printer_jobs() -> anyhow::Result<()> {
    let connection = zbus::Connection::session().await?;
    let sender = connection
        .unique_name()
        .ok_or_else(|| anyhow::anyhow!("Session bus identity unavailable"))?
        .as_str()
        .trim_start_matches(':')
        .replace('.', "_");
    let path: zbus::zvariant::OwnedObjectPath = format!(
        "/org/freedesktop/portal/desktop/request/{sender}/printer_jobs_{}",
        std::process::id()
    )
    .try_into()?;
    let proxy = zbus::Proxy::new(
        &connection,
        BUS_NAME,
        OBJECT_PATH,
        "org.canopa.Portal.PrintJobs",
    )
    .await?;
    let code: u32 = proxy.call("Review", &(path, String::new())).await
        .map_err(|error| anyhow::anyhow!("Printer job review requires the running Canopa backend with --experimental-print: {error}"))?;
    anyhow::ensure!(code != 2, "Printer job review is unavailable");
    Ok(())
}
fn print_jobs_dialog(args: Args) -> anyhow::Result<()> {
    #[cfg(feature = "desktop")]
    {
        use std::io::{Read, Write};
        let mut bytes = Vec::new();
        std::io::stdin().take(8193).read_to_end(&mut bytes)?;
        anyhow::ensure!(
            bytes.len() <= 8192,
            "Printer review configuration exceeds limits"
        );
        let request = serde_json::from_slice(&bytes)?;
        let source = match args.canopa_config {
            Some(path) => Request::common(path),
            None if args.defaults => Request::Defaults,
            None => Request::Discover,
        };
        let configuration = SharedConfiguration::load(&source)?;
        canopa_portal::print_jobs_ui::run(request, configuration.appearance)?;
        std::io::stdout().write_all(b"true")?;
        Ok(())
    }
    #[cfg(not(feature = "desktop"))]
    {
        let _ = args;
        anyhow::bail!("Printer review requires the desktop feature")
    }
}

fn print_recovery(args: Args) -> anyhow::Result<()> {
    #[cfg(feature = "desktop")]
    {
        use canopa_portal::print::file_target::RecoveryStore;
        use std::io::{Read, Write};
        let store = if args.print_recovery_dialog {
            let mut bytes = Vec::new();
            std::io::stdin()
                .take(32 * 1024 + 1)
                .read_to_end(&mut bytes)?;
            anyhow::ensure!(
                bytes.len() <= 32 * 1024,
                "Recovery configuration exceeds limits"
            );
            serde_json::from_slice(&bytes)?
        } else {
            match args.print_recovery_store {
                Some(path) => RecoveryStore::new(&std::path::absolute(path)?)?,
                None => RecoveryStore::discover()?,
            }
        };
        let arguments = match args.canopa_config.as_ref() {
            Some(path) => vec!["--canopa-config".into(), path.as_os_str().to_owned()],
            None if args.defaults => vec!["--defaults".into()],
            None => Vec::new(),
        };
        let source = match args.canopa_config {
            Some(path) => Request::common(path),
            None if args.defaults => Request::Defaults,
            None => Request::Discover,
        };
        let configuration = SharedConfiguration::load(&source)?;
        canopa_portal::print_recovery_ui::run(
            store,
            configuration.appearance,
            canopa_portal::process::DialogProcess {
                executable: canopa_file::executable::current()?,
                arguments,
            },
        )?;
        if args.print_recovery_dialog {
            std::io::stdout().write_all(b"true")?;
        }
        Ok(())
    }
    #[cfg(not(feature = "desktop"))]
    {
        let _ = args;
        anyhow::bail!("Print recovery requires the desktop feature")
    }
}

fn print_worker() -> anyhow::Result<()> {
    #[cfg(feature = "desktop")]
    {
        canopa_portal::print::worker::run_worker()
    }
    #[cfg(not(feature = "desktop"))]
    {
        anyhow::bail!("Print worker requires the desktop feature")
    }
}

fn print_dialog(args: Args) -> anyhow::Result<()> {
    #[cfg(feature = "desktop")]
    {
        use std::io::{Read, Write};
        let mut bytes = Vec::new();
        std::io::stdin()
            .take(u64::from(canopa_portal::print::MAX_REQUEST_BYTES) + 1)
            .read_to_end(&mut bytes)?;
        anyhow::ensure!(
            bytes.len() <= canopa_portal::print::MAX_REQUEST_BYTES as usize,
            "Print dialog request too large"
        );
        let mut request: canopa_portal::print_ui::Request = serde_json::from_slice(&bytes)?;
        let channel = request
            .channel
            .take()
            .map(canopa_portal::print::file_target::inherited_channel)
            .transpose()?;
        let arguments = match args.canopa_config.as_ref() {
            Some(path) => vec!["--canopa-config".into(), path.as_os_str().to_owned()],
            None if args.defaults => vec!["--defaults".into()],
            None => Vec::new(),
        };
        let source = match args.canopa_config {
            Some(path) => Request::common(path),
            None if args.defaults => Request::Defaults,
            None => Request::Discover,
        };
        let configuration = SharedConfiguration::load(&source)?;
        let process = canopa_portal::process::DialogProcess {
            executable: canopa_file::executable::current()?,
            arguments,
        };
        let approved = canopa_portal::print_ui::run(request, configuration.appearance, process)?;
        if let (Some(channel), Some(approved)) = (channel.as_ref(), approved.as_ref()) {
            approved.send(channel)?;
        }
        std::io::stdout().write_all(&serde_json::to_vec(
            &approved.as_ref().map(|approved| &approved.grant),
        )?)?;
        Ok(())
    }
    #[cfg(not(feature = "desktop"))]
    {
        let _ = args;
        anyhow::bail!("Print dialogs require the desktop feature")
    }
}

fn print_authentication_dialog(args: Args) -> anyhow::Result<()> {
    #[cfg(feature = "desktop")]
    {
        use std::io::{Read, Write};
        let mut bytes = Vec::new();
        std::io::stdin()
            .take(u64::from(canopa_portal::print::MAX_REQUEST_BYTES) + 1)
            .read_to_end(&mut bytes)?;
        anyhow::ensure!(
            bytes.len() <= canopa_portal::print::MAX_REQUEST_BYTES as usize,
            "Printer authentication request too large"
        );
        let request = serde_json::from_slice(&bytes)?;
        let source = match args.canopa_config {
            Some(path) => Request::common(path),
            None if args.defaults => Request::Defaults,
            None => Request::Discover,
        };
        let configuration = SharedConfiguration::load(&source)?;
        let accepted =
            canopa_portal::print_authentication_ui::run(request, configuration.appearance)?;
        std::io::stdout().write_all(&serde_json::to_vec(&accepted)?)?;
        Ok(())
    }
    #[cfg(not(feature = "desktop"))]
    {
        let _ = args;
        anyhow::bail!("Printer authentication requires the desktop feature")
    }
}

fn remote_input_worker() -> anyhow::Result<()> {
    #[cfg(feature = "desktop")]
    {
        canopa_portal::remote_input::run_worker()
    }
    #[cfg(not(feature = "desktop"))]
    {
        anyhow::bail!("Remote input requires the desktop feature")
    }
}

fn input_capture_worker() -> anyhow::Result<()> {
    #[cfg(feature = "desktop")]
    {
        canopa_portal::input_capture_transport::run_worker()
    }
    #[cfg(not(feature = "desktop"))]
    {
        anyhow::bail!("Input capture requires the desktop feature")
    }
}

fn input_capture_dialog(args: Args) -> anyhow::Result<()> {
    #[cfg(feature = "desktop")]
    {
        use std::io::{Read, Write};
        let mut bytes = Vec::new();
        std::io::stdin()
            .take(canopa_portal::process::MAX_MESSAGE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        anyhow::ensure!(
            bytes.len() <= canopa_portal::process::MAX_MESSAGE_BYTES,
            "Capture consent too large"
        );
        let source = match args.canopa_config {
            Some(path) => Request::common(path),
            None if args.defaults => Request::Defaults,
            None => Request::Discover,
        };
        let configuration = SharedConfiguration::load(&source)?;
        let output = canopa_portal::input_capture_ui::run(
            serde_json::from_slice(&bytes)?,
            configuration.appearance,
        )?;
        std::io::stdout().write_all(&serde_json::to_vec(&output)?)?;
        Ok(())
    }
    #[cfg(not(feature = "desktop"))]
    {
        let _ = args;
        anyhow::bail!("Input capture consent requires the desktop feature")
    }
}

fn screencast_status(args: Args) -> anyhow::Result<()> {
    #[cfg(feature = "desktop")]
    {
        let mut input = std::io::BufReader::new(std::io::stdin());
        let controls = canopa_portal::process::read_frame(&mut input)?;
        let source = match args.canopa_config {
            Some(path) => Request::common(path),
            None if args.defaults => Request::Defaults,
            None => Request::Discover,
        };
        let configuration = SharedConfiguration::load(&source)?;
        canopa_portal::screencast_status::run(controls, configuration.appearance, input)
    }
    #[cfg(not(feature = "desktop"))]
    {
        let _ = args;
        anyhow::bail!("Screen sharing controls require the desktop feature")
    }
}

fn screencast_dialog(args: Args) -> anyhow::Result<()> {
    #[cfg(feature = "desktop")]
    {
        use std::io::{Read, Write};
        let mut bytes = Vec::new();
        std::io::stdin()
            .take(canopa_portal::process::MAX_MESSAGE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        anyhow::ensure!(
            bytes.len() <= canopa_portal::process::MAX_MESSAGE_BYTES,
            "Dialog request too large"
        );
        let source = match args.canopa_config {
            Some(path) => Request::common(path),
            None if args.defaults => Request::Defaults,
            None => Request::Discover,
        };
        let configuration = SharedConfiguration::load(&source)?;
        let output = if args.screenshot_window_picker {
            let input = serde_json::from_slice(&bytes)?;
            serde_json::to_vec(&canopa_portal::screencast_ui::run_screenshot(
                input,
                configuration.appearance,
            )?)?
        } else if args.remote_desktop_dialog {
            let input: canopa_portal::remote_desktop::DialogInput = serde_json::from_slice(&bytes)?;
            serde_json::to_vec(&canopa_portal::screencast_ui::run_remote(
                input.capture,
                input.devices,
                input.clipboard,
                configuration.appearance,
            )?)?
        } else {
            let input = serde_json::from_slice(&bytes)?;
            serde_json::to_vec(&canopa_portal::screencast_ui::run(
                input,
                configuration.appearance,
            )?)?
        };
        std::io::stdout().write_all(&output)?;
        Ok(())
    }
    #[cfg(not(feature = "desktop"))]
    {
        let _ = args;
        anyhow::bail!("ScreenCast dialogs require the desktop feature")
    }
}

fn screenshot_dialog(args: Args) -> anyhow::Result<()> {
    #[cfg(feature = "desktop")]
    {
        use std::io::{Read, Write};
        let mut bytes = Vec::new();
        std::io::stdin()
            .take(canopa_portal::process::MAX_MESSAGE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        anyhow::ensure!(
            bytes.len() <= canopa_portal::process::MAX_MESSAGE_BYTES,
            "Dialog request too large"
        );
        let input = serde_json::from_slice(&bytes)?;
        let source = match args.canopa_config {
            Some(path) => Request::common(path),
            None if args.defaults => Request::Defaults,
            None => Request::Discover,
        };
        let configuration = SharedConfiguration::load(&source)?;
        let selected = canopa_portal::screenshot_ui::run(input, configuration.appearance)?;
        std::io::stdout().write_all(&serde_json::to_vec(&selected)?)?;
        Ok(())
    }
    #[cfg(not(feature = "desktop"))]
    {
        let _ = args;
        anyhow::bail!("Screenshot dialogs require the desktop feature")
    }
}

fn wallpaper_dialog(args: Args) -> anyhow::Result<()> {
    #[cfg(feature = "desktop")]
    {
        use std::io::{Read, Write};
        let mut bytes = Vec::new();
        std::io::stdin()
            .take(canopa_portal::process::MAX_MESSAGE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        anyhow::ensure!(
            bytes.len() <= canopa_portal::process::MAX_MESSAGE_BYTES,
            "Dialog request too large"
        );
        let input = serde_json::from_slice(&bytes)?;
        let source = match args.canopa_config {
            Some(path) => Request::common(path),
            None if args.defaults => Request::Defaults,
            None => Request::Discover,
        };
        let configuration = SharedConfiguration::load(&source)?;
        let accepted = canopa_portal::wallpaper_ui::run(input, configuration.appearance)?;
        std::io::stdout().write_all(&serde_json::to_vec(&accepted)?)?;
        Ok(())
    }
    #[cfg(not(feature = "desktop"))]
    {
        let _ = args;
        anyhow::bail!("Wallpaper dialogs require the desktop feature")
    }
}

fn dynamic_launcher_dialog(args: Args) -> anyhow::Result<()> {
    #[cfg(feature = "desktop")]
    {
        use std::io::{Read, Write};
        let mut bytes = Vec::new();
        std::io::stdin()
            .take(canopa_portal::process::MAX_MESSAGE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        anyhow::ensure!(
            bytes.len() <= canopa_portal::process::MAX_MESSAGE_BYTES,
            "Dialog request too large"
        );
        let input = serde_json::from_slice(&bytes)?;
        let source = match args.canopa_config {
            Some(path) => Request::common(path),
            None if args.defaults => Request::Defaults,
            None => Request::Discover,
        };
        let configuration = SharedConfiguration::load(&source)?;
        let name = canopa_portal::dynamic_launcher_ui::run(input, configuration.appearance)?;
        std::io::stdout().write_all(&serde_json::to_vec(&name)?)?;
        Ok(())
    }
    #[cfg(not(feature = "desktop"))]
    {
        let _ = args;
        anyhow::bail!("Launcher dialogs require the desktop feature")
    }
}

fn background_dialog(args: Args) -> anyhow::Result<()> {
    #[cfg(feature = "desktop")]
    {
        use std::io::{Read, Write};
        let mut bytes = Vec::new();
        std::io::stdin()
            .take(canopa_portal::process::MAX_MESSAGE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        anyhow::ensure!(
            bytes.len() <= canopa_portal::process::MAX_MESSAGE_BYTES,
            "Dialog request too large"
        );
        let request = serde_json::from_slice(&bytes)?;
        let source = match args.canopa_config {
            Some(path) => Request::common(path),
            None if args.defaults => Request::Defaults,
            None => Request::Discover,
        };
        let configuration = SharedConfiguration::load(&source)?;
        let activation_token = std::env::var("XDG_ACTIVATION_TOKEN")
            .ok()
            .filter(|token| !token.is_empty() && token.len() <= 4096 && !token.contains('\0'));
        let decision =
            canopa_portal::background_ui::run(request, configuration.appearance, activation_token)?;
        std::io::stdout().write_all(&serde_json::to_vec(&decision)?)?;
        Ok(())
    }
    #[cfg(not(feature = "desktop"))]
    {
        let _ = args;
        anyhow::bail!("Background review dialogs require the desktop feature")
    }
}

fn account_dialog(args: Args) -> anyhow::Result<()> {
    #[cfg(feature = "desktop")]
    {
        use std::io::{Read, Write};
        let mut bytes = Vec::new();
        std::io::stdin()
            .take(canopa_portal::process::MAX_MESSAGE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        anyhow::ensure!(
            bytes.len() <= canopa_portal::process::MAX_MESSAGE_BYTES,
            "Dialog request too large"
        );
        let request = serde_json::from_slice(&bytes)?;
        let source = match args.canopa_config {
            Some(path) => Request::common(path),
            None if args.defaults => Request::Defaults,
            None => Request::Discover,
        };
        let configuration = SharedConfiguration::load(&source)?;
        let information = canopa_portal::account_ui::run(request, configuration.appearance)?;
        std::io::stdout().write_all(&serde_json::to_vec(&information)?)?;
        Ok(())
    }
    #[cfg(not(feature = "desktop"))]
    {
        let _ = args;
        anyhow::bail!("Account dialogs require the desktop feature")
    }
}

fn access_dialog(args: Args) -> anyhow::Result<()> {
    #[cfg(feature = "desktop")]
    {
        use std::io::{Read, Write};
        let mut bytes = Vec::new();
        std::io::stdin()
            .take(canopa_portal::process::MAX_MESSAGE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        anyhow::ensure!(
            bytes.len() <= canopa_portal::process::MAX_MESSAGE_BYTES,
            "Dialog request too large"
        );
        let request = serde_json::from_slice(&bytes)?;
        let source = match args.canopa_config {
            Some(path) => Request::common(path),
            None if args.defaults => Request::Defaults,
            None => Request::Discover,
        };
        let configuration = SharedConfiguration::load(&source)?;
        let selection = canopa_portal::access_ui::run(request, configuration.appearance)?;
        std::io::stdout().write_all(&serde_json::to_vec(&selection)?)?;
        Ok(())
    }
    #[cfg(not(feature = "desktop"))]
    {
        let _ = args;
        anyhow::bail!("Access dialogs require the desktop feature")
    }
}

fn usb_dialog(args: Args) -> anyhow::Result<()> {
    #[cfg(feature = "desktop")]
    {
        use std::io::{Read, Write};
        let mut bytes = Vec::new();
        std::io::stdin()
            .take(canopa_portal::process::MAX_MESSAGE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        anyhow::ensure!(
            bytes.len() <= canopa_portal::process::MAX_MESSAGE_BYTES,
            "USB consent request too large"
        );
        let request = serde_json::from_slice(&bytes)?;
        let source = match args.canopa_config {
            Some(path) => Request::common(path),
            None if args.defaults => Request::Defaults,
            None => Request::Discover,
        };
        let configuration = SharedConfiguration::load(&source)?;
        let grant = canopa_portal::usb_ui::run(request, configuration.appearance)?;
        std::io::stdout().write_all(&serde_json::to_vec(&grant)?)?;
        Ok(())
    }
    #[cfg(not(feature = "desktop"))]
    {
        let _ = args;
        anyhow::bail!("USB dialogs require the desktop feature")
    }
}

fn global_shortcuts_dialog(args: Args) -> anyhow::Result<()> {
    #[cfg(feature = "desktop")]
    {
        use canopa_portal::global_shortcuts::editor::{
            Feedback, MAX_FRAME, Request as DialogRequest,
        };
        use std::io::{BufRead, Read};
        fn frame<T: serde::de::DeserializeOwned>(input: &mut impl BufRead) -> anyhow::Result<T> {
            let mut bytes = Vec::new();
            Read::take(input, MAX_FRAME as u64 + 1).read_until(b'\n', &mut bytes)?;
            anyhow::ensure!(
                bytes.len() <= MAX_FRAME && bytes.last() == Some(&b'\n'),
                "Invalid shortcut dialog frame"
            );
            Ok(serde_json::from_slice(&bytes)?)
        }
        let mut input = std::io::BufReader::new(std::io::stdin());
        let request: DialogRequest = frame(&mut input)?;
        let source = match args.canopa_config {
            Some(path) => Request::common(path),
            None if args.defaults => Request::Defaults,
            None => Request::Discover,
        };
        let configuration = SharedConfiguration::load(&source)?;
        let (sender, updates) = tokio::sync::watch::channel(None);
        std::thread::Builder::new()
            .name("shortcut-dialog-controls".into())
            .spawn(move || {
                while let Ok(update) = frame::<Feedback>(&mut input) {
                    sender.send_replace(Some(update));
                }
            })?;
        canopa_portal::global_shortcuts_ui::run(request, configuration.appearance, updates)
    }
    #[cfg(not(feature = "desktop"))]
    {
        let _ = args;
        anyhow::bail!("Shortcut dialogs require the desktop feature")
    }
}

fn app_chooser(args: Args) -> anyhow::Result<()> {
    #[cfg(feature = "desktop")]
    {
        use std::io::Write;
        let mut input = std::io::BufReader::new(std::io::stdin());
        let request: canopa_portal::app_chooser::AppRequest =
            canopa_portal::process::read_frame(&mut input)?;
        let source = match args.canopa_config {
            Some(path) => Request::common(path),
            None if args.defaults => Request::Defaults,
            None => Request::Discover,
        };
        let configuration = SharedConfiguration::load(&source)?;
        let (sender, receiver) = tokio::sync::watch::channel(Some(request.choices.clone()));
        std::thread::Builder::new()
            .name("app-chooser-updates".into())
            .spawn(move || {
                loop {
                    match canopa_portal::process::read_frame::<Vec<String>>(&mut input) {
                        Ok(choices) => {
                            sender.send_replace(Some(choices));
                        }
                        Err(_) => {
                            sender.send_replace(None);
                            break;
                        }
                    }
                }
            })?;
        let selection =
            canopa_portal::app_chooser_ui::run(request, configuration.appearance, receiver)?;
        std::io::stdout().write_all(&serde_json::to_vec(&selection)?)?;
        Ok(())
    }
    #[cfg(not(feature = "desktop"))]
    {
        let _ = args;
        anyhow::bail!("AppChooser requires the desktop feature")
    }
}

fn chooser(args: Args) -> anyhow::Result<()> {
    #[cfg(feature = "desktop")]
    {
        use std::io::{Read, Write};
        let mut bytes = Vec::new();
        std::io::stdin()
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        anyhow::ensure!(bytes.len() <= 1024 * 1024, "Chooser request too large");
        let input = serde_json::from_slice(&bytes)?;
        let source = match args.canopa_config {
            Some(path) => Request::common(path),
            None if args.defaults => Request::Defaults,
            None => Request::Discover,
        };
        let configuration = SharedConfiguration::load(&source)?;
        let selection = canopa_portal::chooser_ui::run(input, configuration.appearance)?;
        std::io::stdout().write_all(&serde_json::to_vec(&selection)?)?;
        Ok(())
    }
    #[cfg(not(feature = "desktop"))]
    {
        let _ = args;
        anyhow::bail!("FileChooser requires the desktop feature")
    }
}

async fn prepare(request: Request) -> anyhow::Result<SharedConfiguration> {
    Ok(tokio::task::spawn_blocking(move || SharedConfiguration::load(&request)).await??)
}

async fn run(args: Args) -> anyhow::Result<()> {
    #[cfg(not(feature = "desktop"))]
    anyhow::ensure!(
        !args.experimental_print,
        "Experimental Print requires the desktop feature"
    );
    #[cfg(feature = "desktop")]
    let print_store = match &args.print_recovery_store {
        Some(path) => std::path::absolute(path)
            .and_then(|path| canopa_portal::print::file_target::RecoveryStore::new(&path)),
        None => canopa_portal::print::file_target::RecoveryStore::discover(),
    };
    let request = match args.canopa_config {
        Some(path) => Request::common(std::path::absolute(path)?),
        None if args.defaults => Request::Defaults,
        None => Request::Discover,
    };
    // Install signal handlers before acquiring the name: readiness includes reload handling.
    let mut reload = signal(SignalKind::hangup())?;
    let mut terminate = signal(SignalKind::terminate())?;
    let initial = prepare(request.clone()).await?;
    let policy_source = match args.lockdown_policy {
        Some(path) => PolicySource::file(path, args.lockdown_read_only)?,
        None => PolicySource::discover(args.lockdown_read_only)?,
    };
    let lockdown = Lockdown::open(policy_source).await?;
    let arguments = match &request {
        Request::File { path, .. } => vec!["--canopa-config".into(), path.as_os_str().to_owned()],
        Request::Defaults => vec!["--defaults".into()],
        _ => Vec::new(),
    };
    let process = std::sync::Arc::new(ProcessPresenter {
        executable: canopa_file::executable::current()?,
        arguments,
    });
    let requests = Requests::default();
    let notices = std::sync::Arc::new(NotifyPresenter::new(
        zbus::Connection::session().await?,
        process.clone(),
    ));
    let observer = Observer::spawn(std::env::var_os("NIRI_SOCKET").map(PathBuf::from));
    let observed = match &observer {
        Ok(observer) => observer.subscribe(),
        Err(_) => tokio::sync::watch::channel(Observation::unavailable(Unavailable::Worker)).1,
    };
    let background = Background::with_requests(notices.clone(), observed, requests.clone());
    let background_changes = background.changes();
    let folder_history = if args.no_file_chooser_history {
        canopa_portal::file_chooser::history::History::disabled()
    } else {
        canopa_portal::file_chooser::history::History::discover()
            .unwrap_or_else(|_| canopa_portal::file_chooser::history::History::disabled())
    };
    let chooser = FileChooser::with_history(process.clone(), requests.clone(), folder_history);
    let applications = AppChooser::with_requests(process.clone(), requests.clone());
    let account = Account::with_requests(process.clone(), requests.clone());
    let launcher = DynamicLauncher::with_requests(process.clone(), requests.clone());
    let wallpaper = Wallpaper::with_requests(
        std::sync::Arc::new(WallpaperPresenter::new(process.clone())),
        requests.clone(),
    );
    let screenshot = Screenshot::with_requests(
        std::sync::Arc::new(ScreenshotPresenter::new(process.clone())),
        requests.clone(),
    );
    #[cfg(feature = "desktop")]
    let cast_presenter =
        std::sync::Arc::new(canopa_portal::screencast::Production::new(process.clone()));
    #[cfg(not(feature = "desktop"))]
    let cast_presenter = std::sync::Arc::new(canopa_portal::screencast::Unavailable);
    #[cfg(feature = "desktop")]
    let remote_presenter = std::sync::Arc::new(canopa_portal::screencast::remote::Production::new(
        process.clone(),
        cast_presenter.clone(),
    ));
    #[cfg(not(feature = "desktop"))]
    let remote_presenter = std::sync::Arc::new(canopa_portal::screencast::remote::Unavailable);
    let screencast =
        ScreenCast::with_presenters(cast_presenter, remote_presenter, requests.clone());
    let remote_desktop = RemoteDesktop::new(screencast.clone());
    #[cfg(feature = "desktop")]
    let input_presenter: std::sync::Arc<dyn canopa_portal::input_capture::Presenter> =
        std::sync::Arc::new(canopa_portal::input_capture::Production::new(
            process.clone(),
        ));
    #[cfg(not(feature = "desktop"))]
    let input_presenter: std::sync::Arc<dyn canopa_portal::input_capture::Presenter> =
        std::sync::Arc::new(canopa_portal::input_capture::Unavailable);
    let input_capture = canopa_portal::input_capture::InputCapture::with_requests(
        input_presenter,
        requests.clone(),
    );
    let usb = canopa_portal::usb::Usb::with_requests(process.clone(), requests.clone());
    #[cfg(feature = "desktop")]
    let shortcut_presenter: std::sync::Arc<
        dyn canopa_portal::global_shortcuts::editor::Presenter,
    > = std::sync::Arc::new(canopa_portal::global_shortcuts::editor::Production::new(
        process.clone(),
    ));
    #[cfg(not(feature = "desktop"))]
    let shortcut_presenter: std::sync::Arc<
        dyn canopa_portal::global_shortcuts::editor::Presenter,
    > = std::sync::Arc::new(canopa_portal::global_shortcuts::editor::Unavailable);
    let shortcuts = canopa_portal::global_shortcuts::GlobalShortcuts::with_requests(
        std::sync::Arc::new(canopa_portal::global_shortcuts::Niri::default()),
        shortcut_presenter,
        canopa_portal::global_shortcuts::history::History::discover()?,
        requests.clone(),
    );
    #[cfg(feature = "desktop")]
    let print = if args.experimental_print {
        let store = print_store
            .as_ref()
            .map_err(|error| anyhow::anyhow!("Print recovery location is unavailable: {error}"))?;
        Some(canopa_portal::print::Print::with_requests(
            std::sync::Arc::new(canopa_portal::print::Production::new(
                process.clone(),
                store.clone(),
            )),
            lockdown.clone(),
            requests.clone(),
        ))
    } else {
        None
    };
    let access = Access::with_requests(process.clone(), requests);
    let builder = zbus::connection::Builder::session()?
        .serve_at(OBJECT_PATH, Settings::new(initial.appearance))?
        .serve_at(OBJECT_PATH, chooser)?
        .serve_at(OBJECT_PATH, applications)?
        .serve_at(OBJECT_PATH, access)?
        .serve_at(OBJECT_PATH, usb)?
        .serve_at(OBJECT_PATH, shortcuts.clone())?
        .serve_at(OBJECT_PATH, account)?
        .serve_at(OBJECT_PATH, background)?
        .serve_at(OBJECT_PATH, launcher)?
        .serve_at(OBJECT_PATH, wallpaper)?
        .serve_at(OBJECT_PATH, screenshot.clone())?
        .serve_at(OBJECT_PATH, screencast.clone())?
        .serve_at(OBJECT_PATH, remote_desktop.clone())?
        .serve_at(OBJECT_PATH, input_capture.clone())?
        .serve_at(
            OBJECT_PATH,
            canopa_portal::clipboard::Clipboard::with_input_capture(
                screencast.clone(),
                input_capture.clone(),
            ),
        )?
        .serve_at(OBJECT_PATH, lockdown.clone())?
        .name(BUS_NAME)?
        .allow_name_replacements(false)
        .replace_existing_names(false);
    #[cfg(feature = "desktop")]
    let builder = match &print {
        Some(print) => builder.serve_at(OBJECT_PATH, print.clone())?.serve_at(
            OBJECT_PATH,
            canopa_portal::print::PrinterJobs(print.clone()),
        )?,
        None => builder,
    };
    let connection = builder.build().await?;
    let screenshot_targets = screenshot.observe_available_targets(&connection).await?;
    let available_devices = remote_desktop
        .observe_available_devices(&connection)
        .await?;
    let policy = lockdown.clone();
    let policy_connection = connection.clone();
    let mut lockdown_relay = tokio::spawn(async move { policy.relay(policy_connection).await });
    let background_relay = tokio::spawn(Background::relay_changes(
        background_changes,
        connection.clone(),
    ));
    eprintln!(
        "level=info component=settings_portal event=ready fingerprint={}",
        initial.source.fingerprint
    );
    #[cfg(feature = "desktop")]
    let recovery = {
        match print_store {
            Ok(store) => Some(canopa_portal::print::RecoveryHost::start(process, store)),
            Err(error) => {
                eprintln!(
                    "level=warn component=print_recovery event=location_unavailable error={error}"
                );
                None
            }
        }
    };
    let mut messages = zbus::MessageStream::from(&connection);
    loop {
        tokio::select! {
            message = messages.next() => {
                if !matches!(message, Some(Ok(_))) { break; }
            },
            _ = terminate.recv() => break,
            _ = &mut lockdown_relay => {
                eprintln!("level=warn component=lockdown_portal event=publication_stopped");
                break;
            },
            result = tokio::signal::ctrl_c() => { result?; break; },
            _ = reload.recv() => {
                let candidate = prepare(request.clone()).await;
                if let Err(error) = lockdown.reload().await {
                    eprintln!("level=warn component=lockdown_portal event=policy_reload_rejected error={error}");
                }
                match candidate {
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
    available_devices.stop().await;
    screenshot_targets.stop().await;
    #[cfg(feature = "desktop")]
    if let Some(print) = print {
        print.shutdown().await;
    }
    #[cfg(feature = "desktop")]
    if let Some(recovery) = recovery {
        recovery.shutdown().await;
    }
    if shortcuts.shutdown().await.is_err() {
        eprintln!("level=warn component=global_shortcuts event=shutdown_unconfirmed");
    }
    if input_capture.shutdown().await.is_err() {
        eprintln!("level=warn component=input_capture event=cleanup_unconfirmed");
    }
    if screencast.shutdown().await.is_err() {
        eprintln!("level=warn component=screencast_portal event=shutdown_unconfirmed");
    }
    background_relay.abort();
    drop(observer);
    if lockdown.shutdown().await.is_err() {
        eprintln!("level=warn component=lockdown_portal event=shutdown_unconfirmed");
    }
    lockdown_relay.abort();
    if notices.shutdown().await.is_err() {
        eprintln!("level=warn component=background_portal event=question_shutdown_unconfirmed");
    }
    connection.close().await?;
    Ok(())
}
