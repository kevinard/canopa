use super::*;

/// Captured dependency bytes. Debug is deliberately redacted.
pub struct ThemeSource {
    pub path: PathBuf,
    pub text: String,
    /// Range of the theme path's value in the original main document.
    pub reference_span: std::ops::Range<usize>,
}
impl std::fmt::Debug for ThemeSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThemeSource")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}
impl ThemeSource {
    pub fn snapshot_text(&self, main: &str, captured_path: &Path) -> String {
        let mut text = main.to_owned();
        let value = kdl::KdlValue::String(captured_path.to_string_lossy().into_owned()).to_string();
        text.replace_range(self.reference_span.clone(), &value);
        text
    }
    pub fn combined_fingerprint(&self, main: &str) -> String {
        combined_fingerprint(main, self)
    }
}

pub(crate) fn theme_error(mut error: Diagnostic) -> Diagnostic {
    error.source = Some(DiagnosticSource::Theme);
    error
}

fn reference(
    text: &str,
    path: Option<&Path>,
) -> Result<Option<(PathBuf, std::ops::Range<usize>)>, Diagnostic> {
    let doc = parse_kdl(text)?;
    let Some(value) = appearance::theme_reference(&doc)? else {
        return Ok(None);
    };
    if value.is_empty() {
        return Err(Diagnostic::new(Code::Appearance, None));
    }
    let target = PathBuf::from(value);
    let target = if target.is_absolute() {
        target
    } else {
        path.and_then(Path::parent)
            .ok_or_else(|| Diagnostic::new(Code::Location, None))?
            .join(target)
    };
    let node = doc
        .get("appearance")
        .unwrap()
        .children()
        .unwrap()
        .get("theme")
        .unwrap();
    let span = node.entries()[0].span();
    Ok(Some((target, span.offset()..span.offset() + span.len())))
}

/// Resolve the dependency without opening it (also useful for watching missing files).
pub fn theme_path(text: &str, path: Option<&Path>) -> Result<Option<PathBuf>, Diagnostic> {
    Ok(reference(text, path)?.map(|(path, _)| path))
}

pub fn capture_theme(text: &str, path: Option<&Path>) -> Result<Option<ThemeSource>, Diagnostic> {
    let Some((path, reference_span)) = reference(text, path)? else {
        return Ok(None);
    };
    let theme = read(&path).map_err(theme_error)?;
    if text.len().saturating_add(theme.len()) > MAX_BYTES {
        return Err(theme_error(Diagnostic::new(Code::Read, None)));
    }
    Ok(Some(ThemeSource {
        path,
        text: theme,
        reference_span,
    }))
}

pub(crate) fn combined_fingerprint(text: &str, theme: &ThemeSource) -> String {
    let mut hash = Sha256::new();
    hash.update((text.len() as u64).to_be_bytes());
    hash.update(text.as_bytes());
    hash.update(theme.path.as_os_str().as_encoded_bytes());
    hash.update([0]);
    hash.update(theme.text.as_bytes());
    format!("{:x}", hash.finalize())
}
