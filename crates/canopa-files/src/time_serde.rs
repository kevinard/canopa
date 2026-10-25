//! File metadata may precede the Unix epoch; serde's standard SystemTime cannot.
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::time::{Duration, SystemTime};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredTime {
    before_epoch: bool,
    seconds: u64,
    nanos: u32,
}

pub fn serialize<S: Serializer>(
    time: &Option<SystemTime>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    time.map(|time| {
        let (before_epoch, duration) = match time.duration_since(SystemTime::UNIX_EPOCH) {
            Ok(duration) => (false, duration),
            Err(error) => (true, error.duration()),
        };
        StoredTime {
            before_epoch,
            seconds: duration.as_secs(),
            nanos: duration.subsec_nanos(),
        }
    })
    .serialize(serializer)
}

pub fn deserialize<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<SystemTime>, D::Error> {
    Option::<StoredTime>::deserialize(deserializer)?
        .map(|time| {
            if time.nanos >= 1_000_000_000 {
                return Err(serde::de::Error::custom("invalid file timestamp fraction"));
            }
            let duration = Duration::new(time.seconds, time.nanos);
            let value = if time.before_epoch {
                SystemTime::UNIX_EPOCH.checked_sub(duration)
            } else {
                SystemTime::UNIX_EPOCH.checked_add(duration)
            };
            value.ok_or_else(|| serde::de::Error::custom("file timestamp is out of range"))
        })
        .transpose()
}
