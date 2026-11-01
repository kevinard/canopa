//! Passive weather snapshot adapter. An external provider owns location/network policy.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Weather {
    pub temperature_celsius: f64,
    pub description: String,
    pub updated_unix: u64,
}
impl Weather {
    /// Ignore malformed, future-dated or >2-hour-old data; never show stale weather as live.
    pub fn read(path: &std::path::Path, now: u64) -> Option<Self> {
        let bytes = canopa_file::read_bounded(path, 16384).ok()?;
        let mut value: Self = serde_json::from_slice(&bytes).ok()?;
        if !value.temperature_celsius.is_finite()
            || !(-100.0..=100.0).contains(&value.temperature_celsius)
            || value.updated_unix > now + 300
            || now.saturating_sub(value.updated_unix) > 7200
        {
            return None;
        }
        value.description = value
            .description
            .chars()
            .filter(|c| !c.is_control())
            .take(80)
            .collect();
        Some(value)
    }
    pub fn label(&self) -> String {
        format!("{:.0}°C · {}", self.temperature_celsius, self.description)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn weather_requires_bounded_fresh_valid_data() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("weather.json");
        let weather = Weather {
            temperature_celsius: 18.5,
            description: "Clear\n".into(),
            updated_unix: 10000,
        };
        std::fs::write(&file, serde_json::to_vec(&weather).unwrap()).unwrap();
        assert_eq!(Weather::read(&file, 10000).unwrap().description, "Clear");
        assert!(Weather::read(&file, 18000).is_none());
        assert!(Weather::read(&file, 9000).is_none());
        std::fs::write(
            &file,
            b"{\"temperature_celsius\":999,\"description\":\"Hot\",\"updated_unix\":10000}",
        )
        .unwrap();
        assert!(Weather::read(&file, 10000).is_none());
        std::fs::write(&file, vec![b' '; 16385]).unwrap();
        assert!(Weather::read(&file, 10000).is_none());
    }
}
