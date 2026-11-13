//! Built-in file glyphs: the fallback when the icon theme has no icon for an
//! entry's shared-mime-info type. Type icons come from `icons_ui`; thumbnails
//! are not prepared yet (see `icons_ui::FileVisual`).
use crate::browse::{Entry, EntryKind};
use canopa_file::mime::{self, MimeDatabase};
/// Source and markup types drawn with the code glyph. Scripts arrive through
/// their `text-x-script` generic icon instead.
const SOURCE_TYPES: &[&str] = &[
    "application/javascript",
    "application/json",
    "application/toml",
    "application/x-yaml",
    "application/xml",
    "text/css",
    "text/html",
    "text/javascript",
    "text/rust",
    "text/x-c++hdr",
    "text/x-c++src",
    "text/x-chdr",
    "text/x-cmake",
    "text/x-csharp",
    "text/x-csrc",
    "text/x-go",
    "text/x-java",
    "text/x-kotlin",
    "text/x-lua",
    "text/x-makefile",
    "text/x-python",
    "text/x-python3",
    "text/x-ruby",
    "text/x-sql",
    "text/x-swift",
];

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
    Code,
    Presentation,
    Document,
    Audio,
    Video,
}
impl FileIcon {
    pub const ALL: [Self; 13] = [
        Self::Folder,
        Self::File,
        Self::Link,
        Self::Image,
        Self::Pdf,
        Self::Markdown,
        Self::Table,
        Self::Archive,
        Self::Code,
        Self::Presentation,
        Self::Document,
        Self::Audio,
        Self::Video,
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
            _ if mime.starts_with(super::icons_ui::SPECIAL_FOLDER) => Self::Folder,
            "inode/symlink" => Self::Link,
            "application/pdf" => Self::Pdf,
            "text/markdown" => Self::Markdown,
            "text/csv" | "text/tab-separated-values" => Self::Table,
            _ if mime.starts_with("image/") => Self::Image,
            _ if mime.starts_with("audio/") || generic == "audio-x-generic" => Self::Audio,
            _ if mime.starts_with("video/") || generic == "video-x-generic" => Self::Video,
            _ if generic == "x-office-spreadsheet" => Self::Table,
            _ if generic == "x-office-presentation" => Self::Presentation,
            _ if generic == "x-office-document" => Self::Document,
            _ if generic == "package-x-generic" => Self::Archive,
            _ if generic == "text-x-script" || SOURCE_TYPES.contains(&mime) => Self::Code,
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
            Self::Code => "text-x-script-symbolic",
            Self::Presentation => "x-office-presentation-symbolic",
            Self::Document => "x-office-document-symbolic",
            Self::Audio => "audio-x-generic-symbolic",
            Self::Video => "video-x-generic-symbolic",
        }
    }
    pub fn fallback(self) -> &'static [u8] {
        match self {
        Self::Folder=>br#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="2 2.5 44 43"><path fill="currentColor" fill-opacity=".52" d="M4 12.5A3.5 3.5 0 0 1 7.5 9h10.4a3.5 3.5 0 0 1 2.6 1.1l2.8 2.9h17.2a3.5 3.5 0 0 1 3.5 3.5V21H4Z"/><path fill="currentColor" fill-opacity=".82" d="M4 20a3 3 0 0 1 3-3h34a3 3 0 0 1 3 3v16.5a3.5 3.5 0 0 1-3.5 3.5h-33A3.5 3.5 0 0 1 4 36.5Z"/></svg>"#,
        Self::File=>br#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="4 2.5 40 43"><path fill="currentColor" fill-opacity=".14" d="M10 4h19l9 10v30H10z"/><path fill="none" stroke="currentColor" stroke-width="3" stroke-linejoin="round" d="M10 4h19l9 10v30H10zM29 4v11h9M16 24h16M16 31h16M16 38h10"/></svg>"#,
        Self::Image=>br#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 48 48"><path fill="none" stroke="currentColor" stroke-width="3.2" stroke-linejoin="round" d="M5 8h38v32H5zm0 25 12-12 9 9 7-7 10 10"/><circle cx="31" cy="17" r="3" fill="currentColor"/></svg>"#,
        Self::Pdf=>br#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="4 2.5 40 43"><path fill="currentColor" fill-opacity=".14" d="M10 4h19l9 10v30H10z"/><path fill="none" stroke="currentColor" stroke-width="3" stroke-linejoin="round" d="M10 4h19l9 10v30H10zM29 4v11h9M15 34c12-2 13-19 9-17s4 18 11 15-10-5-20 2Z"/></svg>"#,
        Self::Markdown=>br#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="4 2.5 40 43"><path fill="currentColor" fill-opacity=".14" d="M10 4h19l9 10v30H10z"/><path fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" d="M10 4h19l9 10v30H10zM29 4v11h9M15 34V23l5 6 5-6v11m5-11v11m-3-3 3 3 3-3"/></svg>"#,
        Self::Table=>br#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="4 2.5 40 43"><path fill="currentColor" fill-opacity=".14" d="M10 4h19l9 10v30H10z"/><path fill="none" stroke="currentColor" stroke-width="3" stroke-linejoin="round" d="M10 4h19l9 10v30H10zM29 4v11h9M15 22h18v16H15zM15 28h18m-18 5h18m-12-11v16m6-16v16"/></svg>"#,
        Self::Archive=>br#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 48 48"><path fill="none" stroke="currentColor" stroke-width="3" stroke-linejoin="round" d="M6 9h36v10H6zm3 10h30v22H9zm10 8h10"/></svg>"#,
        Self::Code=>br#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="4 2.5 40 43"><path fill="currentColor" fill-opacity=".14" d="M10 4h19l9 10v30H10z"/><path fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" d="M10 4h19l9 10v30H10zM29 4v11h9M20 24l-5 5.5 5 5.5m8-11 5 5.5-5 5.5"/></svg>"#,
        Self::Presentation=>br#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="4 2.5 40 43"><path fill="currentColor" fill-opacity=".14" d="M10 4h19l9 10v30H10z"/><path fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" d="M10 4h19l9 10v30H10zM29 4v11h9M15 22h18v13H15zM19.5 31v-3m4.5 3v-5.5m4.5 5.5v-2"/></svg>"#,
        Self::Document=>br#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="4 2.5 40 43"><path fill="currentColor" fill-opacity=".14" d="M10 4h19l9 10v30H10z"/><path fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" d="M10 4h19l9 10v30H10zM29 4v11h9M16 22h9M16 28h16M16 33h16M16 38h12"/></svg>"#,
        Self::Audio=>br#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="4 2.5 40 43"><path fill="currentColor" fill-opacity=".14" d="M10 4h19l9 10v30H10z"/><path fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" d="M10 4h19l9 10v30H10zM29 4v11h9M21 35.5V23l10-2.5v12.5M21 35.5a3 3 0 1 1-6 0 3 3 0 0 1 6 0Zm10-2.5a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z"/></svg>"#,
        Self::Video=>br#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="4 2.5 40 43"><path fill="currentColor" fill-opacity=".14" d="M10 4h19l9 10v30H10z"/><path fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" d="M10 4h19l9 10v30H10zM29 4v11h9M20 23v13l10-6.5Z"/></svg>"#,
        Self::Link=>br#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 48 48"><g fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round"><path d="M20 13l5-5a10 10 0 0 1 15 15l-9 9a10 10 0 0 1-15-2M28 35l-5 5A10 10 0 0 1 8 25l9-9a10 10 0 0 1 15 2M16 32l16-16"/></g></svg>"#,
    }
    }
}
