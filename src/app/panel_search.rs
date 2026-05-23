#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum SearchField {
    Label = 0,
    Secondary = 1,
    Breadcrumb = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SearchScore {
    pub(super) field: SearchField,
    pub(super) score: i32,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SearchText<'a> {
    pub(super) field: SearchField,
    pub(super) text: &'a str,
}

#[derive(Debug, Clone)]
pub(super) struct SearchMatch<T> {
    pub(super) item: T,
    pub(super) score: SearchScore,
    pub(super) source_index: usize,
}

pub(super) fn query_tokens(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(str::to_ascii_lowercase)
        .filter(|token| !token.is_empty())
        .collect()
}

pub(super) fn is_blank_query(query: &str) -> bool {
    query_tokens(query).is_empty()
}

pub(super) fn best_match(fields: &[SearchText<'_>], query: &str) -> Option<SearchScore> {
    let tokens = query_tokens(query);
    if tokens.is_empty() {
        return Some(SearchScore {
            field: SearchField::Label,
            score: 0,
        });
    }

    let mut total = 0;
    let mut lowest_priority_field = SearchField::Label;
    for token in &tokens {
        let token_score = [
            SearchField::Label,
            SearchField::Secondary,
            SearchField::Breadcrumb,
        ]
        .into_iter()
        .filter_map(|field| {
            let best_score = fields
                .iter()
                .filter(|candidate| candidate.field == field)
                .map(|candidate| candidate.text)
                .filter_map(|text| fuzzy_score(text, token))
                .max()?;
            Some(SearchScore {
                field,
                score: best_score,
            })
        })
        .min_by(|left, right| {
            left.field
                .cmp(&right.field)
                .then_with(|| right.score.cmp(&left.score))
        })?;
        lowest_priority_field = lowest_priority_field.max(token_score.field);
        total += token_score.score;
    }

    Some(SearchScore {
        field: lowest_priority_field,
        score: total,
    })
}

pub(super) fn rank_matches<T>(mut matches: Vec<SearchMatch<T>>) -> Vec<T> {
    matches.sort_by(|left, right| {
        left.score
            .field
            .cmp(&right.score.field)
            .then_with(|| right.score.score.cmp(&left.score.score))
            .then_with(|| left.source_index.cmp(&right.source_index))
    });
    matches.into_iter().map(|matched| matched.item).collect()
}

fn fuzzy_score(text: &str, token: &str) -> Option<i32> {
    if token.is_empty() {
        return Some(0);
    }

    let text_lower = text.to_ascii_lowercase();
    if let Some(index) = text_lower.find(token) {
        let boundary_bonus = if is_boundary(&text_lower, index) {
            30
        } else {
            0
        };
        return Some(10_000 + boundary_bonus - index as i32);
    }

    let mut score = 0;
    let mut search_start = 0;
    let mut previous_match = None;
    for ch in token.chars() {
        let remainder = &text_lower[search_start..];
        let relative_index = remainder.find(ch)?;
        let absolute_index = search_start + relative_index;
        score += 100;
        if previous_match == Some(absolute_index.saturating_sub(1)) {
            score += 25;
        }
        if is_boundary(&text_lower, absolute_index) {
            score += 15;
        }
        score -= relative_index as i32;
        previous_match = Some(absolute_index);
        search_start = absolute_index + ch.len_utf8();
    }

    Some(score - text_lower.len() as i32)
}

fn is_boundary(text: &str, index: usize) -> bool {
    if index == 0 {
        return true;
    }
    let mut previous = text[..index].chars().rev();
    let Some(prev) = previous.next() else {
        return true;
    };
    !prev.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordered_subsequence_is_required() {
        assert!(fuzzy_score("Firefox", "fx").is_some());
        assert!(fuzzy_score("Firefox", "xf").is_none());
    }

    #[test]
    fn all_whitespace_tokens_must_match() {
        let fields = [
            SearchText {
                field: SearchField::Label,
                text: "Visual Studio Code",
            },
            SearchText {
                field: SearchField::Secondary,
                text: "editor",
            },
        ];

        assert!(best_match(&fields, "code editor").is_some());
        assert!(best_match(&fields, "code browser").is_none());
    }

    #[test]
    fn label_field_beats_secondary_field() {
        let label = best_match(
            &[SearchText {
                field: SearchField::Label,
                text: "Network Connections",
            }],
            "net",
        )
        .expect("label match");
        let secondary = best_match(
            &[SearchText {
                field: SearchField::Secondary,
                text: "network",
            }],
            "net",
        )
        .expect("secondary match");

        let ranked = rank_matches(vec![
            SearchMatch {
                item: "secondary",
                score: secondary,
                source_index: 0,
            },
            SearchMatch {
                item: "label",
                score: label,
                source_index: 1,
            },
        ]);

        assert_eq!(ranked, vec!["label", "secondary"]);
    }

    #[test]
    fn substring_beats_non_contiguous_fuzzy_match() {
        let substring = fuzzy_score("Terminal", "term").expect("substring");
        let fuzzy = fuzzy_score("Telemetry Renderer Module", "term").expect("fuzzy");

        assert!(substring > fuzzy);
    }

    #[test]
    fn source_order_breaks_equal_score_ties() {
        let score = SearchScore {
            field: SearchField::Label,
            score: 10,
        };

        let ranked = rank_matches(vec![
            SearchMatch {
                item: "second",
                score,
                source_index: 1,
            },
            SearchMatch {
                item: "first",
                score,
                source_index: 0,
            },
        ]);

        assert_eq!(ranked, vec!["first", "second"]);
    }
}
