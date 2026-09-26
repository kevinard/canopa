//! Shared configuration intake. Applications own their namespace schemas and installation.
use std::path::{Path, PathBuf};

use kdl::KdlDocument;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MAX_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Component {
    Shell,
    Notifications,
}

impl Component {
    pub fn namespace(self) -> &'static str {
        match self {
            Self::Shell => "shell",
            Self::Notifications => "notifications",
        }
    }

    fn legacy_directory(self) -> &'static str {
        match self {
            Self::Shell => "niribar",
            Self::Notifications => "canopa-notify",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Format {
    CanopaV1,
    Legacy,
}

/// Explicit formats never fall back or attempt schema guessing.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Request {
    Discover,
    #[default]
    Defaults,
    File {
        path: PathBuf,
        format: Format,
    },
}

impl Request {
    pub fn legacy(path: impl Into<PathBuf>) -> Self {
        Self::File {
            path: path.into(),
            format: Format::Legacy,
        }
    }
    pub fn common(path: impl Into<PathBuf>) -> Self {
        Self::File {
            path: path.into(),
            format: Format::CanopaV1,
        }
    }

    /// Resolve environment and relative paths once, before preparing a candidate.
    pub fn resolve(&self, component: Component) -> Result<Self, Diagnostic> {
        match self {
            Self::Discover => discover_in(&config_home()?, component),
            Self::File { path, format } => Ok(Self::File {
                path: std::path::absolute(path)
                    .map_err(|_| Diagnostic::new(Code::Location, None))?,
                format: *format,
            }),
            Self::Defaults => Ok(Self::Defaults),
        }
    }
}

/// Empty or relative XDG_CONFIG_HOME is ignored. HOME must also be absolute.
pub fn config_home() -> Result<PathBuf, Diagnostic> {
    config_home_from(
        std::env::var_os("XDG_CONFIG_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    )
}

pub fn config_home_from(
    xdg: Option<&std::ffi::OsStr>,
    home: Option<&std::ffi::OsStr>,
) -> Result<PathBuf, Diagnostic> {
    xdg.map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            home.map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .map(|path| path.join(".config"))
        })
        .ok_or_else(|| Diagnostic::new(Code::Location, None))
}

pub fn discover_in(base: &Path, component: Component) -> Result<Request, Diagnostic> {
    for (path, format) in [
        (base.join("canopa/config.kdl"), Format::CanopaV1),
        (
            base.join(component.legacy_directory()).join("config.kdl"),
            Format::Legacy,
        ),
    ] {
        // A dangling symlink is a present invalid source, never permission to fall back.
        match std::fs::symlink_metadata(&path) {
            Ok(_) => return Ok(Request::File { path, format }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(Diagnostic::new(Code::Read, None)),
        }
    }
    Ok(Request::Defaults)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceInfo {
    pub path: Option<PathBuf>,
    pub format: Option<Format>,
    /// SHA-256 of the entire selected source, including other namespaces and comments.
    pub fingerprint: String,
    /// Missing namespaces use embedded defaults without consulting legacy files.
    pub defaults: bool,
}

/// Safe for logs, status, Display and Debug: never contains source text or parser errors.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: Code,
    pub offset: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Code {
    Location,
    Read,
    Syntax,
    Envelope,
    Schema,
}

impl Diagnostic {
    pub fn new(code: Code, offset: Option<usize>) -> Self {
        Self { code, offset }
    }
}
impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.code {
            Code::Location => "configuration requires an absolute XDG_CONFIG_HOME or HOME",
            Code::Read => "configuration must be a readable regular UTF-8 file of at most 1 MiB",
            Code::Syntax => "invalid configuration KDL",
            Code::Envelope => "expected schema-version 1 and unique shell/notifications blocks without arguments or annotations",
            Code::Schema => "invalid component configuration; use the local configuration checker for details",
        })?;
        if let Some(offset) = self.offset {
            write!(f, " near byte {offset}")?;
        }
        Ok(())
    }
}
impl std::error::Error for Diagnostic {}

pub fn read(path: &Path) -> Result<String, Diagnostic> {
    String::from_utf8(
        canopa_file::read_bounded(path, MAX_BYTES)
            .map_err(|_| Diagnostic::new(Code::Read, None))?,
    )
    .map_err(|_| Diagnostic::new(Code::Read, None))
}

