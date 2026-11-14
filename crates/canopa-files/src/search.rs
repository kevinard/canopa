//! Cancellable filename and content search with type, date and size filters
//! (ADR0175). Directory links are results, never traversal edges.
use std::{
    collections::{HashSet, VecDeque},
    fs,
    io::Read,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

use canopa_file::mime::MimeDatabase;

use crate::{
    Cancellation,
    browse::{Directory, Entry, EntryKind, ReadFailure},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchScope {
    CurrentFolder,
    Recursive,
    /// The desktop's recently used files, newest first (ADR0176).
    Recent,
}

/// What kind of item a result must be, from its name's shared-mime-info type.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum KindFilter {
    #[default]
    Any,
    Folders,
    Documents,
    Images,
    Audio,
    Video,
    Archives,
}

/// How recently a result must have been modified.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DateFilter {
    #[default]
    Any,
    Today,
    Week,
    Month,
    Year,
}

/// The size a file result must have. Folders never match a size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SizeFilter {
    #[default]
    Any,
    /// Under 1 MiB.
    Small,
    /// 1 MiB to 100 MiB.
    Medium,
    /// Over 100 MiB.
    Large,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SearchFilters {
    pub kind: KindFilter,
    pub modified: DateFilter,
    pub size: SizeFilter,
    /// Also match the query inside text files (ADR0175).
    pub contents: bool,
}

impl SearchFilters {
    /// Whether a filter narrows results on its own, so an empty query still
    /// searches: "every image changed this week".
    pub fn narrows(&self) -> bool {
        self.kind != KindFilter::Any
            || self.modified != DateFilter::Any
            || self.size != SizeFilter::Any
    }
}

/// Content matching reads text files up to this size whole; larger files and
/// files with a NUL byte near their start are matched by name only.
pub const CONTENT_LIMIT: u64 = 16 * 1024 * 1024;
const BINARY_PROBE: usize = 8 * 1024;
const MIB: u64 = 1024 * 1024;

#[derive(Clone, Debug)]
pub struct SearchRequest {
    pub root: PathBuf,
    pub query: String,
    pub scope: SearchScope,
    pub show_hidden: bool,
    /// The UI can explicitly increase this when results are truncated.
    pub maximum_results: usize,
    pub filters: SearchFilters,
    /// The type database for the kind filter; the shared system one when unset.
    pub mime: Option<Arc<MimeDatabase>>,
    /// The recently used list for [`SearchScope::Recent`]; the desktop's when unset.
    pub recent: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct SearchResults {
    pub root: PathBuf,
    pub query: String,
    pub scope: SearchScope,
    pub filters: SearchFilters,
    pub entries: Vec<Entry>,
    pub failures: Vec<ReadFailure>,
    pub cancelled: bool,
    pub truncated: bool,
    pub scanned: usize,
}

impl SearchRequest {
    pub fn new(root: impl AsRef<Path>, query: impl Into<String>, scope: SearchScope) -> Self {
        Self {
            root: root.as_ref().to_owned(),
            query: query.into(),
            scope,
            show_hidden: false,
            maximum_results: 50_000,
            filters: SearchFilters::default(),
            mime: None,
            recent: None,
        }
    }

    pub fn run(&self, cancellation: &Cancellation) -> SearchResults {
        self.run_with_progress(cancellation, |_| {})
    }

