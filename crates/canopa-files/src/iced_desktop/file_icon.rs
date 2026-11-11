//! Built-in file glyphs: the fallback when the icon theme has no icon for an
//! entry's shared-mime-info type. Type icons come from `icons_ui`; thumbnails
//! are not prepared yet (see `icons_ui::FileVisual`).
use crate::browse::{Entry, EntryKind};
use canopa_file::mime::{self, MimeDatabase};
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum FileIcon {
    Folder,
    File,
    Link,
    Image,
    Pdf,
    Markdown,
    Table,
    Archive,
}
impl FileIcon {
    pub const ALL: [Self; 8] = [
        Self::Folder,
        Self::File,
        Self::Link,
        Self::Image,
        Self::Pdf,
        Self::Markdown,
        Self::Table,
        Self::Archive,
    ];
    /// Before an entry's type is known: only what the listing itself says.
    pub fn for_entry(entry: &Entry) -> Self {
        if entry.is_directory() {
            Self::Folder
        } else if entry.kind == EntryKind::Symlink {
            Self::Link
        } else {
            Self::File
        }
    }
    /// The glyph category of a detected type, through its generic icon.
    pub fn for_type(mime: &str, database: Option<&MimeDatabase>) -> Self {
        let generic = database
            .map(|database| database.icon_names(mime))
            .and_then(|names| names.last().cloned())
            .unwrap_or_default();
        match mime {
            mime::DIRECTORY => Self::Folder,
            "inode/symlink" => Self::Link,
            "application/pdf" => Self::Pdf,
            "text/markdown" => Self::Markdown,
            "text/csv" | "text/tab-separated-values" => Self::Table,
            _ if mime.starts_with("image/") => Self::Image,
            _ if generic == "x-office-spreadsheet" => Self::Table,
            _ if generic == "package-x-generic" => Self::Archive,
            _ => Self::File,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Folder => "folder-symbolic",
            Self::File => "text-x-generic-symbolic",
            Self::Link => "emblem-symbolic-link-symbolic",
            Self::Image => "image-x-generic-symbolic",
            Self::Pdf => "application-pdf-symbolic",
            Self::Markdown => "text-markdown-symbolic",
            Self::Table => "x-office-spreadsheet-symbolic",
            Self::Archive => "package-x-generic-symbolic",
        }
    }
    pub fn fallback(self) -> &'static [u8] {
        match self {
        Self::Folder=>br#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 48 48"><path fill="currentColor" fill-opacity=".22" d="M5 13V8h14l5 5h19v27H5z"/><path fill="none" stroke="currentColor" stroke-width="2.5" stroke-linejoin="round" d="M5 13V8h14l5 5h19v27H5zM5 18h38"/></svg>"#,
        Self::File=>br#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 48 48"><path fill="none" stroke="currentColor" stroke-width="2.5" stroke-linejoin="round" d="M10 4h19l9 10v30H10zM29 4v11h9M16 24h16M16 31h16M16 38h10"/></svg>"#,
        Self::Image=>br#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 48 48"><path fill="none" stroke="currentColor" stroke-width="3" stroke-linejoin="round" d="M5 8h38v32H5zm0 25 12-12 9 9 7-7 10 10"/><circle cx="31" cy="17" r="3" fill="currentColor"/></svg>"#,
        Self::Pdf=>br#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 48 48"><path fill="none" stroke="currentColor" stroke-width="2.5" stroke-linejoin="round" d="M10 4h19l9 10v30H10zM29 4v11h9M15 34c12-2 13-19 9-17s4 18 11 15-10-5-20 2Z"/></svg>"#,
        Self::Markdown=>br#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 48 48"><path fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" d="M10 4h19l9 10v30H10zM29 4v11h9M15 34V23l5 6 5-6v11m5-11v11m-3-3 3 3 3-3"/></svg>"#,
        Self::Table=>br#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 48 48"><path fill="none" stroke="currentColor" stroke-width="2.5" stroke-linejoin="round" d="M10 4h19l9 10v30H10zM29 4v11h9M15 22h18v16H15zM15 28h18m-18 5h18m-12-11v16m6-16v16"/></svg>"#,
        Self::Archive=>br#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 48 48"><path fill="none" stroke="currentColor" stroke-width="3" stroke-linejoin="round" d="M6 9h36v10H6zm3 10h30v22H9zm10 8h10"/></svg>"#,
        Self::Link=>br#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 48 48"><g fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"><path d="M20 13l5-5a10 10 0 0 1 15 15l-9 9a10 10 0 0 1-15-2M28 35l-5 5A10 10 0 0 1 8 25l9-9a10 10 0 0 1 15 2M16 32l16-16"/></g></svg>"#,
    }
    }
}
