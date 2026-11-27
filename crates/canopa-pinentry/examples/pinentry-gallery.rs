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
                error: "Bad Passphrase (try 2 of 3)".into(),
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
                // GnuPG asks for a quality bar for new passphrases.
                quality: Some("Quality:".into()),
                ok: "OK".into(),
                ..Default::default()
            },
        )?,
        "pinentry-new",
    )?;
    let mut c = Controller::new(
        &settings,
        Pin {
            kind: Kind::Pin,
            description: "Please enter the passphrase to protect your new key".into(),
            repeat: Some("Repeat:".into()),
            quality: Some("Quality:".into()),
            ok: "OK".into(),
            ..Default::default()
        },
    )?;
    c.input("correct horse battery");
    c.quality_query();
    c.inquiry_result(
        c.revision(),
        canopa_pinentry::agent::Answer::Quality(Some(72)),
    );
    c.refresh();
    save(&c, "pinentry-strength")?;
    // Motion: strength feedback while a new passphrase is typed (30 fps frames).
    let mut c = Controller::new(
        &settings,
        Pin {
            kind: Kind::Pin,
            description: "Please enter the passphrase to protect your new key".into(),
            repeat: Some("Repeat:".into()),
            quality: Some("Quality:".into()),
            ..Default::default()
        },
    )?;
    let directory = output.join("frames-pinentry-strength");
    std::fs::create_dir_all(&directory)?;
    let start = std::time::Instant::now();
    c.ui.motion().enable();
    let phrase = "correct horse battery staple";
    for frame in 0..66usize {
        // The empty dialog first, with its meter already in place.
        let step = frame.wrapping_sub(6);
        if frame >= 6 && step % 2 == 0 && step / 2 < phrase.len() {
            c.input(&phrase[step / 2..step / 2 + 1]);
            c.quality_query();
            let typed = (step / 2 + 1) as i16;
            c.inquiry_result(
                c.revision(),
                canopa_pinentry::agent::Answer::Quality(Some((typed * 4).min(100))),
            );
            c.refresh();
        }
        c.ui.motion().set_clock(Some(
            start + std::time::Duration::from_micros(frame as u64 * 33_333),
        ));
        let (width, height) = (
            DIALOG_SIZE.0,
            c.ui.dialog_height(DIALOG_MIN_HEIGHT, DIALOG_MAX_HEIGHT),
        );
        c.ui.render_dialog(&settings, width, height, 1.)?
            .0
            .save(directory.join(format!("{frame:04}.png")))?;
    }
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
