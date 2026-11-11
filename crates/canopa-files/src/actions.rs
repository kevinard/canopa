//! Explicit custom actions with argument-safe selection expansion.
pub use crate::process::CommandOutcome as ActionOutcome;
use crate::{
    Cancellation,
    browse::{Entry, EntryKind},
};
use serde::{Deserialize, Serialize};
use std::{
    ffi::{OsStr, OsString},
    io,
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomAction {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub icon: Option<String>,
    pub command: String,
    #[serde(default)]
    pub conditions: ActionConditions,
    #[serde(default)]
    pub shortcut: Option<String>,
    #[serde(default)]
    pub toolbar: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ActionConditions {
    pub minimum_selection: usize,
    pub maximum_selection: Option<usize>,
    pub files: bool,
    pub directories: bool,
    pub patterns: Vec<String>,
}

impl Default for ActionConditions {
    fn default() -> Self {
        Self {
            minimum_selection: 1,
            maximum_selection: None,
            files: true,
            directories: false,
            patterns: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PreparedAction {
    pub program: OsString,
    pub arguments: Vec<OsString>,
    pub working_directory: PathBuf,
}

impl CustomAction {
    pub fn new(id: impl Into<String>, name: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            icon: None,
            command: command.into(),
            conditions: ActionConditions::default(),
            shortcut: None,
            toolbar: false,
        }
    }

    pub fn validate(&self) -> io::Result<()> {
        let invalid = || io::Error::new(io::ErrorKind::InvalidInput, fl!("files-action-invalid"));
        if self.id.is_empty()
            || self.id.len() > 96
            || !self
                .id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
            || self.name.trim().is_empty()
            || self.name.len() > 256
            || self.name.contains('\0')
            || self.command.len() > 16 * 1024
            || self.command.contains('\0')
            || self
                .conditions
                .maximum_selection
                .is_some_and(|max| max < self.conditions.minimum_selection)
            || self.conditions.patterns.len() > 64
            || self
                .conditions
                .patterns
                .iter()
                .any(|pattern| pattern.is_empty() || pattern.len() > 256)
            || self
                .icon
                .as_ref()
                .is_some_and(|icon| icon.len() > 256 || icon.contains('\0'))
            || self
                .shortcut
                .as_ref()
                .is_some_and(|shortcut| shortcut.len() > 64 || shortcut.contains('\0'))
        {
            return Err(invalid());
        }
        let tokens = shlex::split(&self.command)
            .filter(|tokens| !tokens.is_empty())
            .ok_or_else(invalid)?;
        for (index, token) in tokens.iter().enumerate() {
            let mut bytes = token.bytes();
            while let Some(byte) = bytes.next() {
                if byte != b'%' {
                    continue;
                }
                match bytes.next() {
                    Some(b'%') => {}
                    Some(b'f' | b'n' | b'd') if index > 0 => {}
                    Some(b'F' | b'N') if index > 0 && (token == "%F" || token == "%N") => {}
                    _ => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "invalid custom-action placeholder",
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn is_applicable(&self, entries: &[Entry]) -> bool {
        self.applies_to(entries.iter())
    }

    pub(crate) fn applies_to<'a>(
        &self,
        mut entries: impl ExactSizeIterator<Item = &'a Entry>,
    ) -> bool {
        let c = &self.conditions;
        entries.len() >= c.minimum_selection
            && c.maximum_selection
                .is_none_or(|maximum| entries.len() <= maximum)
            && entries.all(|entry| {
                (if entry.is_directory() {
                    c.directories
                } else {
                    c.files && entry.kind != EntryKind::Other
                }) && (c.patterns.is_empty()
                    || c.patterns
                        .iter()
                        .any(|pattern| matches_pattern(pattern, &entry.name())))
            })
    }

    pub fn prepare(
        &self,
        selected: &[PathBuf],
        working_directory: &Path,
    ) -> io::Result<PreparedAction> {
        self.validate()?;
        let working_directory = std::path::absolute(working_directory)?;
        if !working_directory.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                "custom-action location is unavailable",
            ));
        }
        let selected: Vec<_> = selected
            .iter()
            .map(|path| {
                if path.is_absolute() {
                    path.clone()
                } else {
                    working_directory.join(path)
                }
            })
            .collect();
        let entries = selected
            .iter()
            .map(Entry::read)
            .collect::<io::Result<Vec<_>>>()?;
        if !self.is_applicable(&entries) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "custom action does not apply to this selection",
            ));
        }
        let tokens = shlex::split(&self.command).expect("validated command");
        let mut arguments = Vec::new();
        for token in tokens.iter().skip(1) {
            arguments.extend(expand(token, &selected, &working_directory)?);
        }
        let program = expand(&tokens[0], &[], &working_directory)?.remove(0);
        Ok(PreparedAction {
            program,
            arguments,
            working_directory,
        })
    }
}

impl PreparedAction {
    pub fn run(&self, cancellation: &Cancellation) -> io::Result<ActionOutcome> {
        let mut command = std::process::Command::new(&self.program);
        command
            .args(&self.arguments)
            .current_dir(&self.working_directory);
        crate::process::run(command, cancellation)
    }
}

fn expand(token: &str, selected: &[PathBuf], directory: &Path) -> io::Result<Vec<OsString>> {
    if token == "%F" {
        return Ok(selected
            .iter()
            .map(|path| path.as_os_str().to_owned())
            .collect());
    }
    if token == "%N" {
        return Ok(selected
            .iter()
            .map(|path| path.file_name().unwrap_or(path.as_os_str()).to_owned())
            .collect());
    }
    let mut result = Vec::new();
    let mut bytes = token.bytes();
    while let Some(byte) = bytes.next() {
        if byte != b'%' {
            result.push(byte);
            continue;
        }
        let value: &OsStr = match bytes.next() {
            Some(b'%') => {
                result.push(b'%');
                continue;
            }
            Some(b'd') => directory.as_os_str(),
            Some(b'f') => selected
                .first()
                .map(|path| path.as_os_str())
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, fl!("files-action-needs-file"))
                })?,
            Some(b'n') => selected
                .first()
                .map(|path| path.file_name().unwrap_or(path.as_os_str()))
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, fl!("files-action-needs-name"))
                })?,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "invalid placeholder",
                ));
            }
        };
        result.extend_from_slice(value.as_bytes());
    }
    Ok(vec![OsString::from_vec(result)])
}

fn matches_pattern(pattern: &str, text: &str) -> bool {
    let pattern: Vec<_> = pattern.to_lowercase().chars().collect();
    let text: Vec<_> = text.to_lowercase().chars().collect();
    let (mut p, mut t, mut star, mut checkpoint) = (0, 0, None, 0);
    while t < text.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == text[t]) {
            p += 1;
            t += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some(p);
            p += 1;
            checkpoint = t;
        } else if let Some(previous) = star {
            checkpoint += 1;
            t = checkpoint;
            p = previous + 1;
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == '*' {
        p += 1;
    }
    p == pattern.len()
}