/// Bounded immutable input retained by explicit local diagnostics. Debug never shows contents.
pub struct Input {
    text: String,
    path: Option<PathBuf>,
    format: Option<Format>,
}
impl std::fmt::Debug for Input {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Input")
            .field("path", &self.path)
            .field("format", &self.format)
            .finish_non_exhaustive()
    }
}
impl Input {
    pub fn load(
        request: &Request,
        component: Component,
        defaults: &str,
    ) -> Result<Self, Diagnostic> {
        match request.resolve(component)? {
            Request::File { path, format } => Ok(Self {
                text: read(&path)?,
                path: Some(path),
                format: Some(format),
            }),
            Request::Defaults => Ok(Self {
                text: defaults.into(),
                path: None,
                format: None,
            }),
            Request::Discover => unreachable!(),
        }
    }
    pub fn select(&self, component: Component, defaults: &str) -> Result<Document, Diagnostic> {
        match self.format {
            Some(format) => Document::parse(
                self.text.clone(),
                self.path.clone(),
                format,
                component,
                defaults,
            ),
            None => Document::embedded(&self.text),
        }
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
}

/// Owns the original text and parsed document so namespace spans remain absolute.
/// Source text is accessible only explicitly; Debug is redacted.
pub struct Document {
    text: String,
    document: KdlDocument,
    info: SourceInfo,
    namespace: Option<&'static str>,
}
impl std::fmt::Debug for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Document")
            .field("info", &self.info)
            .finish_non_exhaustive()
    }
}
impl Document {
    pub fn load(
        request: &Request,
        component: Component,
        defaults: &str,
    ) -> Result<Self, Diagnostic> {
        Input::load(request, component, defaults)?.select(component, defaults)
    }

    fn embedded(defaults: &str) -> Result<Self, Diagnostic> {
        Ok(Self {
            text: defaults.into(),
            document: parse_kdl(defaults)?,
            namespace: None,
            info: SourceInfo {
                path: None,
                format: None,
                fingerprint: fingerprint(defaults),
                defaults: true,
            },
        })
    }

    pub fn parse(
        text: String,
        path: Option<PathBuf>,
        format: Format,
        component: Component,
        defaults: &str,
    ) -> Result<Self, Diagnostic> {
        if text.len() > MAX_BYTES {
            return Err(Diagnostic::new(Code::Read, None));
        }
        let document = parse_kdl(&text)?;
        let info = SourceInfo {
            path,
            format: Some(format),
            fingerprint: fingerprint(&text),
            defaults: false,
        };
        if format == Format::CanopaV1 {
            validate_envelope(&document)?;
            if document.get(component.namespace()).is_none() {
                let mut fallback = Self::embedded(defaults)?;
                fallback.info = SourceInfo {
                    defaults: true,
                    ..info
                };
                return Ok(fallback);
            }
        }
        Ok(Self {
            text,
            document,
            info,
            namespace: (format == Format::CanopaV1).then_some(component.namespace()),
        })
    }
    pub fn selected(&self) -> &KdlDocument {
        match self.namespace {
            Some(namespace) => self.document.get(namespace).unwrap().children().unwrap(),
            None => &self.document,
        }
    }
    /// Text associated with selected spans (embedded text when the namespace defaults).
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn info(&self) -> &SourceInfo {
        &self.info
    }
}

fn fingerprint(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

pub fn parse_kdl(text: &str) -> Result<KdlDocument, Diagnostic> {
    text.parse().map_err(|error: kdl::KdlError| {
        Diagnostic::new(
            Code::Syntax,
            error.diagnostics.first().map(|d| d.span.offset()),
        )
    })
}

fn validate_envelope(document: &KdlDocument) -> Result<(), Diagnostic> {
    let mut seen = std::collections::HashSet::new();
    for node in document.nodes() {
        let fail = || Diagnostic::new(Code::Envelope, Some(node.span().offset()));
        if node.ty().is_some() || !seen.insert(node.name().value()) {
            return Err(fail());
        }
        match node.name().value() {
            "schema-version" => {
                if node.children().is_some() || node.entries().len() != 1 {
                    return Err(fail());
                }
                let entry = &node.entries()[0];
                if entry.name().is_some()
                    || entry.ty().is_some()
                    || entry.value().as_integer() != Some(1)
                {
                    return Err(fail());
                }
            }
            "shell" | "notifications" => {
                if !node.entries().is_empty() || node.children().is_none() {
                    return Err(fail());
                }
            }
            _ => return Err(fail()),
        }
    }
    if !seen.contains("schema-version") {
        return Err(Diagnostic::new(Code::Envelope, None));
    }
    Ok(())
}
