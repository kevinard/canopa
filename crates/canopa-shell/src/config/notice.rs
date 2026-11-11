//! What the user sees when their configuration is rejected.
//!
//! Logs and IPC replies keep the safe diagnostic codes (ADR 0021). This notice is
//! shown only to the person whose file it is, through Notify and the bar, so it
//! names the file, the line and the parser's message.
use std::path::{Path, PathBuf};

use super::ConfigError;
use crate::notice::{Notice, NoticeAction};

const MESSAGE_LIMIT: usize = 400;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConfigNotice {
    pub(crate) path: Option<PathBuf>,
    /// One-based line and column of the reported position.
    pub(crate) position: Option<(usize, usize)>,
    pub(crate) message: String,
    /// Shell-namespace problems open Settings at the Shell's Widgets page;
    /// others at Personalization.
    pub(crate) shell_namespace: bool,
}

/// Which configuration the running Shell uses while the notice is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fallback {
    EmbeddedDefaults,
    PreviousConfiguration,
}

impl ConfigNotice {
    pub(crate) fn from_error(error: &ConfigError, request: &canopa_config::Request) -> Self {
        match error {
            ConfigError::Diagnostic(diagnostic) => {
                let source = diagnostic.source();
                let mut message = diagnostic.to_string();
                if let Some(help) = diagnostic.help_text() {
                    message = format!("{message} ({help})");
                }
                Self {
                    path: source.path().map(Path::to_path_buf),
                    position: diagnostic
                        .offset()
                        .map(|offset| line_column(source.text(), offset)),
                    message: crate::notice::sanitize(&message, MESSAGE_LIMIT),
                    shell_namespace: true,
                }
            }
            ConfigError::Intake(diagnostic) => {
                let path = match request.resolve(canopa_config::Component::Shell) {
                    Ok(canopa_config::Request::File { path, .. }) => Some(path),
                    _ => None,
                };
                // Theme-file offsets refer to the theme, not to config.kdl.
                let position = diagnostic
                    .offset
                    .filter(|_| diagnostic.source.is_none())
                    .zip(path.as_deref())
                    .and_then(|(offset, path)| {
                        canopa_config::read(path)
                            .ok()
                            .map(|text| line_column(&text, offset))
                    });
                let message = canopa_config::Diagnostic {
                    offset: None,
                    ..diagnostic.clone()
                }
                .to_string();
                Self {
                    path,
                    position,
                    message: crate::notice::sanitize(&message, MESSAGE_LIMIT),
                    shell_namespace: false,
                }
            }
        }
    }

    /// A construction failure after a complete validation. No position exists.
    pub(crate) fn construction(request: &canopa_config::Request) -> Self {
        let path = match request.resolve(canopa_config::Component::Shell) {
            Ok(canopa_config::Request::File { path, .. }) => Some(path),
            _ => None,
        };
        Self {
            path,
            position: None,
            message: "the Shell could not build the configured modules".into(),
            shell_namespace: true,
        }
    }

    /// `path:line:column`, as editors and terminals understand it.
    pub(crate) fn location(&self) -> String {
        let path = self.path.as_ref().map_or_else(
            || "configuration".to_string(),
            |path| path.display().to_string(),
        );
        match self.position {
            Some((line, column)) => format!("{path}:{line}:{column}"),
            None => path,
        }
    }

    pub(crate) fn actions(&self) -> Vec<NoticeAction> {
        // Shell problems open the Widgets page. Envelope, syntax and appearance
        // problems open Personalization, which edits the shared appearance
        // and reports why the file can't be read. A running Settings window
        // switches to the page.
        let settings = NoticeAction::OpenSettings(Some(if self.shell_namespace {
            "widgets"
        } else {
            "personal"
        }));
        match &self.path {
            Some(path) => vec![NoticeAction::OpenFile(path.clone()), settings],
            None => vec![settings],
        }
    }

