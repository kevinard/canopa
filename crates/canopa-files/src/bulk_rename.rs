//! Bulk Rename rules and their preview, independent of the toolkit. A rule
//! changes the part of each name before its extension; folders change whole.
//! Names keep their original Unix bytes; only labels are lossy.
use crate::operations::rename_batch::BatchRename;
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::{OsStr, OsString},
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::PathBuf,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rule {
    /// Every occurrence of `find`, which must not be empty, becomes `with`.
    Replace { find: String, with: String },
    /// Text added before and after each name.
    Add { prefix: String, suffix: String },
    /// `name` followed by a number counting from `start`, zero-padded to the
    /// width of the largest one; the number alone when `name` is empty.
    Number { name: String, start: u64 },
}

/// One selected item, in the order shown, which Number counts in.
#[derive(Clone, Debug)]
pub struct Item {
    pub path: PathBuf,
    pub directory: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    /// Empty, `.`, `..`, or containing `/` or NUL.
    Invalid,
    /// Two selected items would get this name.
    Duplicate,
    /// Another item of the folder, not renamed, already has this name.
    Exists,
    /// The new names swap among selected items.
    Swap,
}

#[derive(Clone, Debug)]
pub struct Row {
    pub source: PathBuf,
    pub name: OsString,
    pub problem: Option<Problem>,
}

impl Row {
    pub fn changed(&self) -> bool {
        self.source.file_name() != Some(self.name.as_os_str())
    }
}

#[derive(Clone, Debug, Default)]
pub struct Plan {
    pub rows: Vec<Row>,
}

impl Plan {
    pub fn changes(&self) -> usize {
        self.rows.iter().filter(|row| row.changed()).count()
    }

    pub fn problems(&self) -> usize {
        self.rows.iter().filter(|row| row.problem.is_some()).count()
    }

    /// The steps to submit, when something changes and nothing is wrong.
    pub fn renames(&self) -> Option<Vec<BatchRename>> {
        if self.problems() > 0 || self.changes() == 0 {
            return None;
        }
        Some(
            self.rows
                .iter()
                .filter(|row| row.changed())
                .map(|row| BatchRename {
                    source: row.source.clone(),
                    destination: row.source.with_file_name(&row.name),
                })
                .collect(),
        )
    }
}

/// Apply `rule` to `items`, all in one folder whose other names are `existing`
/// (it may include the items' own names), and find each new name's problem.
pub fn plan(rule: &Rule, items: &[Item], existing: &BTreeSet<OsString>) -> Plan {
    let width = match rule {
        Rule::Number { start, .. } => {
            digits(start.saturating_add((items.len() as u64).saturating_sub(1)))
        }
        _ => 0,
    };
    let mut rows: Vec<Row> = items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let original = item.path.file_name().unwrap_or_default();
            let (stem, extension) = split(original.as_bytes(), item.directory);
            let stem = match rule {
                Rule::Replace { find, with } if !find.is_empty() => {
                    replace(stem, find.as_bytes(), with.as_bytes())
                }
                Rule::Replace { .. } => stem.to_vec(),
                Rule::Add { prefix, suffix } => {
                    [prefix.as_bytes(), stem, suffix.as_bytes()].concat()
                }
                Rule::Number { name, start } => {
                    let number = format!(
                        "{:0width$}",
                        start.saturating_add(index as u64),
                        width = width
                    );
                    if name.is_empty() {
                        number.into_bytes()
                    } else {
                        format!("{name} {number}").into_bytes()
                    }
                }
            };
            Row {
                source: item.path.clone(),
                name: OsString::from_vec([stem.as_slice(), extension].concat()),
                problem: None,
            }
        })
        .collect();
    let renamed: BTreeSet<&OsStr> = rows
        .iter()
        .filter(|row| row.changed())
        .filter_map(|row| row.source.file_name())
        .collect();
    let mut counts: BTreeMap<OsString, usize> = BTreeMap::new();
    for row in &rows {
        *counts.entry(row.name.clone()).or_default() += 1;
    }
    let problems: Vec<_> = rows
        .iter()
        .map(|row| {
            if !row.changed() {
                return None;
            }
            if !valid(&row.name) {
                Some(Problem::Invalid)
            } else if counts[&row.name] > 1 {
                Some(Problem::Duplicate)
            } else if existing.contains(&row.name) && !renamed.contains(row.name.as_os_str()) {
                Some(Problem::Exists)
            } else {
                None
            }
        })
        .collect();
    for (row, problem) in rows.iter_mut().zip(problems) {
        row.problem = problem;
    }
    mark_swaps(&mut rows);
    Plan { rows }
}

