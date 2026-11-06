//! Local profile discovery and immutable avatar preparation. Run on a worker.
use crate::account::{AccountRequest, FIELD_BYTES};
use canopa_assets::{
    DecoderAllocation, FileAccess, Formats, RasterBatch, RasterBudget, RasterPolicy,
};
use image::ImageEncoder;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use zbus::zvariant::OwnedValue;

mod cache;
pub(crate) use cache::publish;

const SOURCE_BYTES: usize = 8 * 1024 * 1024;
const SVG_BYTES: usize = 256 * 1024;
const IMAGE_SIDE: u32 = 256;

#[derive(Clone)]
pub(crate) struct Avatar {
    pub raster: Arc<canopa_assets::Raster>,
    pub png: Arc<Vec<u8>>,
}

impl std::fmt::Debug for Avatar {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Avatar")
            .field("width", &self.raster.width)
            .field("height", &self.raster.height)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub(crate) struct Profile {
    pub id: String,
    pub name: String,
    pub requester: String,
    pub pictures: PathBuf,
    pub avatar: Option<Avatar>,
}

impl std::fmt::Debug for Profile {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Profile").finish_non_exhaustive()
    }
}

#[derive(Default)]
struct Fields {
    id: Option<String>,
    name: Option<String>,
    image: Option<PathBuf>,
}

fn field(values: &mut HashMap<String, OwnedValue>, name: &str) -> Option<String> {
    let value = String::try_from(values.remove(name)?).ok()?;
    (value.len() <= FIELD_BYTES && !value.contains('\0')).then_some(value)
}

async fn service_fields(connection: &zbus::Connection, uid: u32) -> zbus::Result<Fields> {
    let path = format!("/org/freedesktop/Accounts/User{uid}");
    let reply = connection
        .call_method(
            Some("org.freedesktop.Accounts"),
            path.as_str(),
            Some("org.freedesktop.DBus.Properties"),
            "GetAll",
            &("org.freedesktop.Accounts.User",),
        )
        .await?;
    let mut values: HashMap<String, OwnedValue> = reply.body().deserialize()?;
    let image = field(&mut values, "IconFile")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_absolute());
    Ok(Fields {
        id: field(&mut values, "UserName").filter(|value| !value.is_empty()),
        name: field(&mut values, "RealName"),
        image,
    })
}

fn system_fields() -> Option<Fields> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok()?;
    runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(2), async {
            let connection = zbus::Connection::system().await?;
            service_fields(&connection, rustix::process::geteuid().as_raw()).await
        })
        .await
        .ok()?
        .ok()
    })
}

fn bounded(value: Result<String, std::io::Error>) -> String {
    value
        .ok()
        .filter(|value| value.len() <= FIELD_BYTES && !value.contains('\0'))
        .unwrap_or_default()
}

fn identity(
    fields: Option<&Fields>,
    fallback: impl FnOnce() -> (String, String),
) -> (String, String) {
    let id = fields.and_then(|fields| fields.id.clone());
    let name = fields.and_then(|fields| fields.name.clone());
    if let (Some(id), Some(name)) = (&id, &name) {
        return (id.clone(), name.clone());
    }
    let (fallback_id, fallback_name) = fallback();
    (id.unwrap_or(fallback_id), name.unwrap_or(fallback_name))
}

pub(crate) fn load(request: &AccountRequest, foreground: canopa_theme::Rgb) -> Profile {
    let configured_home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute());
    let home = configured_home
        .clone()
        .unwrap_or_else(|| PathBuf::from("/"));
    let service = system_fields();
    let (id, name) = identity(service.as_ref(), || {
        (
            bounded(whoami::fallible::username()),
            bounded(whoami::fallible::realname()),
        )
    });
    let candidates = if let Some(fields) = service {
        fields.image.into_iter().collect::<Vec<_>>()
    } else {
        configured_home
            .into_iter()
            .flat_map(|home| [home.join(".face"), home.join(".face.icon")])
            .collect()
    };
    let avatar = candidates
        .into_iter()
        .find_map(|path| prepare_avatar(&path, foreground).ok());
    let pictures = canopa_config::config_home_from(
        std::env::var_os("XDG_CONFIG_HOME").as_deref(),
        Some(home.as_os_str()),
    )
    .ok()
    .and_then(|config| {
        canopa_files::personal_folders::PersonalFolders::load(&home, &config)
            .ok()
            .and_then(|places| {
                places
                    .path(canopa_files::interface::PlaceKind::Pictures)
                    .map(Path::to_owned)
            })
    })
    .unwrap_or(home);
    let desktop_id = if request.app_id.ends_with(".desktop") {
        request.app_id.clone()
    } else {
        format!("{}.desktop", request.app_id)
    };
    let requester = canopa_apps::discover_choice_metadata()
        .ok()
        .and_then(|apps| apps.into_iter().find(|app| app.id == desktop_id))
        .map(|app| app.name.chars().take(120).collect())
        .unwrap_or_else(|| {
            if request.app_id.is_empty() {
                "The requesting app".into()
            } else {
                request.app_id.chars().take(120).collect()
            }
        });
    Profile {
        id,
        name,
        requester,
        pictures,
        avatar,
    }
}

pub(crate) fn prepare_avatar(path: &Path, foreground: canopa_theme::Rgb) -> anyhow::Result<Avatar> {
    anyhow::ensure!(path.is_absolute(), "image must be local");
    let bytes = canopa_file::read_bounded(path, SOURCE_BYTES)?;
    let svg = image::guess_format(&bytes).is_err()
        && (path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
            || bytes[..bytes.len().min(4096)]
                .windows(4)
                .any(|word| word == b"<svg"));
    let raster = if svg {
        let canopa_theme::Rgb(r, g, b) = foreground;
        canopa_assets::svg::rasterize(
            &bytes,
            canopa_assets::svg::Policy {
                max_encoded_bytes: SVG_BYTES,
                max_dimension: 8192,
                max_pixels: 16 * 1024 * 1024,
                output_side: IMAGE_SIDE,
                foreground: [r, g, b],
            },
        )
    } else {
        anyhow::ensure!(
            matches!(
                image::guess_format(&bytes),
                Ok(image::ImageFormat::Png
                    | image::ImageFormat::Jpeg
                    | image::ImageFormat::Gif
                    | image::ImageFormat::WebP)
            ),
            "unsupported image format"
        );
        RasterBatch::new(
            RasterPolicy {
                max_encoded_bytes: SOURCE_BYTES,
                max_dimension: 8192,
                max_pixels: 16 * 1024 * 1024,
                decoder_allocation: DecoderAllocation::IncludingOutput(96 * 1024 * 1024),
                formats: Formats::EnabledRasterCodecs,
                file_access: FileAccess::Follow,
                apply_orientation: true,
                thumbnail: Some(IMAGE_SIDE),
            },
            RasterBudget {
                file_attempts: 1,
                encoded_bytes: SOURCE_BYTES,
                pixel_bytes: 64 * 1024 * 1024,
            },
            &|| false,
        )
        .decode(bytes)
    }
    .ok_or_else(|| anyhow::anyhow!("image is unavailable, invalid or over budget"))?;
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png).write_image(
        &raster.pixels,
        raster.width,
        raster.height,
        image::ExtendedColorType::Rgba8,
    )?;
    Ok(Avatar {
        raster: Arc::new(raster),
        png: Arc::new(png),
    })
}

#[cfg(test)]
mod tests;
