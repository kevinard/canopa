use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiagnosticsConfig {
    pub(crate) perf_enabled: bool,
    pub(crate) filter_source: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FilterConfig {
    spec: String,
    source: &'static str,
}

impl FilterConfig {
    pub(crate) fn spec(&self) -> &str {
        &self.spec
    }

    pub(crate) fn source(&self) -> &'static str {
        self.source
    }
}

pub(crate) fn perf_enabled_from_value(value: Option<&str>) -> bool {
    matches!(value.map(str::trim), Some("1" | "true" | "yes" | "on"))
}

pub(crate) fn perf_enabled_from_env() -> bool {
    let value = std::env::var("NIRIBAR_PERF").ok();
    perf_enabled_from_value(value.as_deref())
}

pub(crate) fn default_filter_spec(perf_enabled: bool) -> &'static str {
    if perf_enabled {
        "niribar=warn,niribar::configuration=info,niribar::perf=info"
    } else {
        "niribar=warn,niribar::configuration=info"
    }
}

pub(crate) fn filter_config(rust_log: Option<&str>, perf_enabled: bool) -> FilterConfig {
    match rust_log.map(str::trim).filter(|value| !value.is_empty()) {
        Some(spec) => FilterConfig {
            spec: spec.to_string(),
            source: "RUST_LOG",
        },
        None => FilterConfig {
            spec: default_filter_spec(perf_enabled).to_string(),
            source: "default",
        },
    }
}

pub(crate) fn init() -> DiagnosticsConfig {
    let perf_enabled = perf_enabled_from_env();
    let rust_log = std::env::var("RUST_LOG").ok();
    let filter_config = filter_config(rust_log.as_deref(), perf_enabled);
    let filter = EnvFilter::try_new(filter_config.spec())
        .unwrap_or_else(|_| EnvFilter::new(default_filter_spec(perf_enabled)));

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();

    DiagnosticsConfig {
        perf_enabled,
        filter_source: filter_config.source(),
    }
}

pub(crate) fn log_startup(mode: &'static str, config: &DiagnosticsConfig) {
    tracing::debug!(
        target: "niribar::diagnostics",
        version = env!("CARGO_PKG_VERSION"),
        mode,
        perf_enabled = config.perf_enabled,
        filter_source = config.filter_source,
        "niribar diagnostics initialized"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perf_env_accepts_existing_truthy_values() {
        for value in ["1", "true", "yes", "on"] {
            assert!(
                perf_enabled_from_value(Some(value)),
                "{value} should enable perf"
            );
        }
    }

    #[test]
    fn perf_env_rejects_missing_or_other_values() {
        for value in [None, Some(""), Some("0"), Some("false"), Some("TRUE")] {
            assert!(!perf_enabled_from_value(value));
        }
    }

    #[test]
    fn default_filter_reports_installed_configuration_without_perf() {
        let config = filter_config(None, false);

        assert_eq!(config.spec(), "niribar=warn,niribar::configuration=info");
        assert_eq!(config.source(), "default");
    }

    #[test]
    fn default_filter_enables_perf_info_when_perf_is_enabled() {
        let config = filter_config(None, true);

        assert_eq!(
            config.spec(),
            "niribar=warn,niribar::configuration=info,niribar::perf=info"
        );
        assert_eq!(config.source(), "default");
    }

    #[test]
    fn rust_log_overrides_default_filter() {
        let config = filter_config(Some("niribar=debug"), true);

        assert_eq!(config.spec(), "niribar=debug");
        assert_eq!(config.source(), "RUST_LOG");
    }

    #[test]
    fn empty_rust_log_uses_default_filter() {
        let config = filter_config(Some("   "), true);

        assert_eq!(
            config.spec(),
            "niribar=warn,niribar::configuration=info,niribar::perf=info"
        );
        assert_eq!(config.source(), "default");
    }
}