    pub fn run_with_progress(
        &self,
        cancellation: &Cancellation,
        mut progress: impl FnMut(&SearchResults),
    ) -> SearchResults {
        let mut results = SearchResults {
            root: self.root.clone(),
            query: self.query.clone(),
            scope: self.scope,
            filters: self.filters,
            entries: Vec::new(),
            failures: Vec::new(),
            cancelled: false,
            truncated: false,
            scanned: 0,
        };
        match std::path::absolute(&self.root) {
            Ok(root) => results.root = root,
            Err(error) => {
                results.failures.push(ReadFailure {
                    path: self.root.clone(),
                    message: error.to_string(),
                });
                return results;
            }
        }
        if self.query.is_empty() && !self.filters.narrows() && self.scope != SearchScope::Recent {
            return results;
        }
        let query = self.query.to_lowercase();
        let matcher = Matcher::new(self, &query);
        if self.scope == SearchScope::Recent {
            self.run_recent(&matcher, cancellation, &mut results);
            return results;
        }
        let mut pending = VecDeque::from([results.root.clone()]);
        let mut visited = HashSet::new();
        let mut last_report: Option<Instant> = None;
        'folders: while let Some(path) = pending.pop_front() {
            if cancellation.is_cancelled() {
                results.cancelled = true;
                break;
            }
            match fs::metadata(&path) {
                Ok(metadata) => {
                    if !visited.insert((metadata.dev(), metadata.ino())) {
                        continue;
                    }
                }
                Err(error) => {
                    results.failures.push(ReadFailure {
                        path,
                        message: error.to_string(),
                    });
                    continue;
                }
            }
            let directory = match Directory::read_cancellable(&path, cancellation) {
                Ok(directory) => directory,
                Err(_) if cancellation.is_cancelled() => {
                    results.cancelled = true;
                    break;
                }
                Err(error) => {
                    results.failures.push(ReadFailure {
                        path,
                        message: error.to_string(),
                    });
                    continue;
                }
            };
            results.failures.extend(directory.failures);
            for entry in directory.entries {
                if cancellation.is_cancelled() {
                    results.cancelled = true;
                    break 'folders;
                }
                results.scanned += 1;
                if !self.show_hidden && entry.is_hidden() {
                    continue;
                }
                if matcher.matches(&entry) {
                    if results.entries.len() >= self.maximum_results {
                        results.truncated = true;
                        break 'folders;
                    }
                    results.entries.push(entry.clone());
                }
                if self.scope == SearchScope::Recursive && entry.kind == EntryKind::Directory {
                    pending.push_back(entry.path);
                }
                if last_report.is_none_or(|last| last.elapsed() >= Duration::from_millis(100)) {
                    progress(&results);
                    last_report = Some(Instant::now());
                }
            }
        }
        results.entries.sort_by(|a, b| {
            a.name()
                .to_lowercase()
                .cmp(&b.name().to_lowercase())
                .then_with(|| a.path.cmp(&b.path))
        });
        results
    }
}

impl SearchRequest {
    /// Recent items that still exist and match, in the list's order: most
    /// recently used first. Items that are gone are left out, not reported.
    fn run_recent(
        &self,
        matcher: &Matcher<'_>,
        cancellation: &Cancellation,
        results: &mut SearchResults,
    ) {
        let Some(list) = self.recent.clone().or_else(crate::recent::default_path) else {
            return;
        };
        for (path, _) in crate::recent::read(&list) {
            if cancellation.is_cancelled() {
                results.cancelled = true;
                return;
            }
            let Ok(entry) = Entry::read(&path) else {
                continue;
            };
            results.scanned += 1;
            if !self.show_hidden && entry.is_hidden() {
                continue;
            }
            if matcher.matches(&entry) {
                if results.entries.len() >= self.maximum_results {
                    results.truncated = true;
                    return;
                }
                results.entries.push(entry);
            }
        }
    }
}

/// The per-entry test: the name or contents, then every filter.
struct Matcher<'a> {
    query: &'a str,
    filters: SearchFilters,
    mime: Option<Arc<MimeDatabase>>,
    since: Option<SystemTime>,
}

impl<'a> Matcher<'a> {
    fn new(request: &SearchRequest, query: &'a str) -> Self {
        let mime = (!matches!(request.filters.kind, KindFilter::Any | KindFilter::Folders))
            .then(|| request.mime.clone().unwrap_or_else(MimeDatabase::system));
        Self {
            query,
            filters: request.filters,
            mime,
            since: since(request.filters.modified, SystemTime::now()),
        }
    }

