//! Render representative login-screen states to PNG files for design review.
//!
//! `cargo run -p canopa-greeter --example greeter-gallery -- OUTPUT_DIR [light]`
//! Uses a fixed sample configuration with two accounts, one desktop, power
//! actions and a switchable keyboard layout. Starts no compositor, greetd or
//! account lookup.
use canopa_config::Request;
use canopa_greeter::{
    accounts::Account,
    config::Config,
    controller::{Choice, Controller},
    power::Capabilities,
    protocol::{Event, Kind},
};
use std::path::PathBuf;

const CONFIG: &str = r#"schema-version 1
appearance { color-scheme "SCHEME"; }
greeter {
    discover-users #false
    discover-sessions #false
    session "canopa" {
        label "Canopa"
        command "/bin/true"
    }
    session "recovery" {
        label "Recovery session"
        command "/bin/true"
    }
    view {
        widgets "avatar" "keyboard" "battery"
        auth-timeout 120
        dim 65
        layout {
            margin 42
            gap 18
            center "clock" "date" "avatar" "authentication" "keyboard"
            bottom-center "battery" flow="row"
        }
    }
}
"#;

fn main() -> anyhow::Result<()> {
    canopa_i18n::init();
    let mut args = std::env::args_os().skip(1);
    let output = PathBuf::from(args.next().expect("output directory"));
    let scheme = if args.next().is_some_and(|a| a == "light") {
        "light"
    } else {
        "dark"
    };
    std::fs::create_dir_all(&output)?;
    let source = output.join(format!(".greeter-{scheme}.kdl"));
    std::fs::write(&source, CONFIG.replace("SCHEME", scheme))?;
    let (config, _) = Config::load(&Request::common(source.clone()))?;
    std::fs::remove_file(&source)?;
    let account = |username: &str, name: &str, uid| Account {
        username: username.into(),
        display_name: name.into(),
        uid: Some(uid),
        icon_file: None,
    };
    let fresh = || -> anyhow::Result<Controller> {
        let mut c = Controller::new(&config)?;
        c.power_capabilities = Capabilities {
            shutdown: true,
            restart: true,
        };
        c.apply_accounts(
            &config,
            vec![
                account("alex", "Alex Morgan", 1000),
                account("sam", "Sam Rivera", 1001),
            ],
        );
        c.ui.layout = "English (US)".into();
        c.ui.can_select_layout = true;
        c.ui.set_avatar_initials(Some("AM".into()));
        c.ui.battery = Some(canopa_widgets::battery::BatteryState {
            percentage: 82.,
            power_state: Default::default(),
            present: true,
        });
        c.refresh(&config);
        Ok(c)
    };
    let save = |c: &Controller, name: &str| -> anyhow::Result<()> {
        c.ui.render(&config.view, "gallery", 1280, 800)?
            .save_with_format(output.join(format!("{name}.png")), image::ImageFormat::Png)?;
        Ok(())
    };
    let c = fresh()?;
    save(&c, "greeter")?;
    let mut c = fresh()?;
    c.input("b");
    c.refresh(&config);
    save(&c, "greeter-username")?;

    let mut c = fresh()?;
    let generation = match c.submit() {
        Some(canopa_greeter::protocol::Command::Begin { generation, .. }) => generation,
        _ => anyhow::bail!("sample sign-in did not begin"),
    };
    let prompt = |c: &mut Controller| {
        c.event(
            Event {
                generation,
                kind: Kind::Prompt {
                    text: "Password:".into(),
                    echo: false,
                },
            },
            &config,
        );
    };
    prompt(&mut c);
    c.refresh(&config);
    save(&c, "greeter-password")?;
    c.input("correct horse");
    c.refresh(&config);
    save(&c, "greeter-typing")?;
    c.event(
        Event {
            generation,
            kind: Kind::Message {
                text: "Authentication failed".into(),
                error: true,
            },
        },
        &config,
    );
    prompt(&mut c);
    c.ui.caps = true;
    c.refresh(&config);
    save(&c, "greeter-error")?;

    let mut c = fresh()?;
    c.open_chooser(Choice::Account);
    c.refresh(&config);
    save(&c, "greeter-accounts")?;
    let mut c = fresh()?;
    c.open_chooser(Choice::Desktop);
    c.refresh(&config);
    save(&c, "greeter-desktops")?;
    let mut c = fresh()?;
    c.power_action(20);
    c.refresh(&config);
    save(&c, "greeter-power")?;
    Ok(())
}
