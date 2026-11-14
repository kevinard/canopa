//! Render representative lock-screen states to PNG files for design review.
//!
//! `cargo run -p canopa-session --example lock-gallery -- OUTPUT_DIR [light]`
//! Locks nothing and starts no authentication.
use canopa_config::{Component, Request};
use canopa_session::{lock_ui, settings::Settings};
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    canopa_i18n::init();
    let mut args = std::env::args_os().skip(1);
    let output = PathBuf::from(args.next().expect("output directory"));
    let light = args.next().is_some_and(|a| a == "light");
    std::fs::create_dir_all(&output)?;
    let request = if light {
        let source = output.join(".lock-light.kdl");
        std::fs::write(
            &source,
            "schema-version 1\nappearance { color-scheme \"light\"; }\n",
        )?;
        Request::common(source)
    } else {
        Request::Defaults
    };
    let (settings, _) = Settings::load(&request, Component::Lockscreen)?;
    let fresh = || -> anyhow::Result<lock_ui::LockUi> {
        let mut ui = lock_ui::LockUi::preview(&settings, "Alex Morgan".into())?;
        lock_ui::configure(&mut ui);
        lock_ui::update_actions(&mut ui, false);
        ui.layout = "English (US)".into();
        ui.can_select_layout = true;
        ui.battery = Some(canopa_widgets::battery::BatteryState {
            percentage: 82.,
            power_state: Default::default(),
            present: true,
        });
        Ok(ui)
    };
    let save = |ui: &lock_ui::LockUi, name: &str| -> anyhow::Result<()> {
        ui.render(&settings, "gallery", 1280, 800)?
            .save_with_format(output.join(format!("{name}.png")), image::ImageFormat::Png)?;
        Ok(())
    };
    save(&fresh()?, "lock")?;
    let mut ui = fresh()?;
    ui.secret.push("correct horse");
    save(&ui, "lock-typing")?;
    ui.toggle_reveal();
    save(&ui, "lock-revealed")?;
    let mut ui = fresh()?;
    ui.failed = true;
    ui.message = "Authentication failed. Try again.".into();
    ui.caps = true;
    save(&ui, "lock-error")?;
    if light {
        std::fs::remove_file(output.join(".lock-light.kdl"))?;
    }
    Ok(())
}
