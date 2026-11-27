//! Render representative lock-screen states to PNG files for design review.
//!
//! `cargo run -p canopa-session --example lock-gallery -- OUTPUT_DIR [light|dark] [CONFIG]`
//! A CONFIG file (appearance and lockscreen blocks) replaces the built-in look.
//! Locks nothing and starts no authentication.
use canopa_config::{Component, Request};
use canopa_session::{lock_ui, settings::Settings};
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    canopa_i18n::init();
    let mut args = std::env::args_os().skip(1);
    let output = PathBuf::from(args.next().expect("output directory"));
    let light = args.next().is_some_and(|a| a == "light");
    let config = args.next().map(PathBuf::from);
    std::fs::create_dir_all(&output)?;
    let request = if let Some(config) = &config {
        Request::common(config.clone())
    } else if light {
        let source = output.join(".lock-light.kdl");
        std::fs::write(
            &source,
            "schema-version 1\nappearance { color-scheme \"light\"; }\n",
        )?;
        Request::common(source)
    } else {
        Request::Defaults
    };
    let (mut settings, _) = Settings::load(&request, Component::Lockscreen)?;
    // A default session shows the embedded wallpaper, which the lock follows.
    if config.is_none() {
        settings.built_in_wallpaper.insert("gallery".into());
    }
    let fresh = || -> anyhow::Result<lock_ui::LockUi> {
        let mut ui = lock_ui::LockUi::preview(&settings, "Alex Morgan".into())?;
        lock_ui::configure(&mut ui);
        ui.set_avatar_initials(Some("AM".into()));
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
    let now_playing = || canopa_session_ui::view::NowPlaying {
        title: "Weightless".into(),
        artist: "Marconi Union".into(),
        player: "Music".into(),
        playing: true,
    };
    let notices = || Some("Calendar — New notification\nMessages — New notification".to_owned());
    // At rest: the glance view, with what the Shell plays and Notify allows.
    let mut ui = fresh()?;
    ui.resting = true;
    ui.media = Some(now_playing());
    ui.notifications = notices();
    save(&ui, "lock-glance")?;
    let mut ui = fresh()?;
    ui.resting = true;
    save(&ui, "lock-glance-quiet")?;
    let mut ui = fresh()?;
    ui.media = Some(canopa_session_ui::view::NowPlaying {
        playing: false,
        ..now_playing()
    });
    ui.notifications = notices();
    save(&ui, "lock-cards")?;
    save(&fresh()?, "lock")?;
    let mut ui = fresh()?;
    ui.secret.push("correct horse");
    save(&ui, "lock-typing")?;
    ui.toggle_reveal();
    save(&ui, "lock-revealed")?;
    let mut ui = fresh()?;
    ui.failed = true;
    ui.message = "Incorrect password. Try again.".into();
    ui.caps = true;
    save(&ui, "lock-error")?;
    // Motion, as frame sequences at 30 frames per second.
    let background = lock_ui::LockUi::prepare_background(&settings, "gallery", 960, 600)?;
    let clear = lock_ui::LockUi::prepare_rest_background(&settings, "gallery", 960, 600)?;
    let record = |name: &str,
                  ui: &mut lock_ui::LockUi,
                  frames: usize,
                  script: &mut dyn FnMut(usize, &mut lock_ui::LockUi)|
     -> anyhow::Result<()> {
        let directory = output.join(format!("frames-{name}"));
        std::fs::create_dir_all(&directory)?;
        let start = std::time::Instant::now();
        ui.motion().set_clock(Some(start));
        ui.motion().enable();
        for frame in 0..frames {
            script(frame, ui);
            ui.motion().set_clock(Some(
                start + std::time::Duration::from_micros(frame as u64 * 33_333),
            ));
            ui.render_on_output(
                &settings,
                "gallery",
                ui.rest_background(background.clone(), clear.as_ref()),
            )?
            .0
            .save(directory.join(format!("{frame:04}.png")))?;
        }
        Ok(())
    };
    record("lock-appear", &mut fresh()?, 24, &mut |_, _| {})?;
    // Waking: a key press shows the form (and types into it), then the
    // untouched screen settles back.
    let mut ui = fresh()?;
    ui.resting = true;
    ui.media = Some(now_playing());
    ui.notifications = notices();
    record("lock-wake", &mut ui, 60, &mut |frame, ui| {
        if frame == 6 {
            ui.resting = false;
            ui.secret.push("c");
        }
        if (8..16).contains(&frame) && frame % 2 == 0 {
            ui.secret.push("x");
        }
        if frame == 34 {
            ui.secret.clear();
            ui.resting = true;
        }
    })?;
    let mut ui = fresh()?;
    record("lock-failure", &mut ui, 66, &mut |frame, ui| {
        if (2..20).contains(&frame) && frame % 2 == 0 {
            ui.secret.push("x");
        }
        if frame == 22 {
            // The response goes to PAM; the field shows it is being checked.
            ui.prompt = None;
            // As the lock does: the field keeps the response's length while PAM checks.
            let length = ui.secret.text().chars().count();
            ui.secret.clear();
            ui.secret.push(&"x".repeat(length));
            ui.busy = true;
            lock_ui::update_actions(ui, false);
        }
        if frame == 46 {
            // As the lock does on rejection: the field is cleared for the retry.
            ui.secret.clear();
            ui.mask_response();
            ui.busy = false;
            ui.failed = true;
            ui.message = "Incorrect password. Try again.".into();
            ui.prompt = Some(lock_ui::Prompt {
                id: 2,
                text: "Password".into(),
                echo: false,
            });
            lock_ui::update_actions(ui, true);
        }
    })?;
    if light && config.is_none() {
        std::fs::remove_file(output.join(".lock-light.kdl"))?;
    }
    Ok(())
}
