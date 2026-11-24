use super::{Action, Binding, EXPIRY_OPTION, Identity, NotificationInput};
use canopa_notify::{
    NotificationAction, NotificationBodyFormat, NotificationSound, NotificationTimeout,
    NotificationVisualReference, Urgency,
};
use std::collections::HashMap;
use std::os::fd::AsFd;
use std::os::unix::fs::FileExt;
use zbus::zvariant::{OwnedValue, Value};

const ICON_BYTES: usize = 1024 * 1024;

/// Categories with Canopa-specific handling: reply composer, and alarm/call
/// break-through and lock-screen display.
pub(crate) const SUPPORTED_CATEGORIES: &[&str] = &["im.received", "alarm.ringing", "call.incoming"];
/// Button purposes Notify presents: a reply composer and labelled call controls.
pub(crate) const SUPPORTED_PURPOSES: &[&str] = &[
    "im.reply-with-text",
    "call.accept",
    "call.decline",
    "call.hang-up",
    "call.enable-speakerphone",
    "call.disable-speakerphone",
];
const REPLY_PURPOSE: &str = "im.reply-with-text";

/// Notify's label for a button the sender left unlabelled, in the person's language.
fn purpose_label(purpose: &str) -> Option<String> {
    Some(match purpose {
        REPLY_PURPOSE => fl!("notify-portal-reply"),
        "call.accept" => fl!("notify-portal-answer"),
        "call.decline" => fl!("notify-portal-decline"),
        "call.hang-up" => fl!("notify-portal-hang-up"),
        "call.enable-speakerphone" => fl!("notify-portal-speaker-on"),
        "call.disable-speakerphone" => fl!("notify-portal-speaker-off"),
        _ => return None,
    })
}

/// Portal v2 `display-hint` values (both `lock-screen` spellings are accepted).
#[derive(Default)]
struct DisplayHints {
    transient: bool,
    tray: bool,
    persistent: bool,
    hide_on_lock_screen: bool,
    hide_content_on_lock_screen: bool,
    show_as_new: bool,
}

fn display_hints(values: &mut HashMap<String, OwnedValue>) -> zbus::fdo::Result<DisplayHints> {
    let Some(value) = values.remove("display-hint") else {
        return Ok(DisplayHints::default());
    };
    let hints = Vec::<String>::try_from(value).map_err(|_| invalid())?;
    if hints.len() > 16 {
        return Err(invalid());
    }
    let mut parsed = DisplayHints::default();
    for hint in hints {
        match hint.as_str() {
            "transient" => parsed.transient = true,
            "tray" => parsed.tray = true,
            "persistent" => parsed.persistent = true,
            "hide-on-lock-screen" | "hide-on-lockscreen" => parsed.hide_on_lock_screen = true,
            "hide-content-on-lock-screen" | "hide-content-on-lockscreen" => {
                parsed.hide_content_on_lock_screen = true
            }
            "show-as-new" => parsed.show_as_new = true,
            // Unknown hints are advisory and ignored.
            _ => {}
        }
    }
    // The portal documents `transient` with `tray` as a programmer error.
    if parsed.transient && parsed.tray {
        return Err(invalid());
    }
    Ok(parsed)
}
const SOUND_BYTES: usize = 8 * 1024 * 1024;

fn invalid() -> zbus::fdo::Error {
    zbus::fdo::Error::InvalidArgs("invalid portal notification content".into())
}

pub(super) fn wire_budget(values: &HashMap<String, OwnedValue>) -> zbus::fdo::Result<()> {
    let mut bytes = 0;
    let mut nodes = 0;
    for (key, value) in values {
        bytes += key.len();
        bounded(value, &mut bytes, &mut nodes, 0, true)?;
    }
    Ok(())
}