    fn matches(&self, entry: &Entry) -> bool {
        // Filters first: they are cheap and spare most content reads.
        self.kind(entry)
            && self
                .since
                .is_none_or(|since| entry.modified.is_some_and(|time| time >= since))
            && self.size(entry)
            && (self.query.is_empty()
                || entry.name().to_lowercase().contains(self.query)
                || (self.filters.contents && contains(&entry.path, entry, self.query)))
    }

    fn kind(&self, entry: &Entry) -> bool {
        match self.filters.kind {
            KindFilter::Any => true,
            KindFilter::Folders => entry.kind == EntryKind::Directory,
            _ if entry.kind == EntryKind::Directory => false,
            kind => self
                .mime
                .as_deref()
                .is_some_and(|database| classify(database, &entry.name()) == Some(kind)),
        }
    }

    fn size(&self, entry: &Entry) -> bool {
        if self.filters.size == SizeFilter::Any {
            return true;
        }
        if entry.kind == EntryKind::Directory {
            return false;
        }
        match self.filters.size {
            SizeFilter::Any => true,
            SizeFilter::Small => entry.size < MIB,
            SizeFilter::Medium => (MIB..=100 * MIB).contains(&entry.size),
            SizeFilter::Large => entry.size > 100 * MIB,
        }
    }
}

/// The earliest modification time a date filter accepts. "Today" starts at
/// local midnight; the others are rolling periods.
fn since(filter: DateFilter, now: SystemTime) -> Option<SystemTime> {
    const DAY: u64 = 24 * 60 * 60;
    let days = match filter {
        DateFilter::Any => return None,
        DateFilter::Today => {
            let local = chrono::DateTime::<chrono::Local>::from(now);
            let midnight = local
                .date_naive()
                .and_hms_opt(0, 0, 0)?
                .and_local_timezone(chrono::Local)
                .earliest()?;
            return Some(midnight.into());
        }
        DateFilter::Week => 7,
        DateFilter::Month => 30,
        DateFilter::Year => 365,
    };
    now.checked_sub(Duration::from_secs(days * DAY))
}

/// The kind a file name announces through its unique shared-mime-info type.
fn classify(database: &MimeDatabase, name: &str) -> Option<KindFilter> {
    if crate::archives::Format::detect(std::ffi::OsStr::new(name)).is_some() {
        return Some(KindFilter::Archives);
    }
    let types = database.by_name(name);
    let mime = types.first()?;
    let mime = database.unalias(mime);
    let top = mime.split('/').next().unwrap_or_default();
    match top {
        "image" => return Some(KindFilter::Images),
        "audio" => return Some(KindFilter::Audio),
        "video" => return Some(KindFilter::Video),
        _ => {}
    }
    const ARCHIVES: [&str; 10] = [
        "application/zip",
        "application/x-tar",
        "application/gzip",
        "application/x-7z-compressed",
        "application/vnd.rar",
        "application/x-xz",
        "application/zstd",
        "application/x-bzip2",
        "application/x-compressed-tar",
        "application/x-cd-image",
    ];
    if ARCHIVES
        .iter()
        .any(|archive| database.is_subclass(mime, archive))
    {
        return Some(KindFilter::Archives);
    }
    const DOCUMENTS: [&str; 9] = [
        "application/pdf",
        "application/postscript",
        "application/epub+zip",
        "application/rtf",
        "application/msword",
        "application/vnd.oasis.opendocument.text",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "application/vnd.ms-excel",
        "application/vnd.ms-powerpoint",
    ];
    if top == "text"
        || mime.starts_with("application/vnd.oasis.opendocument.")
        || mime.starts_with("application/vnd.openxmlformats-officedocument.")
        || DOCUMENTS
            .iter()
            .any(|document| database.is_subclass(mime, document))
    {
        return Some(KindFilter::Documents);
    }
    None
}

