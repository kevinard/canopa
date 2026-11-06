//! Noncredential preferences, committed only after greetd acknowledges session start.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    pub last_user: Option<String>,
    pub sessions: BTreeMap<String, String>,
}
impl Preferences {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.sessions.len() <= 128
                && self
                    .last_user
                    .as_deref()
                    .is_none_or(crate::config::valid_user)
                && self.sessions.iter().all(
                    |(user, session)| crate::config::valid_user(user) && valid_session(session)
                ),
            "invalid greeter preferences"
        );
        Ok(())
    }
    pub fn path() -> Option<PathBuf> {
        std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .filter(|p| p.is_absolute())
                    .map(|p| p.join(".local/state"))
            })
            .map(|p| p.join("canopa/greeter.json"))
    }
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = canopa_file::private_state::read(path, 32768)?;
        let value: Self = serde_json::from_slice(&bytes)?;
        value.validate()?;
        Ok(value)
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        canopa_file::private_state::write(path, &serde_json::to_vec(self)?)?;
        Ok(())
    }
    pub fn record_success(&mut self, user: &str, session: &str) {
        if !crate::config::valid_user(user) || !valid_session(session) {
            return;
        }
        if !self.sessions.contains_key(user)
            && self.sessions.len() == 128
            && let Some(key) = self.sessions.keys().next().cloned()
        {
            self.sessions.remove(&key);
        }
        self.sessions.insert(user.into(), session.into());
        self.last_user = Some(user.into());
    }
}
fn valid_session(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && !id.chars().any(char::is_control)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_names_and_session_ids_are_saved_and_invalid_state_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("state/greeter.json");
        let mut p = Preferences::default();
        p.record_success("alice", "canopa");
        p.record_success("bob", "other");
        p.save(&path).unwrap();
        assert_eq!(Preferences::load(&path).unwrap(), p);
        assert_eq!(p.sessions["alice"], "canopa");
        let bytes = std::fs::read_to_string(&path).unwrap();
        assert!(!bytes.contains("argv"));
        canopa_file::private_state::write(&path, br#"{"last_user":"bad user","sessions":{}}"#)
            .unwrap();
        assert!(Preferences::load(&path).is_err());
        p.record_success("bad user", "canopa");
        assert_eq!(p.last_user.as_deref(), Some("bob"));
    }
}
