//! Request-scoped USB consent. Inventory, permission storage and device FDs
//! belong to the standard frontend; this backend never opens a device path.
use crate::requests::{Completion, Options, Requests, option};
use futures_util::future::BoxFuture;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, sync::Arc};
use tokio::sync::watch;
use zbus::{
    Connection, fdo,
    message::Header,
    zvariant::{OwnedObjectPath, OwnedValue, Value},
};

#[cfg(test)]
mod tests;

const MAX_DEVICES: usize = 64;
const FIELD_BYTES: usize = 4096;
const MAX_TEXT_BYTES: usize = 64 * 1024;
const MAX_BODY_BYTES: u32 = 128 * 1024;
const PROPERTIES: &[&str] = &[
    "ID_INPUT_JOYSTICK",
    "ID_MODEL_ID",
    "ID_MODEL_ENC",
    "ID_MODEL_FROM_DATABASE",
    "ID_REVISION",
    "ID_SERIAL",
    "ID_SERIAL_SHORT",
    "ID_TYPE",
    "ID_VENDOR_ENC",
    "ID_VENDOR_ID",
    "ID_VENDOR_FROM_DATABASE",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub hardware_id: String,
    pub serial: String,
    pub location: String,
    pub writable: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsbRequest {
    pub app_id: String,
    pub parent_window: String,
    pub devices: Vec<Device>,
}

fn field(value: &str, budget: &mut usize) -> fdo::Result<()> {
    *budget = budget.saturating_add(value.len());
    if value.len() > FIELD_BYTES || value.contains('\0') || *budget > MAX_TEXT_BYTES {
        return Err(fdo::Error::InvalidArgs(
            "USB request text exceeds limits".into(),
        ));
    }
    Ok(())
}
fn dictionary(values: &Options) -> fdo::Result<()> {
    if values.len() > 32 || values.keys().any(|key| key.len() > 128) {
        return Err(fdo::Error::InvalidArgs(
            "USB metadata exceeds limits".into(),
        ));
    }
    Ok(())
}
pub(crate) fn label(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_control() || matches!(ch, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}') {
                ' '
            } else {
                ch
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
fn decoded(value: &str) -> String {
    let mut bytes = Vec::with_capacity(value.len());
    let mut input = value.as_bytes();
    while !input.is_empty() {
        if input.starts_with(b"\\x")
            && input.len() >= 4
            && let (Some(high), Some(low)) = (
                (input[2] as char).to_digit(16),
                (input[3] as char).to_digit(16),
            )
        {
            bytes.push((high * 16 + low) as u8);
            input = &input[4..];
        } else {
            bytes.push(input[0]);
            input = &input[1..];
        }
    }
    label(&String::from_utf8_lossy(&bytes))
}

impl UsbRequest {
    pub fn parse(
        app_id: String,
        parent_window: String,
        devices: Vec<(String, Options, Options)>,
        options: Options,
    ) -> fdo::Result<Self> {
        dictionary(&options)?;
        if devices.is_empty() || devices.len() > MAX_DEVICES {
            return Err(fdo::Error::InvalidArgs("Invalid USB device count".into()));
        }
        let mut budget = 0;
        field(&app_id, &mut budget)?;
        field(&parent_window, &mut budget)?;
        let mut ids = HashSet::new();
        let mut parsed = Vec::with_capacity(devices.len());
        for (id, mut information, mut access) in devices {
            field(&id, &mut budget)?;
            if id.is_empty() || id.len() > 256 || !ids.insert(id.clone()) {
                return Err(fdo::Error::InvalidArgs(
                    "Invalid or repeated USB device ID".into(),
                ));
            }
            dictionary(&information)?;
            dictionary(&access)?;
            let writable = option(&mut access, "writable")?.unwrap_or(false);
            let _: Option<bool> = option(&mut information, "readable")?;
            let _: Option<bool> = option(&mut information, "writable")?;
            let parent: String = option(&mut information, "parent")?.unwrap_or_default();
            let location: String = option(&mut information, "device-file")?.unwrap_or_default();
            field(&parent, &mut budget)?;
            field(&location, &mut budget)?;
            let mut properties: Options =
                option(&mut information, "properties")?.unwrap_or_default();
            dictionary(&properties)?;
            let mut strings = std::collections::HashMap::new();
            for key in PROPERTIES {
                let value: String = option(&mut properties, key)?.unwrap_or_default();
                field(&value, &mut budget)?;
                strings.insert(*key, value);
            }
            let choose = |plain: &str, encoded: &str| {
                let plain = label(&strings[plain]);
                if plain.is_empty() {
                    decoded(&strings[encoded])
                } else {
                    plain
                }
            };
            let name = choose("ID_MODEL_FROM_DATABASE", "ID_MODEL_ENC");
            let vendor = choose("ID_VENDOR_FROM_DATABASE", "ID_VENDOR_ENC");
            let serial = [&strings["ID_SERIAL_SHORT"], &strings["ID_SERIAL"]]
                .into_iter()
                .map(|text| label(text))
                .find(|text| !text.is_empty())
                .unwrap_or_default();
            let hardware_id = [&strings["ID_VENDOR_ID"], &strings["ID_MODEL_ID"]];
            let hardware_id = if hardware_id
                .iter()
                .all(|value| value.len() == 4 && value.bytes().all(|b| b.is_ascii_hexdigit()))
            {
                format!("{}:{}", hardware_id[0], hardware_id[1])
            } else {
                String::new()
            };
            parsed.push(Device {
                id,
                name: if name.is_empty() {
                    "USB device".into()
                } else {
                    name
                },
                vendor,
                hardware_id,
                serial,
                location: label(&location),
                writable,
            });
        }
        Ok(Self {
            app_id,
            parent_window,
            devices: parsed,
        })
    }

    pub fn valid(&self) -> bool {
        let mut budget = 0;
        let mut ids = HashSet::new();
        !self.devices.is_empty()
            && self.devices.len() <= MAX_DEVICES
            && field(&self.app_id, &mut budget).is_ok()
            && field(&self.parent_window, &mut budget).is_ok()
            && self.devices.iter().all(|device| {
                !device.id.is_empty()
                    && device.id.len() <= 256
                    && ids.insert(&device.id)
                    && [
                        &device.id,
                        &device.name,
                        &device.vendor,
                        &device.hardware_id,
                        &device.serial,
                        &device.location,
                    ]
                    .into_iter()
                    .all(|value| field(value, &mut budget).is_ok())
            })
    }

    fn response(&self, grant: Grant) -> Option<Options> {
        if grant.devices.len() > self.devices.len() {
            return None;
        }
        let mut seen = HashSet::new();
        let mut values = Vec::with_capacity(grant.devices.len());
        for selected in grant.devices {
            let offered = self
                .devices
                .iter()
                .find(|device| device.id == selected.id)?;
            if !seen.insert(selected.id.clone()) || selected.writable && !offered.writable {
                return None;
            }
            values.push((
                selected.id,
                Options::from([("writable".into(), selected.writable.into())]),
            ));
        }
        Some(Options::from([(
            "devices".into(),
            OwnedValue::try_from(Value::from(values)).ok()?,
        )]))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedDevice {
    pub id: String,
    pub writable: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub devices: Vec<SelectedDevice>,
}
pub trait Presenter: Send + Sync + 'static {
    fn present(
        &self,
        request: UsbRequest,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Option<Grant>>>;
}
pub use crate::process::DialogProcess as ProcessPresenter;
impl Presenter for ProcessPresenter {
    fn present(
        &self,
        request: UsbRequest,
        cancelled: watch::Receiver<bool>,
    ) -> BoxFuture<'_, anyhow::Result<Option<Grant>>> {
        Box::pin(self.exchange("--usb-dialog", request, cancelled))
    }
}
pub struct Usb {
    presenter: Arc<dyn Presenter>,
    requests: Requests,
}
impl Usb {
    pub fn new(presenter: Arc<dyn Presenter>) -> Self {
        Self::with_requests(presenter, Requests::default())
    }
    pub fn with_requests(presenter: Arc<dyn Presenter>, requests: Requests) -> Self {
        Self {
            presenter,
            requests,
        }
    }
}

#[zbus::interface(name = "org.freedesktop.impl.portal.Usb")]
impl Usb {
    #[allow(clippy::too_many_arguments)] // Exact published wire arguments and injected context.
    #[zbus(out_args("response", "results"))]
    async fn acquire_devices(
        &self,
        handle: OwnedObjectPath,
        parent_window: String,
        app_id: String,
        devices: Vec<(String, Options, Options)>,
        options: Options,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<(u32, Options)> {
        if header.unix_fds().unwrap_or(0) != 0 {
            return Err(fdo::Error::InvalidArgs(
                "USB consent takes no file descriptors".into(),
            ));
        }
        if header.primary().body_len() > MAX_BODY_BYTES {
            return Err(fdo::Error::LimitsExceeded(
                "USB request exceeds limits".into(),
            ));
        }
        let request = UsbRequest::parse(app_id, parent_window, devices, options)?;
        let completion = self
            .requests
            .run(connection, header, handle, |_, cancelled| {
                self.presenter.present(request.clone(), cancelled)
            })
            .await?;
        Ok(match completion {
            Completion::Cancelled | Completion::Finished(Ok(None)) => (1, Options::new()),
            Completion::Finished(Ok(Some(grant))) => request
                .response(grant)
                .map(|values| (0, values))
                .unwrap_or((2, Options::new())),
            Completion::Finished(Err(_)) => (2, Options::new()),
        })
    }
    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        1
    }
}
