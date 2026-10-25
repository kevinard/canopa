//! Bounded preview preparation. Callers run this on a worker and guard stale results.
use crate::Cancellation;
use std::{
    fs,
    io::{self, Cursor, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};

const MAXIMUM_FILE_BYTES: usize = 128 * 1024 * 1024;
const MAXIMUM_TEXT_BYTES: usize = 1024 * 1024;
mod media;
pub use media::MediaSource;

#[derive(Clone, Debug)]
pub struct Bitmap {
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HighlightKind {
    Keyword,
    String,
    Number,
    Comment,
}

/// Character offsets, matching native text-buffer coordinates rather than UTF-8 bytes.
#[derive(Clone, Debug)]
pub struct TextHighlight {
    pub start: u32,
    pub end: u32,
    pub kind: HighlightKind,
}

#[derive(Clone, Debug)]
pub enum PreviewContent {
    Image(Bitmap),
    Pdf {
        bitmap: Bitmap,
        page: u32,
        pages: u32,
    },
    Text {
        text: String,
        language: Option<String>,
        truncated: bool,
        highlights: Vec<TextHighlight>,
    },
    Audio(MediaSource),
    Video(MediaSource),
    Folder {
        items: usize,
    },
    Binary,
}

#[derive(Clone, Debug)]
pub struct PreparedPreview {
    pub path: PathBuf,
    pub size: u64,
    pub content: PreviewContent,
}

#[derive(Clone, Debug)]
pub struct PreviewRequest {
    pub path: PathBuf,
    pub page: u32,
    /// The original name of an opaque stored file, used only for classification.
    pub name_hint: Option<std::ffi::OsString>,
}

impl PreviewRequest {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_owned(),
            page: 1,
            name_hint: None,
        }
    }

    pub fn prepare(&self, cancellation: &Cancellation) -> io::Result<PreparedPreview> {
        check_cancel(cancellation)?;
        let metadata = fs::metadata(&self.path)?;
        if metadata.is_dir() {
            let mut items = 0;
            for entry in fs::read_dir(&self.path)? {
                check_cancel(cancellation)?;
                entry?;
                items += 1;
            }
            return Ok(PreparedPreview {
                path: self.path.clone(),
                size: 0,
                content: PreviewContent::Folder { items },
            });
        }
        let file = open_regular(&self.path)?;
        let metadata = file.metadata()?;
        let prefix = read_opened(&file, MAXIMUM_TEXT_BYTES, true, cancellation)?;
        let extension = self
            .name_hint
            .as_deref()
            .map(Path::new)
            .unwrap_or(&self.path)
            .extension()
            .map(|ext| ext.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let content = if prefix.starts_with(b"%PDF-") || extension == "pdf" {
            self.pdf(&file, cancellation)?
        } else if image::guess_format(&prefix).is_ok() {
            (&file).seek(SeekFrom::Start(0))?;
            let bytes = read_opened(&file, MAXIMUM_FILE_BYTES, false, cancellation)?;
            PreviewContent::Image(decode_image(bytes)?)
        } else if matches!(
            extension.as_str(),
            "mp4" | "mkv" | "webm" | "mov" | "avi" | "mpg" | "mpeg" | "ogv" | "m4v"
        ) {
            PreviewContent::Video(MediaSource::new(file, metadata.len(), cancellation)?)
        } else if matches!(
            extension.as_str(),
            "mp3" | "ogg" | "opus" | "flac" | "wav" | "m4a" | "aac" | "oga"
        ) {
            PreviewContent::Audio(MediaSource::new(file, metadata.len(), cancellation)?)
        } else if !prefix.contains(&0)
            || prefix.starts_with(&[0xff, 0xfe])
            || prefix.starts_with(&[0xfe, 0xff])
        {
            let truncated = prefix.len() > MAXIMUM_TEXT_BYTES;
            let bytes = &prefix[..prefix.len().min(MAXIMUM_TEXT_BYTES)];
            let text = if bytes.starts_with(&[0xff, 0xfe]) {
                String::from_utf16_lossy(
                    &bytes[2..]
                        .chunks_exact(2)
                        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                        .collect::<Vec<_>>(),
                )
            } else if bytes.starts_with(&[0xfe, 0xff]) {
                String::from_utf16_lossy(
                    &bytes[2..]
                        .chunks_exact(2)
                        .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
                        .collect::<Vec<_>>(),
                )
            } else {
                let bytes = if truncated {
                    match std::str::from_utf8(bytes) {
                        Err(error) if error.error_len().is_none() => &bytes[..error.valid_up_to()],
                        _ => bytes,
                    }
                } else {
                    bytes
                };
                String::from_utf8_lossy(bytes)
                    .trim_start_matches('\u{feff}')
                    .to_owned()
            };
            let language = matches!(
                extension.as_str(),
                "rs" | "py"
                    | "sh"
                    | "kdl"
                    | "json"
                    | "toml"
                    | "yaml"
                    | "yml"
                    | "js"
                    | "ts"
                    | "tsx"
                    | "jsx"
                    | "css"
                    | "html"
                    | "xml"
                    | "svg"
                    | "c"
                    | "cpp"
                    | "h"
                    | "hpp"
                    | "go"
                    | "lua"
                    | "rb"
                    | "java"
                    | "sql"
                    | "md"
                    | "ini"
                    | "conf"
            )
            .then_some(extension);
            let highlights = crate::syntax::highlight(&text, language.as_deref(), cancellation)?;
            PreviewContent::Text {
                text,
                language,
                truncated,
                highlights,
            }
        } else {
            PreviewContent::Binary
        };
        check_cancel(cancellation)?;
        Ok(PreparedPreview {
            path: self.path.clone(),
            size: metadata.len(),
            content,
        })
    }

    fn pdf(&self, file: &fs::File, cancellation: &Cancellation) -> io::Result<PreviewContent> {
        // External readers see an owned snapshot, never a path replaced by a FIFO
        // after inspection. The snapshot and rendered page are private temporary files.
        let mut file = file;
        file.seek(SeekFrom::Start(0))?;
        let bytes = read_opened(file, MAXIMUM_FILE_BYTES, false, cancellation)?;
        let temporary = tempfile::tempdir()?;
        let document = temporary.path().join("document.pdf");
        fs::write(&document, bytes)?;
        let mut command = Command::new("pdfinfo");
        command.env("LC_ALL", "C").arg(&document);
        let info = crate::process::run(command, cancellation)?;
        if info.cancelled {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "preview cancelled",
            ));
        }
        if !info.success {
            return Err(io::Error::other(
                "PDF information is unavailable; the document may be encrypted or invalid",
            ));
        }
        let pages = String::from_utf8_lossy(&info.stdout)
            .lines()
            .find_map(|line| {
                line.strip_prefix("Pages:")
                    .and_then(|value| value.trim().parse::<u32>().ok())
            })
            .filter(|pages| *pages > 0)
            .ok_or_else(|| io::Error::other("PDF page count is unavailable"))?;
        let page = self.page.clamp(1, pages);
        let output = temporary.path().join("page");
        let mut command = Command::new("pdftoppm");
        command
            .args([
                "-f",
                &page.to_string(),
                "-l",
                &page.to_string(),
                "-singlefile",
                "-scale-to",
                "1800",
                "-png",
            ])
            .arg(&document)
            .arg(&output);
        let outcome = crate::process::run(command, cancellation)?;
        if outcome.cancelled {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "preview cancelled",
            ));
        }
        if !outcome.success {
            return Err(io::Error::other("PDF page could not be rendered"));
        }
        let bytes = read_regular(
            &output.with_extension("png"),
            32 * 1024 * 1024,
            false,
            cancellation,
        )?;
        Ok(PreviewContent::Pdf {
            bitmap: decode_image(bytes)?,
            page,
            pages,
        })
    }
}

