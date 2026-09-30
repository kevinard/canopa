use std::{fmt::Display, path::PathBuf, sync::Arc};

use miette::{Diagnostic, LabeledSpan, NamedSource, Severity, SourceCode, SourceSpan};

pub(crate) type ConfigResult<T> = Result<T, ConfigError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConfigSourceName {
    UserFile(PathBuf),
    EmbeddedDefault,
    #[cfg(test)]
    Test(String),
}

impl ConfigSourceName {
    #[cfg(test)]
    pub(crate) fn test(name: impl Into<String>) -> Self {
        Self::Test(name.into())
    }

    pub(crate) fn display_name(&self) -> String {
        match self {
            Self::UserFile(path) => path.display().to_string(),
            Self::EmbeddedDefault => "<embedded default config>".to_string(),
            #[cfg(test)]
            Self::Test(name) => name.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ConfigSource {
    inner: Arc<NamedSource<String>>,
}

impl ConfigSource {
    pub(crate) fn new(name: ConfigSourceName, text: impl Into<String>) -> Self {
        Self {
            inner: Arc::new(NamedSource::new(name.display_name(), text.into())),
        }
    }

    #[cfg(test)]
    pub(crate) fn name(&self) -> &str {
        self.inner.name()
    }
}

impl AsRef<dyn SourceCode + 'static> for ConfigSource {
    fn as_ref(&self) -> &(dyn SourceCode + 'static) {
        self.inner.as_ref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConfigDiagnosticKind {
    Syntax,
    Parse,
    Validate,
}

#[derive(Debug, Clone)]
pub(crate) struct ConfigDiagnostic {
    kind: ConfigDiagnosticKind,
    source: ConfigSource,
    path: Option<String>,
    message: String,
    label: Option<String>,
    help: Option<String>,
    span: Option<SourceSpan>,
}

impl ConfigDiagnostic {
    pub(crate) fn new(
        kind: ConfigDiagnosticKind,
        source: ConfigSource,
        path: Option<String>,
        message: impl Into<String>,
        span: Option<SourceSpan>,
    ) -> Self {
        Self {
            kind,
            source,
            path,
            message: message.into(),
            label: None,
            help: None,
            span,
        }
    }

    pub(crate) fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub(crate) fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    #[cfg(test)]
    pub(crate) fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    #[cfg(test)]
    pub(crate) fn help_text(&self) -> Option<&str> {
        self.help.as_deref()
    }

    #[cfg(test)]
    pub(crate) fn has_span(&self) -> bool {
        self.span.is_some()
    }
}

impl Display for ConfigDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(path) = &self.path {
            write!(f, "{path}: {}", self.message)
        } else {
            write!(f, "{}", self.message)
        }
    }
}

impl std::error::Error for ConfigDiagnostic {}

impl Diagnostic for ConfigDiagnostic {
    fn source_code(&self) -> Option<&dyn SourceCode> {
        Some(self.source.as_ref())
    }

    fn severity(&self) -> Option<Severity> {
        let _ = self.kind;
        Some(Severity::Error)
    }

    fn help<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        self.help
            .as_ref()
            .map(|help| Box::new(help) as Box<dyn Display>)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        self.span.map(|span| {
            let label = self.label.clone().unwrap_or_else(|| "here".to_string());
            Box::new(std::iter::once(LabeledSpan::new_with_span(
                Some(label),
                span,
            ))) as Box<dyn Iterator<Item = LabeledSpan>>
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ConfigError {
    #[error("{0}")]
    Intake(canopa_config::Diagnostic),
    #[error("{0}")]
    Diagnostic(Box<ConfigDiagnostic>),
}

impl From<ConfigDiagnostic> for ConfigError {
    fn from(value: ConfigDiagnostic) -> Self {
        Self::Diagnostic(Box::new(value))
    }
}

impl ConfigError {
    /// Service logs never include arbitrary names, values, or source snippets.
    pub(crate) fn safe_diagnostic(&self) -> canopa_config::Diagnostic {
        match self {
            Self::Intake(error) => error.clone(),
            Self::Diagnostic(error) => canopa_config::Diagnostic::new(
                match error.kind {
                    ConfigDiagnosticKind::Syntax => canopa_config::Code::Syntax,
                    _ => canopa_config::Code::Schema,
                },
                error.span.map(|span| span.offset()),
            ),
        }
    }

    pub(crate) fn from_kdl(source: ConfigSource, error: kdl::KdlError) -> Self {
        let diagnostic = error.diagnostics.first().map_or_else(
            || {
                ConfigDiagnostic::new(
                    ConfigDiagnosticKind::Syntax,
                    source.clone(),
                    None,
                    "KDL syntax error",
                    None,
                )
            },
            |diag| {
                let message = diag
                    .message
                    .clone()
                    .unwrap_or_else(|| "KDL syntax error".to_string());
                let mut out = ConfigDiagnostic::new(
                    ConfigDiagnosticKind::Syntax,
                    source.clone(),
                    None,
                    format!("KDL syntax error: {message}"),
                    Some(diag.span),
                );
                if let Some(label) = &diag.label {
                    out = out.with_label(label.clone());
                }
                if let Some(help) = &diag.help {
                    out = out.with_help(help.clone());
                }
                out
            },
        );
        diagnostic.into()
    }

    pub(crate) fn validation_message(source: ConfigSource, message: impl Into<String>) -> Self {
        ConfigDiagnostic::new(
            ConfigDiagnosticKind::Validate,
            source,
            None,
            message.into(),
            None,
        )
        .into()
    }

    pub(crate) fn as_diagnostic(&self) -> Option<&ConfigDiagnostic> {
        match self {
            Self::Diagnostic(diagnostic) => Some(diagnostic.as_ref()),
            Self::Intake(_) => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn source_name(&self) -> Option<&str> {
        self.as_diagnostic()
            .map(|diagnostic| diagnostic.source.name())
    }
}

impl Diagnostic for ConfigError {
    fn source_code(&self) -> Option<&dyn SourceCode> {
        self.as_diagnostic()
            .and_then(|diagnostic| diagnostic.source_code())
    }

    fn help<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        self.as_diagnostic()
            .and_then(|diagnostic| diagnostic.help())
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        self.as_diagnostic()
            .and_then(|diagnostic| diagnostic.labels())
    }
}
