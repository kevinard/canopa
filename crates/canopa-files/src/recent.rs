//! Recently used files from the desktop's shared `recently-used.xbel`
//! (ADR0176), the list GTK, Qt and most applications record opened files in.
//! Files reads it, and records the documents it opens with an application
//! (ADR0178), as GTK applications do.
use std::{
    fs,
    io::{self, Read, Write},
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::Mutex,
    time::SystemTime,
};

/// The most recent items read; older ones are ignored.
pub const LIMIT: usize = 500;
/// A larger list is not a recent-files list and is not read.
const FILE_LIMIT: u64 = 16 * 1024 * 1024;

/// `$XDG_DATA_HOME/recently-used.xbel`, defaulting to `~/.local/share`.
pub fn default_path() -> Option<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".local/share")))?;
    Some(data.join("recently-used.xbel"))
}

/// Local files in the list, most recently used first, each once. Missing,
/// unreadable or malformed lists are empty.
pub fn read(path: &Path) -> Vec<(PathBuf, SystemTime)> {
    let Ok(file) = fs::File::open(path) else {
        return Vec::new();
    };
    let mut text = String::new();
    if file.take(FILE_LIMIT).read_to_string(&mut text).is_err() {
        return Vec::new();
    }
    parse(&text)
}

pub fn parse(text: &str) -> Vec<(PathBuf, SystemTime)> {
    let Ok(document) = roxmltree::Document::parse(text) else {
        return Vec::new();
    };
    let mut items: Vec<(PathBuf, SystemTime)> = document
        .root_element()
        .children()
        .filter(|node| node.has_tag_name("bookmark"))
        .filter_map(|bookmark| {
            let url = url::Url::parse(bookmark.attribute("href")?).ok()?;
            if url.scheme() != "file" {
                return None;
            }
            let path = url.to_file_path().ok()?;
            // The latest of the times the list records for the item.
            let used = ["visited", "modified", "added"]
                .into_iter()
                .filter_map(|name| bookmark.attribute(name))
                .filter_map(|time| chrono::DateTime::parse_from_rfc3339(time).ok())
                .map(SystemTime::from)
                .max()?;
            Some((path, used))
        })
        .collect();
    items.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut seen = std::collections::HashSet::new();
    items.retain(|(path, _)| seen.insert(path.clone()));
    items.truncate(LIMIT);
    items
}

const BOOKMARK_NS: &str = "http://www.freedesktop.org/standards/desktop-bookmarks";
const MIME_NS: &str = "http://www.freedesktop.org/standards/shared-mime-info";
const EMPTY: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<xbel version=\"1.0\"\n      xmlns:bookmark=\"http://www.freedesktop.org/standards/desktop-bookmarks\"\n      xmlns:mime=\"http://www.freedesktop.org/standards/shared-mime-info\"\n>\n</xbel>\n";
/// Attempts when another program replaces the list between reading and writing.
const ATTEMPTS: usize = 3;
/// One writer in this process at a time; other programs are detected by
/// identity and retried.
static WRITING: Mutex<()> = Mutex::new(());

/// A document opened with an application.
#[derive(Clone, Debug)]
pub struct Use {
    pub path: PathBuf,
    pub mime: String,
    /// The application's name, as GTK records it.
    pub application: String,
    /// Its command line, such as `'gedit %u'`.
    pub exec: String,
}

