//! Local emoji and symbol results for the Applications Panel Menu.
use super::{Action, Batch, NamedAction, RESULT_LIMIT, ResultItem};
use crate::app::panel_search::query_tokens;
use std::sync::OnceLock;

pub(crate) const PROVIDER: usize = usize::MAX - 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Kind {
    Emoji,
    Symbol,
}

impl Kind {
    fn entries(self) -> &'static [Entry] {
        static EMOJI: OnceLock<Vec<Entry>> = OnceLock::new();
        static SYMBOLS: OnceLock<Vec<Entry>> = OnceLock::new();
        match self {
            Self::Emoji => {
                EMOJI.get_or_init(|| parse(include_str!("../../data/characters/emoji.tsv")))
            }
            Self::Symbol => {
                SYMBOLS.get_or_init(|| parse(include_str!("../../data/characters/symbols.tsv")))
            }
        }
    }
}

struct Entry {
    glyph: &'static str,
    name: &'static str,
    folded_name: String,
    keywords: String,
}

fn parse(data: &'static str) -> Vec<Entry> {
    data.lines()
        .map(|line| {
            let (glyph, details) = line.split_once('\t').expect("character catalog row");
            let (name, keywords) = details.split_once('\t').unwrap_or((details, ""));
            Entry {
                glyph,
                name,
                folded_name: name.to_ascii_lowercase(),
                keywords: keywords.to_ascii_lowercase(),
            }
        })
        .collect()
}

impl Entry {
    fn rank(&self, query: &str, words: &[String]) -> Option<u8> {
        if self
            .glyph
            .chars()
            .filter(|c| *c != '\u{fe0f}')
            .eq(query.chars().filter(|c| *c != '\u{fe0f}'))
        {
            Some(0)
        } else if self.folded_name == query {
            Some(1)
        } else if self.folded_name.starts_with(query) {
            Some(2)
        } else if words.iter().all(|word| self.folded_name.contains(word)) {
            Some(3)
        } else if words
            .iter()
            .all(|word| self.folded_name.contains(word) || self.keywords.contains(word))
        {
            Some(4)
        } else {
            None
        }
    }

    fn result(&self) -> ResultItem {
        ResultItem {
            id: self.glyph.into(),
            label: format!("{}  {}", self.glyph, self.name),
            description: String::new(),
            icon: None,
            actions: vec![NamedAction {
                label: "Copy".into(),
                action: Action::Copy {
                    text: self.glyph.into(),
                },
            }],
        }
    }
}

pub(crate) fn search(kind: Kind, query: &str) -> Batch {
    let entries = kind.entries();
    let words = query_tokens(query);
    let results: Vec<_> = if words.is_empty() {
        entries
            .iter()
            .take(RESULT_LIMIT)
            .map(Entry::result)
            .collect()
    } else {
        let query = words.join(" ");
        let mut matches: Vec<_> = entries
            .iter()
            .enumerate()
            .filter_map(|(position, entry)| {
                let rank = entry.rank(&query, &words)?;
                Some((rank, entry.name.len(), position))
            })
            .collect();
        matches.sort_unstable();
        matches
            .into_iter()
            .take(RESULT_LIMIT)
            .map(|(_, _, position)| entries[position].result())
            .collect()
    };
    let status = if results.is_empty() {
        "No matches"
    } else {
        "Search by name or keyword · Enter to copy"
    };
    Batch {
        results,
        status: status.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_copies_complete_sequences_and_distinguishes_emoji_from_symbols() {
        for (kind, query, expected) in [
            (Kind::Emoji, "CHECK MARK", "✔️"),
            (Kind::Emoji, "thumbs up medium skin tone", "👍🏽"),
            (Kind::Emoji, "woman technologist", "👩‍💻"),
            (Kind::Emoji, "❤", "❤️"),
            (Kind::Emoji, "flag japan", "🇯🇵"),
            (Kind::Symbol, "check mark", "✓"),
            (Kind::Symbol, "em dash", "—"),
            (Kind::Symbol, "ohm", "Ω"),
        ] {
            let batch = search(kind, query);
            assert_eq!(
                batch.results[0].actions[0].action,
                Action::Copy {
                    text: expected.into()
                },
                "{query}",
            );
        }
        assert!(
            search(Kind::Symbol, "woman technologist")
                .results
                .is_empty()
        );
        for kind in [Kind::Emoji, Kind::Symbol] {
            assert_eq!(search(kind, "").results.len(), RESULT_LIMIT);
        }
    }
}
