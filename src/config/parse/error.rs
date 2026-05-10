//! Shared error-path construction for config parsing.
//!
//! Most parsing errors in `config/parse.rs` start with
//! `modules[{idx}].{module}.{field}`. Funneling that prefix through one helper
//! lets us change the wording in a single place and keeps error paths uniform.

use ::kdl::{KdlDocument, KdlEntry, KdlNode};
use miette::SourceSpan;

use crate::config::{
    ConfigDiagnostic, ConfigDiagnosticKind, ConfigError, ConfigResult, ConfigSource,
};

#[derive(Debug, Clone)]
pub(super) struct ParseCtx {
    source: ConfigSource,
    path: String,
}

impl ParseCtx {
    pub(super) fn root(source: ConfigSource) -> Self {
        Self {
            source,
            path: String::new(),
        }
    }

    pub(super) fn child(&self, name: impl AsRef<str>) -> Self {
        let name = name.as_ref();
        let path = if self.path.is_empty() {
            name.to_string()
        } else {
            format!("{}.{}", self.path, name)
        };
        Self {
            source: self.source.clone(),
            path,
        }
    }

    pub(super) fn index(&self, idx: usize) -> Self {
        Self {
            source: self.source.clone(),
            path: format!("{}[{idx}]", self.path),
        }
    }

    pub(super) fn path(&self) -> &str {
        &self.path
    }

    pub(super) fn err(
        &self,
        span: Option<SourceSpan>,
        message: impl Into<String>,
        help: Option<String>,
    ) -> ConfigError {
        let mut diagnostic = ConfigDiagnostic::new(
            ConfigDiagnosticKind::Parse,
            self.source.clone(),
            (!self.path.is_empty()).then(|| self.path.clone()),
            message,
            span,
        );
        if let Some(help) = help {
            diagnostic = diagnostic.with_help(help);
        }
        diagnostic.into()
    }

    pub(super) fn node_err(
        &self,
        node: &KdlNode,
        message: impl Into<String>,
        help: Option<String>,
    ) -> ConfigError {
        self.err(Some(node.span()), message, help)
    }

    pub(super) fn entry_err(
        &self,
        entry: &KdlEntry,
        message: impl Into<String>,
        help: Option<String>,
    ) -> ConfigError {
        self.err(Some(entry.span()), message, help)
    }

    pub(super) fn validate_child_schema(
        &self,
        children: &KdlDocument,
        allowed: &[&str],
        unique: &[&str],
    ) -> ConfigResult<()> {
        for node in children.nodes() {
            let name = node.name().value();
            if !allowed.contains(&name) {
                let field_ctx = self.child(name);
                let help = suggestion(name, allowed)
                    .map(|suggestion| format!("did you mean `{suggestion}`?"))
                    .or_else(|| Some(format!("expected one of: {}", allowed.join(", "))));
                return Err(field_ctx.node_err(node, format!("unknown field `{name}`"), help));
            }
        }

        for field in unique {
            let mut first: Option<&KdlNode> = None;
            for node in children
                .nodes()
                .iter()
                .filter(|node| node.name().value() == *field)
            {
                if first.replace(node).is_some() {
                    return Err(self.child(field).node_err(
                        node,
                        format!("duplicate field `{field}`"),
                        Some(format!("remove one `{field}` entry")),
                    ));
                }
            }
        }

        Ok(())
    }
}

fn suggestion<'a>(input: &str, allowed: &'a [&str]) -> Option<&'a str> {
    allowed
        .iter()
        .copied()
        .map(|candidate| (candidate, edit_distance(input, candidate)))
        .filter(|(_, distance)| *distance <= 3)
        .min_by_key(|(_, distance)| *distance)
        .map(|(candidate, _)| candidate)
}

fn edit_distance(a: &str, b: &str) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0; b.len() + 1];
    for (i, ca) in a.chars().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.chars().enumerate() {
            let cost = usize::from(ca != cb);
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}