/// Record `uses` at `now` in the list at `list`: a known item gets new
/// `modified` and `visited` times and its application's count goes up; a new
/// item is added with its type and application. Everything else in the file is
/// kept byte for byte. The list is replaced atomically; a malformed or
/// oversized list is left alone and reported.
pub fn add(list: &Path, uses: &[Use], now: SystemTime) -> io::Result<()> {
    if uses.is_empty() {
        return Ok(());
    }
    let _writing = WRITING.lock().unwrap_or_else(|poison| poison.into_inner());
    let list = match fs::symlink_metadata(list) {
        Ok(metadata) if metadata.file_type().is_symlink() => fs::canonicalize(list)?,
        _ => list.to_path_buf(),
    };
    let parent = list
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no folder"))?;
    fs::create_dir_all(parent)?;
    let time = stamp(now);
    for _ in 0..ATTEMPTS {
        let (text, seen, mode) = match fs::File::open(&list) {
            Ok(file) => {
                let metadata = file.metadata()?;
                if metadata.len() > FILE_LIMIT {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "list too large"));
                }
                let mut text = String::new();
                file.take(FILE_LIMIT).read_to_string(&mut text)?;
                (
                    text,
                    Some(version(&metadata)),
                    metadata.permissions().mode() & 0o777,
                )
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                (EMPTY.to_owned(), None, 0o600)
            }
            Err(error) => return Err(error),
        };
        let updated = edit(&text, uses, &time)?;
        let mut staged = tempfile::Builder::new()
            .prefix(".recently-used.xbel.")
            .tempfile_in(parent)?;
        staged.write_all(updated.as_bytes())?;
        staged
            .as_file()
            .set_permissions(fs::Permissions::from_mode(mode))?;
        staged.as_file().sync_all()?;
        let current = match fs::metadata(&list) {
            Ok(metadata) => Some(version(&metadata)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };
        if current != seen {
            // Another program wrote the list meanwhile: start again from its version.
            continue;
        }
        staged.persist(&list).map_err(|error| error.error)?;
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::WouldBlock,
        "the list kept changing",
    ))
}

/// What tells two versions of the list apart.
fn version(metadata: &fs::Metadata) -> (u64, u64, u64, i64, i64) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.size(),
        metadata.mtime(),
        metadata.mtime_nsec(),
    )
}

/// The ISO 8601 form GLib writes, with microseconds.
fn stamp(time: SystemTime) -> String {
    chrono::DateTime::<chrono::Utc>::from(time)
        .format("%Y-%m-%dT%H:%M:%S%.6fZ")
        .to_string()
}

fn escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            character => escaped.push(character),
        }
    }
    escaped
}

/// The text with `uses` recorded, as splices of the original.
fn edit(text: &str, uses: &[Use], time: &str) -> io::Result<String> {
    let invalid = |message: &str| io::Error::new(io::ErrorKind::InvalidData, message.to_owned());
    let document = roxmltree::Document::parse(text).map_err(|_| invalid("malformed list"))?;
    let root = document.root_element();
    if !root.has_tag_name("xbel") {
        return Err(invalid("not a bookmark list"));
    }
    let mut splices: Vec<(std::ops::Range<usize>, String)> = Vec::new();
    let mut added = String::new();
    let mut recorded: Vec<&Path> = Vec::new();
    for item in uses {
        if recorded.contains(&item.path.as_path()) {
            continue;
        }
        recorded.push(&item.path);
        let Ok(url) = url::Url::from_file_path(&item.path) else {
            continue;
        };
        let known = root
            .children()
            .filter(|node| node.has_tag_name("bookmark"))
            .find(|bookmark| {
                bookmark
                    .attribute("href")
                    .and_then(|href| url::Url::parse(href).ok())
                    .and_then(|href| href.to_file_path().ok())
                    .is_some_and(|path| path == item.path)
            });
        match known {
            Some(bookmark) => update(text, bookmark, item, time, &mut splices),
            None => added.push_str(&new_bookmark(root, &url, item, time)),
        }
    }
    if !added.is_empty() {
        let end = closing_tag(text, root).ok_or_else(|| invalid("unclosed list"))?;
        splices.push((end..end, added));
    }
    splices.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    let mut updated = text.to_owned();
    for (range, replacement) in splices {
        updated.replace_range(range, &replacement);
    }
    Ok(updated)
}

/// Where an element's closing tag starts; `None` for `<empty/>` elements.
fn closing_tag(text: &str, node: roxmltree::Node) -> Option<usize> {
    let range = node.range();
    let element = &text[range.clone()];
    if element.ends_with("/>") {
        return None;
    }
    element.rfind("</").map(|offset| range.start + offset)
}

