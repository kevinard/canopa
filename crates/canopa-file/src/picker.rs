//! Request and result exchanged by the portal and the Files picker.
use serde::{Deserialize, Serialize};
use std::{ffi::OsString, os::unix::ffi::OsStringExt, path::PathBuf};

pub type Filter = (String, Vec<(u32, String)>);
pub type Choice = (String, String, Vec<(String, String)>, String);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Open,
    Save,
    SaveMany,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChooserRequest {
    pub mode: Mode,
    pub app_id: String,
    pub parent_window: String,
    pub title: String,
    pub accept_label: String,
    pub modal: bool,
    pub multiple: bool,
    pub directory: bool,
    pub filters: Vec<Filter>,
    pub current_filter: Option<Filter>,
    pub choices: Vec<Choice>,
    pub current_folder: Option<Vec<u8>>,
    pub current_file: Option<Vec<u8>>,
    pub current_name: Option<String>,
    pub files: Vec<Vec<u8>>,
}

pub fn path_bytes(bytes: &[u8]) -> Option<PathBuf> {
    let bytes = bytes.strip_suffix(&[0])?;
    (!bytes.is_empty() && !bytes.contains(&0))
        .then(|| PathBuf::from(OsString::from_vec(bytes.to_vec())))
}

pub fn file_name(path: &std::path::Path) -> bool {
    path.file_name()
        .is_some_and(|name| name == path.as_os_str())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Selection {
    /// Original absolute Unix path bytes, without the wire's trailing NUL.
    pub paths: Vec<Vec<u8>>,
    pub choices: Vec<(String, String)>,
    pub current_filter: Option<Filter>,
    pub writable: bool,
}