fn bounded(
    value: &Value<'_>,
    bytes: &mut usize,
    nodes: &mut usize,
    depth: usize,
    allow_fd: bool,
) -> zbus::fdo::Result<()> {
    *nodes += 1;
    *bytes += 16;
    if *nodes > 8192 || *bytes > 256 * 1024 || depth > 16 {
        return Err(invalid());
    }
    match value {
        Value::Str(value) => *bytes += value.len(),
        Value::ObjectPath(value) => *bytes += value.as_str().len(),
        Value::Signature(value) => *bytes += value.to_string().len(),
        Value::Array(value) => {
            for item in value.inner() {
                bounded(item, bytes, nodes, depth + 1, allow_fd)?;
            }
        }
        Value::Structure(value) => {
            for item in value.fields() {
                bounded(item, bytes, nodes, depth + 1, allow_fd)?;
            }
        }
        Value::Dict(value) => {
            for (key, item) in value.iter() {
                bounded(key, bytes, nodes, depth + 1, allow_fd)?;
                bounded(item, bytes, nodes, depth + 1, allow_fd)?;
            }
        }
        Value::Value(value) => bounded(value, bytes, nodes, depth + 1, allow_fd)?,
        Value::Fd(_) if !allow_fd => return Err(invalid()),
        _ => {}
    }
    if *bytes > 256 * 1024 {
        return Err(invalid());
    }
    Ok(())
}

fn text(
    values: &HashMap<String, OwnedValue>,
    key: &str,
    max: usize,
) -> zbus::fdo::Result<Option<String>> {
    values
        .get(key)
        .map(|value| {
            let value = <&str>::try_from(value).map_err(|_| invalid())?;
            if value.len() > max {
                return Err(invalid());
            }
            Ok(value.to_string())
        })
        .transpose()
}

fn target(
    values: &mut HashMap<String, OwnedValue>,
    key: &str,
) -> zbus::fdo::Result<Option<OwnedValue>> {
    let Some(value) = values.remove(key) else {
        return Ok(None);
    };
    bounded(&value, &mut 0, &mut 0, 0, false)?;
    Ok(Some(value))
}

#[cfg(test)]
pub(super) fn prepare(
    identity: Identity,
    values: HashMap<String, OwnedValue>,
) -> zbus::fdo::Result<(NotificationInput, Binding)> {
    prepare_at(identity, values, std::time::Instant::now())
}

