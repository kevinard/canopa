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
                },
            }),
            Request::File { path, format } => {
                let path =
                    std::path::absolute(path).map_err(|_| Diagnostic::new(Code::Location, None))?;
                let text = read(&path)?;
                let document = parse_kdl(&text)?;
                validate_envelope(&document)?;
                let appearance = appearance::parse(&document)?;
                Ok(Self {
                    appearance,
                    source: SourceInfo {
                        path: Some(path),
                        format: Some(format),
                        fingerprint: fingerprint(&text),
                        defaults: document.get("appearance").is_none(),
                    },
                })
            }
            Request::Discover => unreachable!(),
        }
    }
}