/// New times on a known item, and its application counted once more.
fn update(
    text: &str,
    bookmark: roxmltree::Node,
    item: &Use,
    time: &str,
    splices: &mut Vec<(std::ops::Range<usize>, String)>,
) {
    set_attributes(
        text,
        bookmark,
        &[("modified", time), ("visited", time)],
        splices,
    );
    let applications = bookmark.descendants().find(|node| {
        node.tag_name().namespace() == Some(BOOKMARK_NS) && node.tag_name().name() == "applications"
    });
    let Some(applications) = applications else {
        return;
    };
    let known = applications.children().find(|node| {
        node.tag_name().namespace() == Some(BOOKMARK_NS)
            && node.tag_name().name() == "application"
            && node.attribute("name") == Some(item.application.as_str())
    });
    match known {
        Some(application) => {
            let count = application
                .attribute("count")
                .and_then(|count| count.parse::<u64>().ok())
                .unwrap_or(0)
                .saturating_add(1)
                .to_string();
            // Older GLib versions wrote seconds in `timestamp`; newer ones `modified`.
            let changed: &[(&str, &str)] = if application.attribute("timestamp").is_some()
                && application.attribute("modified").is_none()
            {
                &[("count", &count)]
            } else {
                &[("modified", time), ("count", &count)]
            };
            set_attributes(text, application, changed, splices);
        }
        None => {
            let Some(end) = closing_tag(text, applications) else {
                return;
            };
            let prefix = applications
                .lookup_prefix(BOOKMARK_NS)
                .map(|prefix| format!("{prefix}:"))
                .unwrap_or_default();
            // Before the closing tag, at its indentation plus one level.
            let indent: String = text[..end]
                .chars()
                .rev()
                .take_while(|c| *c == ' ' || *c == '\t')
                .collect();
            splices.push((
                end..end,
                format!(
                    "  <{prefix}application name=\"{}\" exec=\"{}\" modified=\"{time}\" count=\"1\"/>\n{indent}",
                    escape(&item.application),
                    escape(&item.exec)
                ),
            ));
        }
    }
}

/// Replace the values of `attributes` on `node`, adding those it lacks to its
/// start tag.
fn set_attributes(
    text: &str,
    node: roxmltree::Node,
    attributes: &[(&str, &str)],
    splices: &mut Vec<(std::ops::Range<usize>, String)>,
) {
    let mut missing = String::new();
    for (name, value) in attributes {
        match node
            .attributes()
            .find(|attribute| attribute.namespace().is_none() && attribute.name() == *name)
        {
            Some(attribute) => splices.push((attribute.range_value(), escape(value))),
            None => missing.push_str(&format!(" {name}=\"{}\"", escape(value))),
        }
    }
    if missing.is_empty() {
        return;
    }
    // After the last attribute, or after the tag name when there is none.
    let start = node.range().start;
    let at = node
        .attributes()
        .map(|attribute| attribute.range().end)
        .max()
        .unwrap_or_else(|| {
            let name = &text[start + 1..];
            start
                + 1
                + name
                    .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
                    .unwrap_or(0)
        });
    splices.push((at..at, missing));
}