/// A changed row whose new name is another changed row's old name must wait
/// for it; following those links back to the start is a swap.
fn mark_swaps(rows: &mut [Row]) {
    let sources: BTreeMap<&OsStr, usize> = rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row.changed())
        .filter_map(|(index, row)| Some((row.source.file_name()?, index)))
        .collect();
    let next = |index: usize| sources.get(rows[index].name.as_os_str()).copied();
    let mut swapped = Vec::new();
    for start in sources.values().copied() {
        let mut current = start;
        for _ in 0..rows.len() {
            match next(current) {
                Some(following) if following == start => {
                    swapped.push(start);
                    break;
                }
                Some(following) => current = following,
                None => break,
            }
        }
    }
    for index in swapped {
        rows[index].problem.get_or_insert(Problem::Swap);
    }
}

fn valid(name: &OsStr) -> bool {
    let bytes = name.as_bytes();
    !bytes.is_empty()
        && bytes != b"."
        && bytes != b".."
        && !bytes.contains(&b'/')
        && !bytes.contains(&0)
}

/// The name before its extension, and the extension with its dot. A
/// compressed tarball's two suffixes count as one; folders have none.
fn split(name: &[u8], directory: bool) -> (&[u8], &[u8]) {
    const COMPOUND: [&[u8]; 6] = [
        b".tar.gz",
        b".tar.bz2",
        b".tar.xz",
        b".tar.zst",
        b".tar.lz",
        b".tar.lzma",
    ];
    if directory {
        return (name, &[]);
    }
    let compound = COMPOUND.iter().find_map(|suffix| {
        let start = name
            .len()
            .checked_sub(suffix.len())
            .filter(|start| *start > 0)?;
        name[start..].eq_ignore_ascii_case(suffix).then_some(start)
    });
    let dot = compound.or_else(|| {
        name.iter()
            .rposition(|byte| *byte == b'.')
            .filter(|dot| *dot > 0)
    });
    match dot {
        Some(dot) => name.split_at(dot),
        None => (name, &[]),
    }
}

fn replace(haystack: &[u8], find: &[u8], with: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(haystack.len());
    let mut rest = haystack;
    while let Some(position) = rest.windows(find.len()).position(|window| window == find) {
        result.extend_from_slice(&rest[..position]);
        result.extend_from_slice(with);
        rest = &rest[position + find.len()..];
    }
    result.extend_from_slice(rest);
    result
}

