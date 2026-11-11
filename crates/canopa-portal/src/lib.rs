//! Canopa's desktop portal backends. The frontend owns sandbox/document grants.
pub mod access;
#[cfg(feature = "desktop")]
pub mod access_ui;
#[cfg(feature = "desktop")]
mod accessibility_ui;
pub mod account;
#[cfg(feature = "desktop")]
mod account_profile;
#[cfg(feature = "desktop")]
pub mod account_ui;
pub mod app_chooser;
#[cfg(feature = "desktop")]
pub mod app_chooser_ui;
pub mod background;
#[cfg(feature = "desktop")]
pub mod background_ui;
#[cfg(feature = "desktop")]
pub mod capture;
mod choices;
#[cfg(feature = "desktop")]
pub mod chooser_ui;
pub mod clipboard;
#[cfg(feature = "desktop")]
pub mod clipboard_transport;
pub mod dynamic_launcher;
#[cfg(feature = "desktop")]
pub mod dynamic_launcher_icon;
#[cfg(feature = "desktop")]
pub mod dynamic_launcher_ui;
pub mod file_chooser;
#[cfg(feature = "desktop")]
mod icons;
pub mod input_capture;
#[cfg(feature = "desktop")]
pub mod input_capture_transport;
#[cfg(feature = "desktop")]
pub mod input_capture_ui;
pub mod lockdown;
pub mod print;
#[cfg(feature = "desktop")]
pub mod print_authentication_ui;
#[cfg(feature = "desktop")]
pub mod print_jobs_ui;
#[cfg(feature = "desktop")]
pub mod print_recovery_ui;
#[cfg(feature = "desktop")]
pub mod print_ui;
pub mod process;
#[cfg(feature = "desktop")]
pub mod remote_input;
pub mod requests;
pub mod screencast;
pub use screencast::remote as remote_desktop;
pub mod global_shortcuts;
#[cfg(feature = "desktop")]
pub mod global_shortcuts_ui;
pub mod screencast_capture;
#[cfg(feature = "desktop")]
pub mod screencast_status;
#[cfg(feature = "desktop")]
pub mod screencast_ui;
pub mod screenshot;
#[cfg(feature = "desktop")]
mod screenshot_image;
#[cfg(feature = "desktop")]
pub mod screenshot_ui;
pub mod usb;
#[cfg(feature = "desktop")]
pub mod usb_ui;
pub mod wallpaper;
#[cfg(feature = "desktop")]
mod wallpaper_image;
#[cfg(feature = "desktop")]
pub mod wallpaper_ui;
#[cfg(feature = "desktop")]
mod worker;
use canopa_theme::{Appearance, ColorScheme, Rgb};
use std::collections::HashMap;
use zbus::{
    object_server::SignalEmitter,
    zvariant::{OwnedValue, Value},
};

pub const BUS_NAME: &str = "org.freedesktop.impl.portal.desktop.canopa";
pub const OBJECT_PATH: &str = "/org/freedesktop/portal/desktop";
pub const NAMESPACE: &str = "org.freedesktop.appearance";

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.freedesktop.portal.Error")]
pub enum SettingsError {
    NotFound(String),
}

/// A complete read-only snapshot. Installation is serialized by the object server.
pub struct Settings {
    appearance: Appearance,
}

impl Settings {
    pub fn new(appearance: Appearance) -> Self {
        Self { appearance }
    }

    fn values(&self) -> HashMap<&'static str, OwnedValue> {
        let Rgb(r, g, b) = self.appearance.palette().accent;
        HashMap::from([
            (
                "color-scheme",
                OwnedValue::from(match self.appearance.color_scheme {
                    ColorScheme::Dark => 1_u32,
                    ColorScheme::Light => 2_u32,
                }),
            ),
            (
                "accent-color",
                Value::from((
                    f64::from(r) / 255.0,
                    f64::from(g) / 255.0,
                    f64::from(b) / 255.0,
                ))
                .try_into()
                .expect("RGB value"),
            ),
            // 0 means no preference; Canopa's high-contrast palette is 1 (high).
            (
                "contrast",
                OwnedValue::from(u32::from(self.appearance.high_contrast())),
            ),
            // 0 means no preference, 1 asks applications to reduce motion.
            (
                "reduced-motion",
                OwnedValue::from(u32::from(self.appearance.reduced_motion())),
            ),
        ])
    }

    /// Install before emitting; unchanged effective values produce no signals.
    pub async fn install(
        &mut self,
        appearance: Appearance,
        emitter: &SignalEmitter<'_>,
    ) -> zbus::Result<()> {
        let old = self.values();
        self.appearance = appearance;
        let mut changed: Vec<_> = self
            .values()
            .into_iter()
            .filter(|(key, value)| old.get(key) != Some(value))
            .collect();
        changed.sort_by_key(|(key, _)| *key);
        for (key, value) in changed {
            Self::setting_changed(emitter, NAMESPACE, key, value).await?;
        }
        Ok(())
    }
}

#[zbus::interface(name = "org.freedesktop.impl.portal.Settings")]
impl Settings {
    fn read_all(
        &self,
        namespaces: Vec<String>,
    ) -> HashMap<&'static str, HashMap<&'static str, OwnedValue>> {
        let matches = namespaces.is_empty()
            || namespaces.iter().any(|pattern| {
                pattern.is_empty()
                    || pattern == NAMESPACE
                    || pattern == "*"
                    || pattern.strip_suffix(".*").is_some_and(|prefix| {
                        NAMESPACE
                            .strip_prefix(prefix)
                            .is_some_and(|rest| rest.starts_with('.'))
                    })
            });
        if matches {
            HashMap::from([(NAMESPACE, self.values())])
        } else {
            HashMap::new()
        }
    }

    fn read(&self, namespace: &str, key: &str) -> Result<OwnedValue, SettingsError> {
        if namespace == NAMESPACE
            && let Some(value) = self.values().remove(key)
        {
            return Ok(value);
        }
        Err(SettingsError::NotFound(
            "Requested setting not found".into(),
        ))
    }

    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        1
    }

    #[zbus(signal)]
    async fn setting_changed(
        emitter: &SignalEmitter<'_>,
        namespace: &str,
        key: &str,
        value: OwnedValue,
    ) -> zbus::Result<()>;
}