    pub(crate) fn notice(&self, fallback: Fallback) -> Notice {
        let consequence = match fallback {
            Fallback::EmbeddedDefaults => fl!("shell-config-notice-defaults"),
            Fallback::PreviousConfiguration => fl!("shell-config-notice-previous"),
        };
        Notice {
            summary: fl!("shell-config-notice-summary"),
            body: format!("{}\n{}\n{consequence}", self.location(), self.message),
            persistent: true,
            actions: self.actions(),
        }
    }
}

/// One-based line and column (in characters) of a byte offset.
pub(crate) fn line_column(text: &str, offset: usize) -> (usize, usize) {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    let before = &text[..offset];
    let line_start = before.rfind('\n').map_or(0, |index| index + 1);
    (
        before.matches('\n').count() + 1,
        before[line_start..].chars().count() + 1,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, text: &str) -> PathBuf {
        let path = dir.join("config.kdl");
        std::fs::write(&path, text).unwrap();
        path
    }

    #[test]
    fn line_column_counts_lines_and_characters() {
        assert_eq!(line_column("abc", 0), (1, 1));
        assert_eq!(line_column("ab\ncdé\nf", 7), (2, 4));
        assert_eq!(line_column("ab\ncdé\nf", 8), (3, 1));
        assert_eq!(line_column("ab\ncdé\nf", 9), (3, 2));
        // Offsets inside a character or past the end stay in range.
        assert_eq!(line_column("é", 1), (1, 1));
        assert_eq!(line_column("ab", 99), (1, 3));
    }

    #[test]
    fn shell_schema_errors_name_the_file_line_and_parser_message() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(
            dir.path(),
            "schema-version 1\nshell {\n    bar \"main\" {\n        edge \"nowhere\"\n    }\n}\n",
        );
        let request = canopa_config::Request::common(&path);
        let error = crate::config::load(&request, true).unwrap_err();
        let notice = ConfigNotice::from_error(&error, &request);
        assert_eq!(notice.path.as_deref(), Some(path.as_path()));
        let (line, _) = notice.position.expect("schema errors carry a span");
        assert_eq!(line, 4);
        assert!(notice.message.contains("edge"), "{}", notice.message);
        assert!(
            notice
                .location()
                .starts_with(&format!("{}:4:", path.display()))
        );
        assert_eq!(
            notice.actions(),
            vec![
                NoticeAction::OpenFile(path.clone()),
                NoticeAction::OpenSettings(Some("widgets"))
            ]
        );
        let shown = notice.notice(Fallback::EmbeddedDefaults);
        assert!(shown.persistent);
        assert!(shown.body.contains("built-in layout"));
        assert!(
            notice
                .notice(Fallback::PreviousConfiguration)
                .body
                .contains("last working configuration")
        );
    }

    #[test]
    fn syntax_and_envelope_errors_point_into_the_shared_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(dir.path(), "schema-version 1\nshell {\n  bar \"x\" {\n");
        let request = canopa_config::Request::common(&path);
        let notice =
            ConfigNotice::from_error(&crate::config::load(&request, true).unwrap_err(), &request);
        assert_eq!(notice.path.as_deref(), Some(path.as_path()));
        assert!(notice.position.is_some());
        assert!(
            notice.message.contains("KDL syntax error"),
            "{}",
            notice.message
        );

        let path = write(dir.path(), "schema-version 2\n");
        let request = canopa_config::Request::common(&path);
        let notice =
            ConfigNotice::from_error(&crate::config::load(&request, true).unwrap_err(), &request);
        assert_eq!(notice.position, Some((1, 1)));
        assert!(notice.message.starts_with("expected schema-version 1"));
        assert!(!notice.message.contains("near byte"));
        assert!(!notice.shell_namespace);
        assert_eq!(
            notice.actions()[1],
            NoticeAction::OpenSettings(Some("personal"))
        );
    }
}