fn digits(value: u64) -> usize {
    value.checked_ilog10().map_or(1, |log| log as usize + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(names: &[&str]) -> Vec<Item> {
        names
            .iter()
            .map(|name| Item {
                path: PathBuf::from("/folder").join(name.trim_end_matches('/')),
                directory: name.ends_with('/'),
            })
            .collect()
    }

    fn names(plan: &Plan) -> Vec<String> {
        plan.rows
            .iter()
            .map(|row| row.name.to_string_lossy().into_owned())
            .collect()
    }

    fn existing(names: &[&str]) -> BTreeSet<OsString> {
        names.iter().map(OsString::from).collect()
    }

    #[test]
    fn rules_change_the_name_before_its_extension() {
        let selected = items(&["IMG_1.jpg", "IMG_2.JPG", "backup.tar.gz", "IMG folder/"]);
        let replace = Rule::Replace {
            find: "IMG".into(),
            with: "Trip".into(),
        };
        assert_eq!(
            names(&plan(&replace, &selected, &BTreeSet::new())),
            ["Trip_1.jpg", "Trip_2.JPG", "backup.tar.gz", "Trip folder"]
        );
        let add = Rule::Add {
            prefix: "2026 ".into(),
            suffix: " (old)".into(),
        };
        assert_eq!(
            names(&plan(&add, &selected, &BTreeSet::new())),
            [
                "2026 IMG_1 (old).jpg",
                "2026 IMG_2 (old).JPG",
                "2026 backup (old).tar.gz",
                "2026 IMG folder (old)"
            ]
        );
        let many: Vec<_> = (0..12)
            .map(|n| Item {
                path: PathBuf::from(format!("/folder/{n}.png")),
                directory: false,
            })
            .collect();
        let number = Rule::Number {
            name: "Photo".into(),
            start: 1,
        };
        let numbered = names(&plan(&number, &many, &BTreeSet::new()));
        assert_eq!(numbered[0], "Photo 01.png");
        assert_eq!(numbered[11], "Photo 12.png");
        let bare = Rule::Number {
            name: String::new(),
            start: 7,
        };
        assert_eq!(
            names(&plan(&bare, &items(&[".bashrc"]), &BTreeSet::new())),
            ["7"]
        );
    }

    #[test]
    fn nothing_selected_plans_nothing() {
        let rule = Rule::Number {
            name: "x".into(),
            start: u64::MAX,
        };
        assert!(plan(&rule, &[], &BTreeSet::new()).renames().is_none());
    }

    #[test]
    fn non_utf8_names_keep_their_bytes() {
        let raw = PathBuf::from(OsString::from_vec(b"/folder/caf\xe9.txt".to_vec()));
        let selected = [Item {
            path: raw,
            directory: false,
        }];
        let rule = Rule::Add {
            prefix: "new ".into(),
            suffix: String::new(),
        };
        let plan = plan(&rule, &selected, &BTreeSet::new());
        assert_eq!(plan.rows[0].name.as_bytes(), b"new caf\xe9.txt");
    }

    #[test]
    fn problems_block_submission() {
        let selected = items(&["a.txt", "b.txt", "c.txt"]);
        let same = Rule::Number {
            name: "x".into(),
            start: 1,
        };
        assert!(
            plan(&same, &selected, &existing(&["x 2.txt"])).rows[1].problem
                == Some(Problem::Exists)
        );
        let collide = Rule::Replace {
            find: "a".into(),
            with: "b".into(),
        };
        // b.txt is selected and keeps its name, so a.txt cannot take it.
        let collided = plan(&collide, &selected, &existing(&["a.txt", "b.txt", "c.txt"]));
        assert_eq!(collided.rows[0].problem, Some(Problem::Duplicate));
        assert!(collided.renames().is_none());
        let slash = Rule::Add {
            prefix: "a/".into(),
            suffix: String::new(),
        };
        assert_eq!(
            plan(&slash, &selected, &BTreeSet::new()).rows[0].problem,
            Some(Problem::Invalid)
        );
        let empty = Rule::Replace {
            find: "x".into(),
            with: String::new(),
        };
        assert!(
            plan(&empty, &selected, &BTreeSet::new())
                .renames()
                .is_none()
        );
        let duplicate = Rule::Replace {
            find: "b".into(),
            with: "c".into(),
        };
        let duplicated = plan(&duplicate, &items(&["b.txt", "c.txt"]), &BTreeSet::new());
        assert_eq!(duplicated.rows[0].problem, Some(Problem::Duplicate));
    }

    #[test]
    fn chains_are_allowed_and_swaps_are_not() {
        let shift = Rule::Number {
            name: String::new(),
            start: 2,
        };
        let shifted = plan(
            &shift,
            &items(&["1", "2", "3"]),
            &existing(&["1", "2", "3"]),
        );
        assert_eq!(shifted.problems(), 0, "{:?}", shifted.rows);
        assert_eq!(shifted.renames().unwrap().len(), 3);
        let swap = Rule::Number {
            name: String::new(),
            start: 1,
        };
        let swapped = plan(&swap, &items(&["2", "1"]), &existing(&["1", "2"]));
        assert!(
            swapped
                .rows
                .iter()
                .all(|row| row.problem == Some(Problem::Swap))
        );
    }
}
