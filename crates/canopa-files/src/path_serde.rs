//! Preserve Unix paths across JSON state without using display labels as identifiers.
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    ffi::OsString,
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::PathBuf,
};

#[derive(Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Representation {
    Text(String),
    Bytes { unix_bytes: Vec<u8> },
}

pub fn serialize<S: Serializer>(path: &std::path::Path, serializer: S) -> Result<S::Ok, S::Error> {
    match path.to_str() {
        Some(text) => Representation::Text(text.to_owned()),
        None => Representation::Bytes {
            unix_bytes: path.as_os_str().as_bytes().to_vec(),
        },
    }
    .serialize(serializer)
}

pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<PathBuf, D::Error> {
    let bytes = match Representation::deserialize(deserializer)? {
        Representation::Text(text) => text.into_bytes(),
        Representation::Bytes { unix_bytes } => unix_bytes,
    };
    if bytes.contains(&0) {
        return Err(serde::de::Error::custom("paths cannot contain NUL bytes"));
    }
    Ok(PathBuf::from(OsString::from_vec(bytes)))
}

#[derive(Deserialize)]
#[serde(transparent)]
struct StoredPath(#[serde(with = "crate::path_serde")] PathBuf);

struct PathRef<'a>(&'a std::path::Path);
impl Serialize for PathRef<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize(self.0, serializer)
    }
}

pub mod optional {
    use super::*;

    pub fn serialize<S: Serializer>(
        path: &Option<PathBuf>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        path.as_deref().map(PathRef).serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<PathBuf>, D::Error> {
        Ok(Option::<StoredPath>::deserialize(deserializer)?.map(|path| path.0))
    }
}

pub mod sequence {
    use super::*;
    use serde::ser::SerializeSeq;

    pub fn serialize<S: Serializer>(paths: &[PathBuf], serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(paths.len()))?;
        for path in paths {
            sequence.serialize_element(&PathRef(path))?;
        }
        sequence.end()
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<PathBuf>, D::Error> {
        Ok(Vec::<StoredPath>::deserialize(deserializer)?
            .into_iter()
            .map(|path| path.0)
            .collect())
    }
}
