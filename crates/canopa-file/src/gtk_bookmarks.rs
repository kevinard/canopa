//! The GTK bookmarks file (`$XDG_CONFIG_HOME/gtk-3.0/bookmarks`), shared with GTK
//! file choosers and other file managers. Each line holds a URI, optionally
//! followed by one space and a label. Local folders are `file://` URIs; servers
//! keep their own schemes (`smb://`, `sftp://`, ...).
//!
//! Writers change only their own line and keep every other line byte for byte,
//! including lines this module cannot interpret. The file is replaced atomically.
use std::{
    ffi::OsString,
    fs, io,
    io::Write,
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
};

/// Bookmark files larger than this are not read or rewritten.
pub const MAX_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bookmark {
    pub uri: String,
    pub label: Option<String>,
}

impl Bookmark {
    /// The local folder a `file://` bookmark names, with its exact Unix bytes.
    /// Other hosts, queries, fragments and invalid escapes yield `None`.
    pub fn local_path(&self) -> Option<PathBuf> {
        local_path(&self.uri)
    }

    /// The URI scheme, lowercased.
    pub fn scheme(&self) -> Option<String> {
        let (scheme, _) = self.uri.split_once(':')?;
        (!scheme.is_empty()
            && scheme
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"+-.".contains(&byte)))
        .then(|| scheme.to_ascii_lowercase())
    }

    /// The label, or a readable form of the URI.
    pub fn display_name(&self) -> String {
        if let Some(label) = self.label.as_ref().filter(|label| !label.is_empty()) {
            return label.clone();
        }
        if let Some(path) = self.local_path() {
            return path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.to_string_lossy().into_owned());
        }
        String::from_utf8_lossy(&decode(self.uri.as_bytes()).unwrap_or_default()).into_owned()
    }

    fn line(&self) -> String {
        match self.label.as_ref().filter(|label| !label.is_empty()) {
            Some(label) => format!("{} {label}", self.uri),
            None => self.uri.clone(),
        }
    }
}

/// `$XDG_CONFIG_HOME/gtk-3.0/bookmarks`, else `$HOME/.config/gtk-3.0/bookmarks`.
pub fn default_path() -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .map(|home| home.join(".config"))
        })?;
    Some(config.join("gtk-3.0").join("bookmarks"))
}

/// Bookmarks in file order. Blank, oversized and invalid lines are skipped.
pub fn parse(text: &str) -> Vec<Bookmark> {
    text.lines().filter_map(parse_line).collect()
}

fn parse_line(line: &str) -> Option<Bookmark> {
    let line = line.trim_end_matches('\r');
    if line.trim().is_empty() || line.len() > 8192 {
        return None;
    }
    let (uri, label) = match line.split_once(' ') {
        Some((uri, label)) => (uri, Some(label.to_owned())),
        None => (line, None),
    };
    let bookmark = Bookmark {
        uri: uri.to_owned(),
        label,
    };
    (bookmark.scheme().is_some() && !uri.chars().any(char::is_control)).then_some(bookmark)
}

/// Read the bookmarks; a missing file has none.
pub fn read(path: &Path) -> io::Result<Vec<Bookmark>> {
    Ok(parse(&String::from_utf8_lossy(&read_raw(path)?)))
}

fn read_raw(path: &Path) -> io::Result<Vec<u8>> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
        Ok(_) => {}
    }
    crate::read_bounded(path, MAX_BYTES).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "the GTK bookmarks file is unreadable or too large",
        )
    })
}

/// Append a bookmark unless its URI is already listed. Returns whether it was added.
pub fn add(path: &Path, bookmark: &Bookmark) -> io::Result<bool> {
    validate(bookmark)?;
    let raw = read_raw(path)?;
    let text = String::from_utf8_lossy(&raw);
    if text
        .lines()
        .filter_map(parse_line)
        .any(|existing| existing.uri == bookmark.uri)
    {
        return Ok(false);
    }
    let mut output = raw.clone();
    if !output.is_empty() && !output.ends_with(b"\n") {
        output.push(b'\n');
    }
    output.extend_from_slice(bookmark.line().as_bytes());
    output.push(b'\n');
    replace(path, &output)?;
    Ok(true)
}

/// Remove every line naming `uri`. Returns whether anything was removed.
pub fn remove(path: &Path, uri: &str) -> io::Result<bool> {
    let raw = read_raw(path)?;
    let mut output = Vec::with_capacity(raw.len());
    let mut removed = false;
    for line in raw.split_inclusive(|byte| *byte == b'\n') {
        let text = String::from_utf8_lossy(line);
        if parse_line(text.trim_end_matches('\n')).is_some_and(|bookmark| bookmark.uri == uri) {
            removed = true;
        } else {
            output.extend_from_slice(line);
        }
    }
    if removed {
        replace(path, &output)?;
    }
    Ok(removed)
}

fn validate(bookmark: &Bookmark) -> io::Result<()> {
    let invalid = |message: &str| io::Error::new(io::ErrorKind::InvalidInput, message.to_owned());
    if bookmark.uri.is_empty()
        || bookmark.uri.len() > 4096
        || bookmark.uri.contains([' ', '\n', '\r'])
        || bookmark.uri.chars().any(char::is_control)
        || bookmark.scheme().is_none()
    {
        return Err(invalid("a bookmark needs a valid URI"));
    }
    if bookmark
        .label
        .as_ref()
        .is_some_and(|label| label.len() > 1024 || label.chars().any(char::is_control))
    {
        return Err(invalid("a bookmark label must be one short line"));
    }
    Ok(())
}