pub(super) fn prepare_at(
    identity: Identity,
    mut values: HashMap<String, OwnedValue>,
    admitted: std::time::Instant,
) -> zbus::fdo::Result<(NotificationInput, Binding)> {
    wire_budget(&values)?;
    let deadline = values
        .remove(EXPIRY_OPTION)
        .map(|value| {
            let milliseconds = u32::try_from(value).map_err(|_| invalid())?;
            if !(1..=30_000).contains(&milliseconds) {
                return Err(invalid());
            }
            Ok(admitted + std::time::Duration::from_millis(milliseconds.into()))
        })
        .transpose()?;
    let title = text(&values, "title", 8 * 1024)?.unwrap_or_default();
    let (body, body_format) = match text(&values, "markup-body", 64 * 1024)? {
        Some(markup) => (markup, NotificationBodyFormat::PortalMarkup),
        None => (
            text(&values, "body", 64 * 1024)?.unwrap_or_default(),
            NotificationBodyFormat::Plain,
        ),
    };
    let urgency = match text(&values, "priority", 16)?.as_deref() {
        Some("low") => Urgency::Low,
        Some("normal" | "high") | None => Urgency::Normal,
        Some("urgent") => Urgency::Critical,
        _ => return Err(invalid()),
    };
    let category = text(&values, "category", 256)?;
    let display = display_hints(&mut values)?;
    let mut reply = None;
    let mut actions = HashMap::new();
    let default_name = text(&values, "default-action", 4096)?.unwrap_or_default();
    validate_action(&default_name, true)?;
    actions.insert(
        "default".to_string(),
        Action {
            name: default_name,
            target: target(&mut values, "default-action-target")?,
        },
    );
    let mut controls = vec![NotificationAction::new("default", "")];
    if let Some(buttons) = values.remove("buttons") {
        let buttons =
            Vec::<HashMap<String, OwnedValue>>::try_from(buttons).map_err(|_| invalid())?;
        if buttons.len() > 63 {
            return Err(invalid());
        }
        for (index, mut button) in buttons.into_iter().enumerate() {
            let purpose = text(&button, "purpose", 256)?;
            // A known purpose supplies a label; an unknown one degrades to the label.
            let label = text(&button, "label", 4096)?
                .filter(|label| !label.trim().is_empty())
                .or_else(|| purpose.as_deref().and_then(purpose_label));
            let Some(label) = label else {
                continue;
            };
            let name = text(&button, "action", 4096)?.ok_or_else(invalid)?;
            validate_action(&name, false)?;
            let key = format!("portal-button-{index}");
            if purpose.as_deref() == Some(REPLY_PURPOSE) && reply.is_none() {
                reply = Some(canopa_notify::ReplyRequest {
                    action_key: key.clone(),
                    placeholder: None,
                });
            }
            controls.push(NotificationAction::new(&key, label));
            actions.insert(
                key,
                Action {
                    name,
                    target: target(&mut button, "target")?,
                },
            );
        }
    }
    let (app_icon, image_data) = match values.remove("icon") {
        Some(value) => icon(value)?,
        None => (None, None),
    };
    let (suppress_sound, sound) = match values.remove("sound") {
        None => (false, None),
        Some(value) => {
            if let Ok(name) = <&str>::try_from(&value) {
                match name {
                    "default" => (false, None),
                    "silent" => (true, None),
                    _ => return Err(invalid()),
                }
            } else {
                let (kind, value) = serialized(value)?;
                if kind != "file-descriptor" {
                    return Err(invalid());
                }
                let bytes = descriptor(&value, SOUND_BYTES)?;
                if !(bytes.starts_with(b"OggS")
                    || bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WAVE"))
                {
                    return Err(invalid());
                }
                (false, Some(NotificationSound::Encoded(bytes)))
            }
        }
    };
    // Display hints are advisory. Existing Notify delivery/user policy remains
    // authoritative; unknown categories/purposes receive ordinary presentation.
    let mut bytes = identity.app_id.len() + identity.id.len();
    for (key, action) in &actions {
        bytes += key.len() + action.name.len();
        if let Some(target) = &action.target {
            bounded(target, &mut bytes, &mut 0, 0, false)?;
        }
    }
    let input = NotificationInput {
        app_name: canopa_apps::desktop_entry_name(&identity.app_id)
            .map(|name| {
                name.chars()
                    .filter(|ch| !ch.is_control())
                    .take(120)
                    .collect()
            })
            .filter(|name: &String| !name.is_empty())
            .unwrap_or_else(|| identity.app_id.clone()),
        desktop_entry: Some(identity.app_id.clone()),
        summary: title,
        body,
        body_format,
        urgency,
        category,
        actions: controls,
        app_icon,
        image_data,
        sound,
        suppress_sound,
        // Request questions remain runtime-only. This is transport withdrawal,
        // not Notify's display-based attention timeout. `transient` asks for a
        // banner without a Center/history entry.
        transient: deadline.is_some() || display.transient,
        // `persistent` notifications stay after an action until withdrawn.
        resident: display.persistent,
        presentation: canopa_notify::PresentationHints {
            // The banner hides after its timeout; callbacks stay live in the
            // Center until the user or application ends the notification.
            keep_after_banner: deadline.is_none() && !display.transient,
            center_only: display.tray,
            lock_screen: if display.hide_on_lock_screen {
                Some(canopa_notify::LockScreenVisibility::Hidden)
            } else if display.hide_content_on_lock_screen {
                Some(canopa_notify::LockScreenVisibility::NameOnly)
            } else {
                None
            },
            quiet_update: !display.show_as_new,
            reply,
        },
        // Portal has no sender timeout, so ordinary portal notifications follow
        // the server default banner policy like freedesktop requests with -1:
        // low/normal expire after display and critical (`urgent`) stays. Timed
        // requests keep their own withdrawal deadline instead.
        expire_timeout: if deadline.is_some() {
            NotificationTimeout::Never
        } else {
            NotificationTimeout::ServerDefault
        },
        ..Default::default()
    };
    Ok((
        input,
        Binding {
            identity,
            actions,
            bytes,
            deadline,
        },
    ))
}

fn validate_action(name: &str, allow_empty: bool) -> zbus::fdo::Result<()> {
    if (name.is_empty() && !allow_empty) || name == "app." {
        return Err(invalid());
    }
    if let Some(name) = name.strip_prefix("app.")
        && !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'.')
    {
        return Err(invalid());
    }
    Ok(())
}