/// Whether a text file contains the lowercase query, ignoring case. Binary
/// files, unreadable files and files over [`CONTENT_LIMIT`] do not match.
fn contains(path: &Path, entry: &Entry, query: &str) -> bool {
    if entry.kind != EntryKind::File || entry.size > CONTENT_LIMIT || entry.size == 0 {
        return false;
    }
    let Ok(file) = fs::File::open(path) else {
        return false;
    };
    let mut bytes = Vec::with_capacity(entry.size as usize);
    if file.take(CONTENT_LIMIT).read_to_end(&mut bytes).is_err() {
        return false;
    }
    if bytes[..bytes.len().min(BINARY_PROBE)].contains(&0) {
        return false;
    }
    String::from_utf8_lossy(&bytes)
        .to_lowercase()
        .contains(query)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn database(root: &Path) -> Arc<MimeDatabase> {
        let dir = root.join("mime");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("globs2"),
            "50:image/png:*.png\n50:audio/flac:*.flac\n50:video/webm:*.webm\n\
             50:application/pdf:*.pdf\n50:text/markdown:*.md\n\
             50:application/x-7z-compressed:*.7z\n",
        )
        .unwrap();
        Arc::new(MimeDatabase::load(&[dir]))
    }

    fn names(results: &SearchResults) -> Vec<String> {
        results
            .entries
            .iter()
            .map(|entry| entry.name().into_owned())
            .collect()
    }

    fn request(root: &Path, query: &str, filters: SearchFilters) -> SearchRequest {
        let mut request = SearchRequest::new(root, query, SearchScope::Recursive);
        request.filters = filters;
        request.mime = Some(database(&root.join(".types")));
        request
    }

    #[test]
    fn kind_filters_use_the_type_database_and_archive_names() {
        let root = tempfile::tempdir().unwrap();
        let files = root.path().join("files");
        fs::create_dir(&files).unwrap();
        for name in [
            "photo.png",
            "song.flac",
            "clip.webm",
            "paper.pdf",
            "notes.md",
            "bundle.7z",
            "backup.tar.gz",
            "unknown.bin",
        ] {
            fs::write(files.join(name), b"x").unwrap();
        }
        fs::create_dir(files.join("album")).unwrap();
        let expect = |kind, expected: &[&str]| {
            let results = request(
                &files,
                "",
                SearchFilters {
                    kind,
                    ..Default::default()
                },
            )
            .run(&Cancellation::default());
            assert_eq!(names(&results), expected, "{kind:?}");
        };
        expect(KindFilter::Images, &["photo.png"]);
        expect(KindFilter::Audio, &["song.flac"]);
        expect(KindFilter::Video, &["clip.webm"]);
        expect(KindFilter::Documents, &["notes.md", "paper.pdf"]);
        expect(KindFilter::Archives, &["backup.tar.gz", "bundle.7z"]);
        expect(KindFilter::Folders, &["album"]);
    }

    #[test]
    fn date_and_size_filters_narrow_files_and_size_skips_folders() {
        let root = tempfile::tempdir().unwrap();
        let old = root.path().join("old.txt");
        fs::write(&old, b"x").unwrap();
        fs::File::options()
            .write(true)
            .open(&old)
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(40 * 24 * 60 * 60))
            .unwrap();
        fs::write(root.path().join("new.txt"), b"x").unwrap();
        let large = fs::File::create(root.path().join("large.img")).unwrap();
        large.set_len(200 * MIB).unwrap();
        fs::create_dir(root.path().join("folder")).unwrap();
        let week = request(
            root.path(),
            "",
            SearchFilters {
                modified: DateFilter::Week,
                ..Default::default()
            },
        )
        .run(&Cancellation::default());
        assert_eq!(names(&week), ["folder", "large.img", "new.txt"]);
        let year = request(
            root.path(),
            "txt",
            SearchFilters {
                modified: DateFilter::Year,
                ..Default::default()
            },
        )
        .run(&Cancellation::default());
        assert_eq!(names(&year), ["new.txt", "old.txt"]);
        let small = request(
            root.path(),
            "",
            SearchFilters {
                size: SizeFilter::Small,
                ..Default::default()
            },
        )
        .run(&Cancellation::default());
        assert_eq!(names(&small), ["new.txt", "old.txt"]);
        let large = request(
            root.path(),
            "",
            SearchFilters {
                size: SizeFilter::Large,
                ..Default::default()
            },
        )
        .run(&Cancellation::default());
        assert_eq!(names(&large), ["large.img"]);
    }

    #[test]
    fn content_search_matches_text_ignoring_case_and_skips_binary_and_large_files() {
        let root = tempfile::tempdir().unwrap();
        fs::write(
            root.path().join("letter.txt"),
            "Dear Élise, the MEETING moved.",
        )
        .unwrap();
        fs::write(root.path().join("other.txt"), "nothing here").unwrap();
        let mut binary = b"meeting".to_vec();
        binary.push(0);
        fs::write(root.path().join("blob.bin"), binary).unwrap();
        let huge = fs::File::create(root.path().join("huge.log")).unwrap();
        huge.set_len(CONTENT_LIMIT + 1).unwrap();
        std::os::unix::fs::FileExt::write_all_at(&huge, b"meeting", 0).unwrap();
        let contents = SearchFilters {
            contents: true,
            ..Default::default()
        };
        let results = request(root.path(), "meeting", contents).run(&Cancellation::default());
        assert_eq!(names(&results), ["letter.txt"]);
        let accented = request(root.path(), "élise", contents).run(&Cancellation::default());
        assert_eq!(names(&accented), ["letter.txt"]);
        // Without the option only names match.
        let names_only =
            request(root.path(), "meeting", SearchFilters::default()).run(&Cancellation::default());
        assert!(names_only.entries.is_empty());
        // Contents alone does not search with an empty query.
        let empty = request(root.path(), "", contents).run(&Cancellation::default());
        assert!(empty.entries.is_empty() && empty.scanned == 0);
    }

    #[test]
    fn recent_lists_existing_items_newest_first_through_the_query_and_filters() {
        let root = tempfile::tempdir().unwrap();
        let older = root.path().join("older report.pdf");
        let newer = root.path().join("newer.png");
        fs::write(&older, b"x").unwrap();
        fs::write(&newer, b"x").unwrap();
        let gone = root.path().join("gone.txt");
        let href = |path: &Path| url::Url::from_file_path(path).unwrap().to_string();
        let list = root.path().join("recently-used.xbel");
        fs::write(
            &list,
            format!(
                r#"<?xml version="1.0"?><xbel version="1.0">
<bookmark href="{}" added="2026-01-01T00:00:00Z" visited="2026-01-03T00:00:00Z"/>
<bookmark href="{}" added="2026-01-05T00:00:00Z"/>
<bookmark href="{}" added="2026-01-09T00:00:00Z"/>
</xbel>"#,
                href(&older),
                href(&newer),
                href(&gone)
            ),
        )
        .unwrap();
        let recent = |query: &str, filters: SearchFilters| {
            let mut request = SearchRequest::new(root.path(), query, SearchScope::Recent);
            request.recent = Some(list.clone());
            request.filters = filters;
            request.mime = Some(database(&root.path().join(".types")));
            names(&request.run(&Cancellation::default()))
        };
        // Newest first, not by name; the missing item is left out.
        assert_eq!(
            recent("", SearchFilters::default()),
            ["newer.png", "older report.pdf"]
        );
        assert_eq!(
            recent("report", SearchFilters::default()),
            ["older report.pdf"]
        );
        let images = SearchFilters {
            kind: KindFilter::Images,
            ..Default::default()
        };
        assert_eq!(recent("", images), ["newer.png"]);
    }

    #[test]
    fn today_starts_at_local_midnight() {
        let now = SystemTime::now();
        let midnight = since(DateFilter::Today, now).unwrap();
        assert!(midnight <= now);
        assert!(now.duration_since(midnight).unwrap() <= Duration::from_secs(25 * 60 * 60));
        assert!(since(DateFilter::Any, now).is_none());
    }
}