fn new_bookmark(root: roxmltree::Node, url: &url::Url, item: &Use, time: &str) -> String {
    let (bookmark, mime, declarations) =
        match (root.lookup_prefix(BOOKMARK_NS), root.lookup_prefix(MIME_NS)) {
            (Some("bookmark"), Some("mime")) => ("bookmark", "mime", String::new()),
            _ => (
                "bookmark",
                "mime",
                format!(" xmlns:bookmark=\"{BOOKMARK_NS}\" xmlns:mime=\"{MIME_NS}\""),
            ),
        };
    format!(
        "  <bookmark href=\"{href}\" added=\"{time}\" modified=\"{time}\" visited=\"{time}\"{declarations}>\n    <info>\n      <metadata owner=\"http://freedesktop.org\">\n        <{mime}:mime-type type=\"{kind}\"/>\n        <{bookmark}:applications>\n          <{bookmark}:application name=\"{name}\" exec=\"{exec}\" modified=\"{time}\" count=\"1\"/>\n        </{bookmark}:applications>\n      </metadata>\n    </info>\n  </bookmark>\n",
        href = escape(url.as_str()),
        kind = escape(&item.mime),
        name = escape(&item.application),
        exec = escape(&item.exec),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::ffi::OsStrExt;

    #[test]
    fn local_items_come_newest_first_once_with_raw_bytes() {
        let text = r#"<?xml version="1.0" encoding="UTF-8"?>
<xbel version="1.0" xmlns:bookmark="http://www.freedesktop.org/standards/desktop-bookmarks"
      xmlns:mime="http://www.freedesktop.org/standards/shared-mime-info">
  <bookmark href="file:///home/u/old.txt" added="2026-01-01T10:00:00Z" modified="2026-01-02T10:00:00Z" visited="2026-01-01T10:00:00Z">
    <info><metadata owner="http://freedesktop.org"><mime:mime-type type="text/plain"/></metadata></info>
  </bookmark>
  <bookmark href="file:///home/u/caf%C3%A9%20menu.pdf" added="2026-03-01T10:00:00.123456Z" visited="2026-03-05T10:00:00Z"/>
  <bookmark href="file:///home/u/raw-%FF" added="2026-02-01T10:00:00Z"/>
  <bookmark href="https://example.org/page" added="2026-04-01T10:00:00Z"/>
  <bookmark href="file:///home/u/old.txt" added="2025-01-01T10:00:00Z"/>
  <bookmark href="file:///home/u/undated"/>
</xbel>"#;
        let items = parse(text);
        let paths: Vec<_> = items.iter().map(|(path, _)| path.clone()).collect();
        assert_eq!(
            paths,
            [
                PathBuf::from("/home/u/café menu.pdf"),
                PathBuf::from(std::ffi::OsStr::from_bytes(b"/home/u/raw-\xff")),
                PathBuf::from("/home/u/old.txt"),
            ]
        );
        assert!(parse("not xml").is_empty());
        assert!(read(Path::new("/nonexistent/recently-used.xbel")).is_empty());
    }

    fn opened(path: &str, application: &str) -> Use {
        Use {
            path: PathBuf::from(path),
            mime: "text/plain".into(),
            application: application.into(),
            exec: format!("'{}' %u", application.to_lowercase()),
        }
    }

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(seconds)
    }

    #[test]
    fn a_first_use_creates_a_private_list_that_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let list = dir.path().join("share/recently-used.xbel");
        add(
            &list,
            &[opened("/home/u/a & b.txt", "Editor")],
            at(1_800_000_000),
        )
        .unwrap();
        let mode = fs::metadata(&list).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        assert_eq!(
            read(&list),
            [(PathBuf::from("/home/u/a & b.txt"), at(1_800_000_000))]
        );
        let text = fs::read_to_string(&list).unwrap();
        assert!(
            text.contains(r#"<mime:mime-type type="text/plain"/>"#),
            "{text}"
        );
        assert!(
            text.contains(r#"name="Editor" exec="&apos;editor&apos; %u""#),
            "{text}"
        );
        assert!(
            text.contains(r#"href="file:///home/u/a%20&amp;%20b.txt""#),
            "{text}"
        );
        let leftovers: Vec<_> = fs::read_dir(list.parent().unwrap()).unwrap().collect();
        assert_eq!(leftovers.len(), 1, "no staged file is left behind");
    }

    #[test]
    fn a_known_item_is_counted_and_everything_else_is_kept_byte_for_byte() {
        let original = r#"<?xml version="1.0" encoding="UTF-8"?>
<xbel version="1.0"
      xmlns:bookmark="http://www.freedesktop.org/standards/desktop-bookmarks"
      xmlns:mime="http://www.freedesktop.org/standards/shared-mime-info"
>
  <bookmark href="file:///home/u/other.pdf" added="2026-01-01T10:00:00.000000Z" modified="2026-01-01T10:00:00.000000Z" visited="2026-01-01T10:00:00.000000Z">
    <info>
      <metadata owner="http://freedesktop.org">
        <mime:mime-type type="application/pdf"/>
        <bookmark:groups><bookmark:group>Private</bookmark:group></bookmark:groups>
        <bookmark:applications>
          <bookmark:application name="Viewer" exec="&apos;viewer %u&apos;" modified="2026-01-01T10:00:00Z" count="3"/>
        </bookmark:applications>
        <bookmark:private/>
      </metadata>
    </info>
  </bookmark>
  <bookmark href="file:///home/u/notes.txt" added="2026-02-01T10:00:00Z" modified="2026-02-01T10:00:00Z">
    <info>
      <metadata owner="http://freedesktop.org">
        <mime:mime-type type="text/plain"/>
        <bookmark:applications>
          <bookmark:application name="Editor" exec="&apos;editor %u&apos;" modified="2026-02-01T10:00:00Z" count="4"/>
        </bookmark:applications>
      </metadata>
    </info>
  </bookmark>
</xbel>
"#;
        let dir = tempfile::tempdir().unwrap();
        let list = dir.path().join("recently-used.xbel");
        fs::write(&list, original).unwrap();
        fs::set_permissions(&list, fs::Permissions::from_mode(0o640)).unwrap();
        let now = at(1_800_000_000);
        add(
            &list,
            &[
                opened("/home/u/notes.txt", "Editor"),
                opened("/home/u/other.pdf", "Reader"),
            ],
            now,
        )
        .unwrap();
        let text = fs::read_to_string(&list).unwrap();
        assert_eq!(
            fs::metadata(&list).unwrap().permissions().mode() & 0o777,
            0o640
        );
        let stamp = stamp(now);
        assert!(text.contains(&format!(
            r#"<bookmark href="file:///home/u/notes.txt" added="2026-02-01T10:00:00Z" modified="{stamp}" visited="{stamp}">"#
        )), "{text}");
        assert!(
            text.contains(&format!(
                r#"name="Editor" exec="&apos;editor %u&apos;" modified="{stamp}" count="5"/>"#
            )),
            "{text}"
        );
        assert!(text.contains(&format!(
            "          <bookmark:application name=\"Viewer\" exec=\"&apos;viewer %u&apos;\" modified=\"2026-01-01T10:00:00Z\" count=\"3\"/>\n          <bookmark:application name=\"Reader\" exec=\"&apos;reader&apos; %u\" modified=\"{stamp}\" count=\"1\"/>\n        </bookmark:applications>\n        <bookmark:private/>"
        )), "{text}");
        assert!(text.contains(
            "<bookmark:groups><bookmark:group>Private</bookmark:group></bookmark:groups>"
        ));
        let items = read(&list);
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|(_, used)| *used == now));
        // Only the changed values differ from the original.
        let restored = text.replace(&stamp, "X");
        assert_eq!(
            restored.matches('X').count() - original.matches('X').count(),
            6
        );
    }

    #[test]
    fn foreign_prefixes_get_their_own_declarations() {
        let original = r#"<?xml version="1.0"?>
<xbel version="1.0" xmlns:b="http://www.freedesktop.org/standards/desktop-bookmarks">
</xbel>
"#;
        let dir = tempfile::tempdir().unwrap();
        let list = dir.path().join("recently-used.xbel");
        fs::write(&list, original).unwrap();
        add(
            &list,
            &[opened("/home/u/a.txt", "Editor")],
            at(1_800_000_000),
        )
        .unwrap();
        let text = fs::read_to_string(&list).unwrap();
        roxmltree::Document::parse(&text).expect("still well formed");
        assert!(text.contains(
            r#"xmlns:bookmark="http://www.freedesktop.org/standards/desktop-bookmarks""#
        ));
        assert_eq!(read(&list).len(), 1);
    }

    #[test]
    fn a_malformed_list_belonging_to_others_is_not_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let list = dir.path().join("recently-used.xbel");
        fs::write(&list, "<xbel><bookmark").unwrap();
        let error = add(&list, &[opened("/home/u/a.txt", "Editor")], at(1)).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(fs::read_to_string(&list).unwrap(), "<xbel><bookmark");
    }
}