fn check_cancel(cancellation: &Cancellation) -> io::Result<()> {
    if cancellation.is_cancelled() {
        Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "preview cancelled",
        ))
    } else {
        Ok(())
    }
}

fn read_regular(
    path: &Path,
    maximum: usize,
    prefix: bool,
    cancellation: &Cancellation,
) -> io::Result<Vec<u8>> {
    read_opened(&open_regular(path)?, maximum, prefix, cancellation)
}

fn open_regular(path: &Path) -> io::Result<fs::File> {
    use rustix::fs::{Mode, OFlags, open};
    let descriptor = open(
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )?;
    let file = fs::File::from(descriptor);
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "only regular files can be previewed",
        ));
    }
    Ok(file)
}

fn read_opened(
    mut file: &fs::File,
    maximum: usize,
    prefix: bool,
    cancellation: &Cancellation,
) -> io::Result<Vec<u8>> {
    let metadata = file.metadata()?;
    if !prefix && metadata.len() > maximum as u64 {
        return Err(io::Error::other("file exceeds the preview budget"));
    }
    let mut bytes = Vec::new();
    let mut buffer = [0; 32 * 1024];
    loop {
        check_cancel(cancellation)?;
        let amount = buffer.len().min(maximum + 1 - bytes.len());
        if amount == 0 {
            break;
        }
        let count = match file.read(&mut buffer[..amount]) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    if !prefix && bytes.len() > maximum {
        return Err(io::Error::other("file exceeds the preview budget"));
    }
    Ok(bytes)
}

fn decode_image(bytes: Vec<u8>) -> io::Result<Bitmap> {
    let mut reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16_384);
    limits.max_image_height = Some(16_384);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let rgba = reader.decode().map_err(io::Error::other)?.into_rgba8();
    let (width, height) = rgba.dimensions();
    Ok(Bitmap {
        width,
        height,
        rgba: rgba.into_raw().into(),
    })
}
