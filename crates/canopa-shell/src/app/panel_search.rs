#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum SearchField {
    Label = 0,
    Secondary = 1,
    Breadcrumb = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SearchScore {
    pub(crate) field: SearchField,
    pub(crate) score: i32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SearchText<'a> {
    pub(crate) field: SearchField,
    pub(crate) text: &'a str,
}

#[derive(Debug, Clone)]
pub(crate) struct SearchMatch<T> {
    pub(crate) item: T,
    pub(crate) score: SearchScore,
    pub(crate) source_index: usize,
}

pub(crate) fn query_tokens(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(fold)
        .filter(|token| !token.is_empty())
        .collect()
}

/// Folds `text` for matching: Unicode lowercase, then accents removed as
/// canonical decomposition followed by dropping combining marks would.
///
/// The workspace has no Unicode normalization crate, so precomposed letters
/// use [`ACCENT_BASES`], which covers Latin (including Vietnamese), Greek
/// with tonos and dialytika, and Cyrillic. Other scripts, CJK included, keep
/// their characters; separate combining marks are dropped everywhere.
pub(crate) fn fold(text: &str) -> String {
    if text.is_ascii() {
        return text.to_ascii_lowercase();
    }
    let mut folded = String::with_capacity(text.len());
    for ch in text.chars() {
        fold_char(ch, &mut folded);
    }
    folded
}

/// The byte range of `text` whose folded form contains folded `query`, so a
/// highlight covers whole original characters, including trailing marks.
pub(crate) fn find_folded(text: &str, query: &str) -> Option<std::ops::Range<usize>> {
    let needle = fold(query);
    if needle.is_empty() {
        return None;
    }
    let mut folded = String::with_capacity(text.len());
    // (start in `folded`, original byte range) for each character that folds to something.
    let mut origins: Vec<(usize, std::ops::Range<usize>)> = Vec::new();
    for (index, ch) in text.char_indices() {
        let start = folded.len();
        fold_char(ch, &mut folded);
        let end = index + ch.len_utf8();
        if folded.len() > start {
            origins.push((start, index..end));
        } else if let Some((_, range)) = origins.last_mut() {
            range.end = end;
        }
    }
    let start = folded.find(&needle)?;
    let end = start + needle.len();
    let first = origins.partition_point(|(origin, _)| *origin <= start) - 1;
    let last = origins.partition_point(|(origin, _)| *origin < end) - 1;
    Some(origins[first].1.start..origins[last].1.end)
}

fn fold_char(ch: char, folded: &mut String) {
    for lower in ch.to_lowercase() {
        match lower {
            _ if is_combining_mark(lower) => {}
            // Full case folding, beyond lowercase.
            'ß' => folded.push_str("ss"),
            'ς' => folded.push('σ'),
            'ſ' => folded.push('s'),
            _ => folded.push(accent_base(lower).unwrap_or(lower)),
        }
    }
}

fn is_combining_mark(ch: char) -> bool {
    matches!(
        ch,
        '\u{0300}'..='\u{036F}'
            | '\u{1AB0}'..='\u{1AFF}'
            | '\u{1DC0}'..='\u{1DFF}'
            | '\u{FE20}'..='\u{FE2F}'
    )
}

/// Lowercase letters whose canonical decomposition is a base letter plus
/// combining marks. Each block maps consecutive code points from its start to
/// the base letters in its string; `_` keeps the character, for letters such
/// as `ø`, `ł` or `đ` that do not decompose.
const ACCENT_BASES: &[(char, &str)] = &[
    ('\u{00E0}', "aaaaaa_ceeeeiiii_nooooo__uuuuy_y"),
    (
        '\u{0100}',
        concat!(
            "aaaaaaccccccccdd",
            "__eeeeeeeeeegggg",
            "gggghh__iiiiiiii",
            "i___jjkk_llllll_",
            "___nnnnnn___oooo",
            "oo__rrrrrrssssss",
            "sstttt__uuuuuuuu",
            "uuuuwwyyyzzzzzz_",
        ),
    ),
    ('\u{01A0}', "oo"),
    ('\u{01AF}', "uu"),
    ('\u{01CD}', "aaiioouuuuuuuuuu"),
    ('\u{01DE}', "aaaa"),
    ('\u{01E6}', "ggkkoooo"),
    ('\u{01F0}', "j"),
    ('\u{01F4}', "gg"),
    ('\u{01F8}', "nnaa"),
    ('\u{0200}', "aaaaeeeeiiiioooorrrruuuusstt"),
    ('\u{021E}', "hh"),
    ('\u{0226}', "aaeeooooooooyy"),
    ('\u{0390}', "ι"),
    ('\u{03AC}', "αεηιυ"),
    ('\u{03CA}', "ιυουω"),
    ('\u{0439}', "и"),
    ('\u{0450}', "ее_г___і____киу_"),
    (
        '\u{1E00}',
        concat!(
            "aabbbbbbccdddddd",
            "ddddeeeeeeeeeeff",
            "gghhhhhhhhhhiiii",
            "kkkkkkllllllllmm",
            "mmmmnnnnnnnnoooo",
            "oooopppprrrrrrrr",
            "sssssssssstttttt",
            "ttuuuuuuuuuuvvvv",
            "wwwwwwwwwwxxxxyy",
            "zzzzzzhtwy",
        ),
    ),
    (
        '\u{1EA0}',
        concat!(
            "aaaaaaaaaaaaaaaa",
            "aaaaaaaaeeeeeeee",
            "eeeeeeeeiiiioooo",
            "oooooooooooooooo",
            "oooouuuuuuuuuuuu",
            "uuyyyyyyyy",
        ),
    ),
];

