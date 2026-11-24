//! Filename/MIME hints from shared-mime-info, without a GLib/GTK dependency.
use canopa_file::picker::Filter;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

struct MimeGlob {
    mime: String,
    pattern: glob::Pattern,
    case_sensitive: bool,
}
#[derive(Default)]
pub struct MimeGlobs(Vec<MimeGlob>);

impl MimeGlobs {
    pub fn load() -> Self {
        let mut roots = Vec::new();
        if let Some(root) = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .filter(|p| p.is_absolute())
                    .map(|p| p.join(".local/share"))
            })
        {
            roots.push(root);
        }
        roots.extend(
            std::env::split_paths(
                &std::env::var_os("XDG_DATA_DIRS")
                    .unwrap_or_else(|| "/usr/local/share:/usr/share".into()),
            )
            .filter(|p| p.is_absolute()),
        );
        let mut seen = BTreeSet::new();
        let mut globs = Vec::new();
        for root in roots {
            if !seen.insert(root.clone()) {
                continue;
            }
            if let Ok(source) = std::fs::read_to_string(root.join("mime/globs2")) {
                for line in source.lines().filter(|line| !line.starts_with('#')) {
                    let parts: Vec<_> = line.splitn(4, ':').collect();
                    if parts.len() >= 3
                        && let Ok(pattern) = glob::Pattern::new(parts[2])
                    {
                        globs.push(MimeGlob {
                            mime: parts[1].into(),
                            pattern,
                            case_sensitive: parts
                                .get(3)
                                .is_some_and(|flags| flags.split(',').any(|f| f == "cs")),
                        });
                    }
                }
            }
        }
        Self(globs)
    }

    pub fn matches(&self, filter: &Filter, path: &Path) -> bool {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        filter.1.iter().any(|(kind, pattern)| match kind {
            0 => glob::Pattern::new(pattern).is_ok_and(|p| p.matches(&name)),
            1 => self.0.iter().any(|entry| {
                (entry.mime == *pattern
                    || pattern
                        .strip_suffix("/*")
                        .is_some_and(|prefix| entry.mime.starts_with(&format!("{prefix}/"))))
                    && entry.pattern.matches_with(
                        &name,
                        glob::MatchOptions {
                            case_sensitive: entry.case_sensitive,
                            ..Default::default()
                        },
                    )
            }),
            _ => false,
        })
    }
}
