//! Private producer identities for input workers, never public portal results.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mapping {
    pub(crate) session: String,
    pub(crate) stream: String,
    pub(crate) source_id: u64,
    pub(crate) mapping_id: String,
    pub(crate) size: (i32, i32),
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) physical_scale: Option<f64>,
}
impl Mapping {
    pub(crate) fn valid(&self) -> bool {
        path_id(&self.session, "Session").is_some()
            && path_id(&self.stream, "Stream").is_some()
            && self.source_id != 0
            && valid_mapping_id(&self.mapping_id)
            && valid_size(self.size)
            && self.physical_scale.is_none_or(valid_scale)
    }
    pub(super) fn matches(&self, source: &Source) -> bool {
        source_identity(source) == Some(self.source_id)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Region {
    pub(crate) node: u32,
    pub(crate) stream: String,
    pub(crate) source_id: u64,
    pub(crate) mapping_id: String,
    pub(crate) size: (i32, i32),
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) physical_scale: Option<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bindings {
    pub(crate) compositor: String,
    pub(crate) session: String,
    pub(crate) regions: Vec<Region>,
}
impl Bindings {
    pub(crate) fn from_streams(owner: &str, streams: &[Stream]) -> anyhow::Result<Option<Self>> {
        if streams.iter().any(|stream| stream.mapping.is_none()) {
            return Ok(None);
        }
        let session = streams
            .first()
            .and_then(|s| s.mapping.as_ref())
            .ok_or_else(|| anyhow::anyhow!("Missing capture mappings"))?
            .session
            .clone();
        let mut regions = Vec::with_capacity(streams.len());
        for stream in streams {
            let mapping = stream.mapping.as_ref().expect("all mappings present");
            anyhow::ensure!(
                mapping.valid() && mapping.matches(&stream.source) && mapping.session == session,
                "Invalid capture mapping"
            );
            regions.push(Region {
                node: stream.node,
                stream: mapping.stream.clone(),
                source_id: mapping.source_id,
                mapping_id: mapping.mapping_id.clone(),
                size: mapping.size,
                physical_scale: mapping.physical_scale,
            });
        }
        let binding = Self {
            compositor: owner.to_owned(),
            session,
            regions,
        };
        binding.validate()?;
        Ok(Some(binding))
    }
    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        let _: zbus::names::UniqueName<'_> = self.compositor.as_str().try_into()?;
        anyhow::ensure!(
            path_id(&self.session, "Session").is_some()
                && !self.regions.is_empty()
                && self.regions.len() <= MAX_SOURCES,
            "Invalid capture binding"
        );
        let mut nodes = HashSet::new();
        let mut streams = HashSet::new();
        let mut mappings = HashSet::new();
        for region in &self.regions {
            anyhow::ensure!(
                region.node != 0
                    && region.node != u32::MAX
                    && nodes.insert(region.node)
                    && path_id(&region.stream, "Stream").is_some()
                    && streams.insert(&region.stream)
                    && region.source_id != 0
                    && valid_mapping_id(&region.mapping_id)
                    && mappings.insert(&region.mapping_id)
                    && valid_size(region.size)
                    && region.physical_scale.is_none_or(valid_scale),
                "Invalid capture region"
            );
        }
        Ok(())
    }
}

pub(crate) fn source_identity(source: &Source) -> Option<u64> {
    match source {
        Source::Monitor(monitor) => monitor.identity(),
        Source::Window(id) => Some(*id),
    }
}
pub(crate) fn valid_size((w, h): (i32, i32)) -> bool {
    (1..=32768).contains(&w) && (1..=32768).contains(&h)
}
pub(crate) fn valid_scale(scale: f64) -> bool {
    scale.is_finite() && scale > 0. && scale <= 64.
}
pub(crate) fn valid_mapping_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 256 && !id.chars().any(char::is_control)
}
pub(crate) fn path_id(path: &str, kind: &str) -> Option<u64> {
    let suffix = path
        .strip_prefix("/org/gnome/Mutter/ScreenCast/")?
        .strip_prefix(kind)?
        .strip_prefix("/u")?;
    if suffix.is_empty()
        || suffix.len() > 20
        || suffix.starts_with('0')
        || !suffix.bytes().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    suffix.parse().ok()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Shape {
    pub node: u32,
    pub size: (i32, i32),
    pub physical_scale: Option<f64>,
}

impl Bindings {
    /// Revalidate the exact producer objects; size/scale may change, identity may not.
    pub(crate) async fn current_shapes(
        &self,
        connection: &Connection,
        pid: u32,
    ) -> anyhow::Result<Vec<Shape>> {
        use futures_util::{StreamExt, TryStreamExt, stream};
        self.validate()?;
        tokio::time::timeout(CALL_TIME, async {
            let bus = zbus::fdo::DBusProxy::new(connection).await?;
            anyhow::ensure!(
                bus.get_name_owner(SERVICE.try_into()?).await?.as_str() == self.compositor
                    && bus
                        .get_connection_unix_process_id(self.compositor.as_str().try_into()?)
                        .await?
                        == pid,
                "Capture binding belongs to another compositor"
            );
            stream::iter(self.regions.iter().map(|region| async move {
                let value: zbus::zvariant::OwnedValue = worker::call(
                    connection,
                    &self.compositor,
                    &region.stream,
                    "org.freedesktop.DBus.Properties",
                    "Get",
                    &(STREAM, "Parameters"),
                    CALL_TIME,
                )
                .await?;
                let mut values =
                    std::collections::HashMap::<String, zbus::zvariant::OwnedValue>::try_from(
                        value,
                    )?;
                let id = u64::try_from(
                    values
                        .remove("canopa-source-id")
                        .ok_or_else(|| anyhow::anyhow!("Missing source identity"))?,
                )?;
                let mapping = String::try_from(
                    values
                        .remove("mapping-id")
                        .ok_or_else(|| anyhow::anyhow!("Missing mapping identity"))?,
                )?;
                let size = values
                    .remove("size")
                    .ok_or_else(|| anyhow::anyhow!("Missing mapping size"))?
                    .try_into()?;
                let physical_scale = values
                    .remove("canopa-physical-scale")
                    .map(f64::try_from)
                    .transpose()?;
                anyhow::ensure!(
                    id == region.source_id && mapping == region.mapping_id,
                    "Capture mapping changed identity"
                );
                anyhow::ensure!(
                    valid_size(size)
                        && physical_scale.is_none_or(valid_scale)
                        && (region.physical_scale.is_none() || physical_scale.is_some()),
                    "Capture projection metadata unavailable"
                );
                Ok::<_, anyhow::Error>(Shape {
                    node: region.node,
                    size,
                    physical_scale,
                })
            }))
            .buffered(4)
            .try_collect()
            .await
        })
        .await?
    }
}
