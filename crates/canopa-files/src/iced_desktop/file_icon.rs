//! Themed file symbols with small built-in vector fallbacks; never thumbnails.
use crate::browse::{Entry, EntryKind};
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum FileIcon {
    Folder,
    File,
    Link,
}
impl FileIcon {
    pub const ALL: [Self; 3] = [Self::Folder, Self::File, Self::Link];
    pub fn for_entry(entry: &Entry) -> Self {
        if entry.kind == EntryKind::Symlink {
            Self::Link
        } else if entry.is_directory() {
            Self::Folder
        } else {
            Self::File
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Folder => "folder-symbolic",
            Self::File => "text-x-generic-symbolic",
            Self::Link => "emblem-symbolic-link-symbolic",
        }
    }
    pub fn fallback(self) -> &'static [u8] {
        match self {
        Self::Folder=>br#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 48 48"><path fill="none" stroke="currentColor" stroke-width="2.5" stroke-linejoin="round" d="M5 13V8h14l5 5h19v27H5zM5 18h38"/></svg>"#,
        Self::File=>br#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 48 48"><path fill="none" stroke="currentColor" stroke-width="2.5" stroke-linejoin="round" d="M10 4h19l9 10v30H10zM29 4v11h9M16 24h16M16 31h16M16 38h10"/></svg>"#,
        Self::Link=>br#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 48 48"><g fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round"><path d="M20 13l5-5a10 10 0 0 1 15 15l-9 9a10 10 0 0 1-15-2M28 35l-5 5A10 10 0 0 1 8 25l9-9a10 10 0 0 1 15 2M16 32l16-16"/></g></svg>"#,
    }
    }
}