fn accent_base(ch: char) -> Option<char> {
    let (start, bases) = ACCENT_BASES.iter().rev().find(|(start, _)| *start <= ch)?;
    let offset = (ch as u32 - *start as u32) as usize;
    bases.chars().nth(offset).filter(|base| *base != '_')
}

pub(crate) fn is_blank_query(query: &str) -> bool {
    query_tokens(query).is_empty()
}

pub(crate) fn best_match(fields: &[SearchText<'_>], query: &str) -> Option<SearchScore> {
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

pub(crate) fn rank_matches<T>(mut matches: Vec<SearchMatch<T>>) -> Vec<T> {
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

    let text_lower = fold(text);
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
    !prev.is_alphanumeric()
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

    fn label(text: &str) -> [SearchText<'_>; 1] {
        [SearchText {
            field: SearchField::Label,
            text,
        }]
    }

    #[test]
    fn launcher_search_folds_cyrillic_case() {
        assert!(best_match(&label("Терминал"), "терм").is_some());
        assert!(best_match(&label("терминал"), "ТЕРМ").is_some());
        assert!(best_match(&label("Ёлка"), "елка").is_some());
        assert!(best_match(&label("Терминал"), "файл").is_none());
    }

    #[test]
    fn launcher_search_folds_greek_case_tonos_and_final_sigma() {
        assert!(best_match(&label("Αριθμομηχανή"), "αριθμ").is_some());
        assert!(best_match(&label("Ρυθμίσεις"), "ρυθμισεις").is_some());
        assert!(best_match(&label("ΡΥΘΜΙΣΕΙΣ"), "ρυθμίσεις").is_some());
        assert_eq!(fold("Σύστημα ΟΔΥΣΣΕΥΣ"), "συστημα οδυσσευσ");
    }

    #[test]
    fn launcher_search_folds_latin_accents_both_ways() {
        assert!(best_match(&label("Café"), "cafe").is_some());
        assert!(best_match(&label("Cafe"), "café").is_some());
        assert!(best_match(&label("Éditeur de texte"), "éd").is_some());
        assert!(best_match(&label("Éditeur de texte"), "EDITEUR").is_some());
        assert!(best_match(&label("Straße"), "strasse").is_some());
        assert!(best_match(&label("Trình duyệt"), "trinh duyet").is_some());
        assert!(best_match(&label("Ångström"), "angstrom").is_some());
        // Already-decomposed input loses its combining marks too.
        assert!(best_match(&label("Cafe\u{301}"), "café").is_some());
        // Letters without a decomposition stay distinct.
        assert!(best_match(&label("Łódź"), "lodz").is_none());
        assert!(best_match(&label("Łódź"), "łodz").is_some());
    }

    #[test]
    fn launcher_search_matches_cjk_without_case() {
        assert!(best_match(&label("终端"), "终端").is_some());
        assert!(best_match(&label("ファイル マネージャー"), "ファイル").is_some());
        assert!(best_match(&label("파일 관리자"), "관리").is_some());
        assert!(best_match(&label("终端"), "文件").is_none());
        assert_eq!(fold("終端エミュレータ"), "終端エミュレータ");
    }

    #[test]
    fn unicode_word_starts_earn_the_boundary_bonus() {
        let start = fuzzy_score("Ёлка Терминал", "тер").expect("word start");
        let middle = fuzzy_score("Ёлкатерминал", "тер").expect("word middle");
        assert!(start > middle);
    }

    #[test]
    fn launcher_highlight_maps_folded_matches_to_original_text() {
        fn highlight<'a>(text: &'a str, query: &str) -> Option<&'a str> {
            find_folded(text, query).map(|range| &text[range])
        }

        assert_eq!(highlight("Терминал", "терм"), Some("Терм"));
        assert_eq!(highlight("Mon Café", "cafe"), Some("Café"));
        assert_eq!(highlight("Éditeur", "ed"), Some("Éd"));
        assert_eq!(highlight("Cafe\u{301} noir", "cafe"), Some("Cafe\u{301}"));
        assert_eq!(highlight("Große Datei", "grosse"), Some("Große"));
        assert_eq!(highlight("Große Datei", "gros"), Some("Groß"));
        assert_eq!(highlight("Ρυθμίσεις", "μισ"), Some("μίσ"));
        assert_eq!(highlight("文件管理器", "管理"), Some("管理"));
        assert_eq!(highlight("Firefox", "fox"), Some("fox"));
        assert_eq!(highlight("Firefox", "zz"), None);
        assert_eq!(highlight("Firefox", ""), None);
    }

    #[test]
    fn accent_blocks_do_not_overlap() {
        for pair in ACCENT_BASES.windows(2) {
            let (start, bases) = pair[0];
            let end = start as u32 + bases.chars().count() as u32;
            assert!(
                end <= pair[1].0 as u32,
                "block at {start:?} overlaps the next"
            );
        }
    }
}