/// Write through a private temporary file in the same folder, then rename it
/// over the original, so a reader never sees a partial file.
fn replace(path: &Path, contents: &[u8]) -> io::Result<()> {
    if contents.len() > MAX_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "the GTK bookmarks file would be too large",
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "bookmarks need a folder"))?;
    fs::create_dir_all(parent)?;
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "bookmarks need a name"))?;
    let mode = fs::metadata(path)
        .map(|metadata| std::os::unix::fs::PermissionsExt::mode(&metadata.permissions()) & 0o777)
        .unwrap_or(0o644);
    for attempt in 0..16_u32 {
        let mut temporary = OsString::from(".");
        temporary.push(name);
        temporary.push(format!(".canopa-{}-{attempt}", std::process::id()));
        let temporary = parent.join(temporary);
        let mut file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        };
        let written = (|| {
            file.write_all(contents)?;
            file.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(mode))?;
            file.sync_all()?;
            fs::rename(&temporary, path)?;
            fs::File::open(parent)?.sync_all()
        })();
        if written.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        return written;
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not create a temporary bookmarks file",
    ))
}

/// The `file://` URI of an absolute local path, percent-encoding every byte
/// outside the unreserved set.
pub fn file_uri(path: &Path) -> String {
    let mut uri = String::from("file://");
    for byte in path.as_os_str().as_bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~".contains(byte) {
            uri.push(char::from(*byte));
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    uri
}

fn local_path(uri: &str) -> Option<PathBuf> {
    let rest = uri
        .strip_prefix("file://")
        .or_else(|| uri.strip_prefix("FILE://"))?;
    let slash = rest.find('/')?;
    let host = &rest[..slash];
    if !host.is_empty() && !host.eq_ignore_ascii_case("localhost") {
        return None;
    }
    let path = &rest[slash..];
    if path.contains(['?', '#']) {
        return None;
    }
    let bytes = decode(path.as_bytes())?;
    if bytes.contains(&0) {
        return None;
    }
    Some(PathBuf::from(OsString::from_vec(bytes)))
}

fn decode(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let pair = bytes.get(index + 1..index + 3)?;
            output.push(u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?);
            index += 3;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }
    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_keep_uris_labels_and_exact_local_bytes() {
        let bookmarks = parse(concat!(
            "file:///home/me/Projects\n",
            "file:///home/me/caf%C3%A9%20bar Caf\u{e9} bar\n",
            "\n",
            "smb://nas.local/share Family photos\n",
            "file:///raw/%FF\n",
            "not a uri\n",
            "file://other-host/x\n",
        ));
        assert_eq!(bookmarks.len(), 5);
        assert_eq!(
            bookmarks[0].local_path(),
            Some(PathBuf::from("/home/me/Projects"))
        );
        assert_eq!(bookmarks[1].label.as_deref(), Some("Café bar"));
        assert_eq!(
            bookmarks[1].local_path(),
            Some(PathBuf::from("/home/me/café bar"))
        );
        assert_eq!(bookmarks[2].scheme().as_deref(), Some("smb"));
        assert_eq!(bookmarks[2].local_path(), None);
        assert_eq!(bookmarks[2].display_name(), "Family photos");
        assert_eq!(
            bookmarks[3].local_path().unwrap().as_os_str().as_bytes(),
            b"/raw/\xff"
        );
        assert_eq!(bookmarks[4].local_path(), None, "another host is not local");
        assert_eq!(
            file_uri(Path::new("/raw/\u{e9} x")),
            "file:///raw/%C3%A9%20x"
        );
    }

    #[test]
    fn adding_and_removing_keep_every_other_line() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("gtk-3.0/bookmarks");
        let server = Bookmark {
            uri: "sftp://me@box/srv".into(),
            label: Some("Box".into()),
        };
        assert!(add(&path, &server).unwrap(), "the folder is created");
        assert!(!add(&path, &server).unwrap(), "no duplicates");
        fs::write(
            &path,
            "file:///a A\nweird line kept\nsftp://me@box/srv Box\nfile:///b",
        )
        .unwrap();
        let other = Bookmark {
            uri: "smb://nas/share".into(),
            label: None,
        };
        assert!(add(&path, &other).unwrap());
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "file:///a A\nweird line kept\nsftp://me@box/srv Box\nfile:///b\nsmb://nas/share\n"
        );
        assert!(remove(&path, "sftp://me@box/srv").unwrap());
        assert!(!remove(&path, "sftp://me@box/srv").unwrap());
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "file:///a A\nweird line kept\nfile:///b\nsmb://nas/share\n"
        );
        assert!(
            add(
                &path,
                &Bookmark {
                    uri: "smb://x y".into(),
                    label: None
                }
            )
            .is_err()
        );
        assert!(read(&root.path().join("missing")).unwrap().is_empty());
        let leftovers = fs::read_dir(path.parent().unwrap()).unwrap().count();
        assert_eq!(leftovers, 1, "no temporary files remain");
    }
}
