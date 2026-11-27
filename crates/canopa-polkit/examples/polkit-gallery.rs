//! Render representative authorization-dialog states to PNG files for design review.
//!
//! `cargo run -p canopa-polkit --example polkit-gallery -- OUTPUT_DIR [light]`
//! Registers no agent and contacts no authority.
use canopa_config::Request;
use canopa_polkit::{controller::Controller, native::Event};
use canopa_session_ui::view::{AUTHORIZATION_DIALOG_SIZE, DIALOG_MAX_HEIGHT, DIALOG_MIN_HEIGHT};
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    canopa_i18n::init();
    let mut args = std::env::args_os().skip(1);
    let output = PathBuf::from(args.next().expect("output directory"));
    let light = args.next().is_some_and(|a| a == "light");
    std::fs::create_dir_all(&output)?;
    let request = if light {
        let source = output.join(".polkit-light.kdl");
        std::fs::write(
            &source,
            "schema-version 1\nappearance { color-scheme \"light\"; }\n",
        )?;
        Request::common(source)
    } else {
        Request::Defaults
    };
    let (settings, _) = canopa_polkit::config::load(&request)?;
    let save = |c: &Controller, name: &str| -> anyhow::Result<()> {
        let (width, height) = (
            AUTHORIZATION_DIALOG_SIZE.0,
            c.ui.dialog_height(DIALOG_MIN_HEIGHT, DIALOG_MAX_HEIGHT),
        );
        c.ui.render_dialog(&settings, width, height, 1.)?
            .0
            .save_with_format(output.join(format!("{name}.png")), image::ImageFormat::Png)?;
        Ok(())
    };
    let c = Controller::preview(&settings)?;
    save(&c, "polkit")?;
    let mut c = Controller::preview(&settings)?;
    c.input("hunter2");
    save(&c, "polkit-typing")?;
    let mut c = Controller::preview(&settings)?;
    c.event(Event::Failed { generation: 1 });
    c.refresh();
    save(&c, "polkit-failed")?;
    let mut c = Controller::preview(&settings)?;
    c.details();
    save(&c, "polkit-details")?;
    // Motion, as frame sequences at 30 frames per second.
    let record = |name: &str,
                  c: &mut Controller,
                  frames: usize,
                  script: &mut dyn FnMut(usize, &mut Controller)|
     -> anyhow::Result<()> {
        let directory = output.join(format!("frames-{name}"));
        std::fs::create_dir_all(&directory)?;
        let start = std::time::Instant::now();
        c.ui.motion().set_clock(Some(start));
        c.ui.motion().enable();
        for frame in 0..frames {
            script(frame, c);
            c.ui.motion().set_clock(Some(
                start + std::time::Duration::from_micros(frame as u64 * 33_333),
            ));
            c.ui.render_dialog(
                &settings,
                AUTHORIZATION_DIALOG_SIZE.0,
                c.ui.dialog_height(DIALOG_MIN_HEIGHT, DIALOG_MAX_HEIGHT),
                1.,
            )?
            .0
            .save(directory.join(format!("{frame:04}.png")))?;
        }
        Ok(())
    };
    let mut c = Controller::preview(&settings)?;
    record("polkit-failure", &mut c, 72, &mut |frame, c| {
        if (2..20).contains(&frame) && frame % 2 == 0 {
            c.input("x");
        }
        if frame == 22 {
            c.submit();
        }
        if frame == 46 {
            let generation = c.generation;
            c.event(Event::Failed { generation });
        }
    })?;
    if light {
        std::fs::remove_file(output.join(".polkit-light.kdl"))?;
    }
    Ok(())
}
