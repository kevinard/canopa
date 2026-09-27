//! Canopa's Settings backend. UI, application policy and configuration stay with their owners.
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