fn descriptor(value: &OwnedValue, maximum: usize) -> zbus::fdo::Result<Vec<u8>> {
    use rustix::fs::{SealFlags, fcntl_get_seals};
    let fd = <&zbus::zvariant::Fd<'_>>::try_from(value).map_err(|_| invalid())?;
    let seals = fcntl_get_seals(fd).map_err(|_| invalid())?;
    if !seals.contains(SealFlags::WRITE | SealFlags::SHRINK | SealFlags::GROW) {
        return Err(invalid());
    }
    let file = std::fs::File::from(fd.as_fd().try_clone_to_owned().map_err(|_| invalid())?);
    let metadata = file.metadata().map_err(|_| invalid())?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > maximum as u64 {
        return Err(invalid());
    }
    let mut bytes = vec![0; metadata.len() as usize];
    file.read_exact_at(&mut bytes, 0).map_err(|_| invalid())?;
    Ok(bytes)
}

fn serialized(value: OwnedValue) -> zbus::fdo::Result<(String, OwnedValue)> {
    // zvariant's generic tuple conversion does not check the field count. Check
    // the complete wire shape first, then unwrap exactly its declared variant.
    if value.value_signature().to_string() != "(sv)" {
        return Err(invalid());
    }
    let (kind, value) = <(String, OwnedValue)>::try_from(value).map_err(|_| invalid())?;
    let Value::Value(value) = Value::from(value) else {
        return Err(invalid());
    };
    Ok((kind, value.try_into_owned().map_err(|_| invalid())?))
}

fn icon(
    value: OwnedValue,
) -> zbus::fdo::Result<(
    Option<NotificationVisualReference>,
    Option<canopa_notify::NotificationImageData>,
)> {
    if let Ok(name) = <&str>::try_from(&value) {
        return themed(vec![name.to_string()]);
    }
    let (kind, value) = serialized(value)?;
    match kind.as_str() {
        "themed" => themed(Vec::<String>::try_from(value).map_err(|_| invalid())?),
        "file-descriptor" => {
            let bytes = descriptor(&value, ICON_BYTES)?;
            decoded_icon(bytes).map(|image| (None, Some(image)))
        }
        _ => Err(invalid()),
    }
}

fn themed(
    names: Vec<String>,
) -> zbus::fdo::Result<(
    Option<NotificationVisualReference>,
    Option<canopa_notify::NotificationImageData>,
)> {
    if names.is_empty()
        || names.len() > 32
        || names.iter().any(|name| {
            name.is_empty() || name.len() > 255 || name.contains('/') || name.contains(':')
        })
    {
        return Err(invalid());
    }
    Ok((Some(NotificationVisualReference::IconNames(names)), None))
}

#[cfg(not(feature = "gui"))]
fn decoded_icon(_bytes: Vec<u8>) -> zbus::fdo::Result<canopa_notify::NotificationImageData> {
    Err(zbus::fdo::Error::NotSupported(
        "descriptor icon decoding requires the graphical adapter".into(),
    ))
}

#[cfg(feature = "gui")]
fn decoded_icon(bytes: Vec<u8>) -> zbus::fdo::Result<canopa_notify::NotificationImageData> {
    use canopa_assets::{
        DecoderAllocation, FileAccess, Formats, RasterBatch, RasterBudget, RasterPolicy,
    };
    let raster = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") || bytes.starts_with(b"\xff\xd8\xff") {
        RasterBatch::new(
            RasterPolicy {
                max_encoded_bytes: ICON_BYTES,
                max_dimension: 4096,
                max_pixels: 4 * 1024 * 1024,
                decoder_allocation: DecoderAllocation::IncludingOutput(32 * 1024 * 1024),
                formats: Formats::StaticPngJpeg,
                file_access: FileAccess::Follow,
                apply_orientation: true,
                thumbnail: Some(128),
            },
            RasterBudget {
                file_attempts: 1,
                encoded_bytes: ICON_BYTES,
                pixel_bytes: 16 * 1024 * 1024,
            },
            &|| false,
        )
        .decode(bytes)
    } else {
        canopa_assets::svg::rasterize(
            &bytes,
            canopa_assets::svg::Policy {
                max_encoded_bytes: ICON_BYTES,
                max_dimension: 4096,
                max_pixels: 4 * 1024 * 1024,
                output_side: 128,
                foreground: [128, 128, 128],
            },
        )
    }
    .ok_or_else(invalid)?;
    Ok(canopa_notify::NotificationImageData {
        width: raster.width,
        height: raster.height,
        rowstride: raster.width * 4,
        has_alpha: true,
        channels: 4,
        data: raster.pixels,
    })
}
