//! Render representative PIN/passphrase dialog states to PNG files for design review.
//!
//! `cargo run -p canopa-pinentry --example pinentry-gallery -- OUTPUT_DIR [light]`
//! Talks to no agent.
use canopa_config::Request;
use canopa_pinentry::{
    controller::Controller,
    protocol::{Kind, Request as Pin},
};
use canopa_session_ui::view::{DIALOG_MAX_HEIGHT, DIALOG_MIN_HEIGHT, DIALOG_SIZE};
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    canopa_i18n::init();
    let mut args = std::env::args_os().skip(1);
    let output = PathBuf::from(args.next().expect("output directory"));
    let light = args.next().is_some_and(|a| a == "light");
    std::fs::create_dir_all(&output)?;
    let request = if light {
        let source = output.join(".pinentry-light.kdl");
        std::fs::write(
            &source,
            "schema-version 1\nappearance { color-scheme \"light\"; }\n",
        )?;
        Request::common(source)
    } else {
        Request::Defaults
    };
    let (settings, _) = canopa_pinentry::config::load(&request)?;
    let save = |c: &Controller, name: &str| -> anyhow::Result<()> {
        let (width, height) = (
            DIALOG_SIZE.0,
            c.ui.dialog_height(DIALOG_MIN_HEIGHT, DIALOG_MAX_HEIGHT),
        );
        c.ui.render_dialog(&settings, width, height, 1.)?
            .0
            .save_with_format(output.join(format!("{name}.png")), image::ImageFormat::Png)?;
        Ok(())
    };
    // The same request as `canopa-pinentry preview`.
    let unlock = || Pin {
        title: "Alex Morgan · alex@example.org · Ed25519 OpenPGP key".into(),
        description: "Fingerprint\n3CE7 7DD6 4F0A 2C50 27D5 45F4 C6C4 1BA5 2B00 921E".into(),
        ok: "Unlock key".into(),
        requester: Some("git".into()),
        ..Default::default()
    };
    save(&Controller::new(&settings, unlock())?, "pinentry")?;
    let mut c = Controller::new(&settings, unlock())?;
    c.input("correct horse battery");
    save(&c, "pinentry-typing")?;
    save(
        &Controller::new(
            &settings,
            Pin {
                error: "Bad passphrase".into(),
                ..unlock()
            },
        )?,
        "pinentry-error",
    )?;
    save(
        &Controller::new(
            &settings,
            Pin {
                kind: Kind::Pin,
                description: "Please enter the passphrase to protect your new key".into(),
                repeat: Some("Repeat:".into()),
                ok: "OK".into(),
                ..Default::default()
            },
        )?,
        "pinentry-new",
    )?;
    save(
        &Controller::new(
            &settings,
            Pin {
                kind: Kind::Confirm,
                description: "Do you really want to permanently delete the OpenPGP secret key \"Alex Morgan\"?".into(),
                ok: "Delete key".into(),
                ..Default::default()
            },
        )?,
        "pinentry-confirm",
    )?;
    if light {
        std::fs::remove_file(output.join(".pinentry-light.kdl"))?;
    }
    Ok(())
}
