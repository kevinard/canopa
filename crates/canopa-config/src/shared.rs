use super::*;

/// Shared settings without selecting or interpreting an application's namespace.
#[derive(Clone, Debug)]
pub struct SharedConfiguration {
    pub appearance: canopa_theme::Appearance,
    pub source: SourceInfo,
}

impl SharedConfiguration {
    /// Discovery uses only the Canopa file. Explicit legacy input is rejected.
    /// The caller owns when to reload and whether to install a valid candidate.
    pub fn load(request: &Request) -> Result<Self, Diagnostic> {
        let resolved = match request {
            Request::Discover => discover_common_in(&config_home()?)?,
            Request::File {
                format: Format::Legacy,
                ..
            } => {
                return Err(Diagnostic::new(Code::Envelope, None));
            }
            other => other.clone(),
        };
        match resolved {
            Request::Defaults => Ok(Self {
                appearance: Default::default(),
                source: SourceInfo {
                    path: None,
                    format: None,
                    fingerprint: fingerprint(""),
                    defaults: true,
                    dependencies: Vec::new(),
                    combined_fingerprint: None,
                },
            }),
            Request::File { path, format } => {
                let path =
                    std::path::absolute(path).map_err(|_| Diagnostic::new(Code::Location, None))?;
                let text = read(&path)?;
                let raw = parse_kdl(&text)?;
                validate_envelope(&raw)?;
                let document = Document::parse(text, Some(path), format, Component::Shell, "")?;
                let mut source = document.info().clone();
                source.defaults = raw.get("appearance").is_none();
                Ok(Self {
                    appearance: document.appearance(),
                    source,
                })
            }
            Request::Discover => unreachable!(),
        }
    }
}
