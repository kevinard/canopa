//! A fake systemd-localed with `en_US.UTF-8`, `fr_FR.UTF-8` and
//! `en_GB.UTF-8` installed.
use super::TestBus;
use crate::locale::{PATH, SERVICE};
use std::sync::{Arc, Mutex};
use zbus::{fdo, object_server::SignalEmitter};

const INSTALLED: [&str; 3] = ["en_US.UTF-8", "fr_FR.UTF-8", "en_GB.UTF-8"];

struct Localed {
    locale: Arc<Mutex<Vec<String>>>,
    keyboard: Arc<Mutex<(String, bool)>>,
}

#[zbus::interface(name = "org.freedesktop.locale1")]
impl Localed {
    async fn set_locale(
        &self,
        locale: Vec<String>,
        _interactive: bool,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        for line in &locale {
            let value = line.split_once('=').map_or("", |(_, value)| value);
            if !INSTALLED.contains(&value) {
                return Err(fdo::Error::InvalidArgs(format!(
                    "Locale {value} not installed, refusing."
                )));
            }
        }
        *self.locale.lock().unwrap() = locale;
        self.locale_changed(&emitter).await?;
        Ok(())
    }
    #[zbus(name = "SetX11Keyboard")]
    #[allow(clippy::too_many_arguments)] // localed's signature.
    async fn set_x11_keyboard(
        &self,
        layout: &str,
        _model: &str,
        _variant: &str,
        _options: &str,
        convert: bool,
        _interactive: bool,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        *self.keyboard.lock().unwrap() = (layout.into(), convert);
        self.x11_layout_changed(&emitter).await?;
        Ok(())
    }
    #[zbus(property)]
    fn locale(&self) -> Vec<String> {
        self.locale.lock().unwrap().clone()
    }
    #[zbus(property, name = "X11Layout")]
    fn x11_layout(&self) -> String {
        self.keyboard.lock().unwrap().0.clone()
    }
    #[zbus(property, name = "X11Model")]
    fn x11_model(&self) -> &str {
        ""
    }
    #[zbus(property, name = "X11Variant")]
    fn x11_variant(&self) -> &str {
        ""
    }
    #[zbus(property, name = "X11Options")]
    fn x11_options(&self) -> &str {
        ""
    }
    #[zbus(property, name = "VConsoleKeymap")]
    fn vconsole_keymap(&self) -> &str {
        "us"
    }
}

/// localed on a private bus: `LANG=en_US.UTF-8`, layout `us`.
pub struct FakeLocaled {
    pub connection: zbus::Connection,
    /// The X11 layout set last, and whether the console was converted too.
    pub keyboard: Arc<Mutex<(String, bool)>>,
}

impl FakeLocaled {
    pub async fn serve(bus: &TestBus) -> Self {
        let keyboard = Arc::new(Mutex::new(("us".to_owned(), false)));
        let connection = zbus::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .serve_at(
                PATH,
                Localed {
                    locale: Arc::new(Mutex::new(vec!["LANG=en_US.UTF-8".into()])),
                    keyboard: keyboard.clone(),
                },
            )
            .unwrap()
            .name(SERVICE)
            .unwrap()
            .build()
            .await
            .unwrap();
        Self {
            connection,
            keyboard,
        }
    }
}
