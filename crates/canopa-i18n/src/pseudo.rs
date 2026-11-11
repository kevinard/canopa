//! The pseudo-locale: English with accented letters, about 40% longer, framed
//! by brackets and shown right to left, so truncation, hard-coded text and
//! missing mirroring are visible. Selected with `CANOPA_PSEUDO_LOCALE=1`.
use std::borrow::Cow;

/// Right-to-left embedding and pop: the framed text reads as an RTL run.
const RLE: char = '\u{202B}';
const PDF: char = '\u{202C}';

fn accented(c: char) -> char {
    match c {
        'a' => 'á',
        'b' => 'ƀ',
        'c' => 'ç',
        'd' => 'ð',
        'e' => 'é',
        'f' => 'ƒ',
        'g' => 'ĝ',
        'h' => 'ĥ',
        'i' => 'î',
        'j' => 'ĵ',
        'k' => 'ķ',
        'l' => 'ļ',
        'm' => 'ɱ',
        'n' => 'ñ',
        'o' => 'ö',
        'p' => 'þ',
        'q' => 'ǫ',
        'r' => 'ŕ',
        's' => 'š',
        't' => 'ţ',
        'u' => 'û',
        'v' => 'ṽ',
        'w' => 'ŵ',
        'x' => 'ẋ',
        'y' => 'ý',
        'z' => 'ž',
        'A' => 'Å',
        'B' => 'Ɓ',
        'C' => 'Ç',
        'D' => 'Ð',
        'E' => 'É',
        'F' => 'Ƒ',
        'G' => 'Ĝ',
        'H' => 'Ĥ',
        'I' => 'Î',
        'J' => 'Ĵ',
        'K' => 'Ķ',
        'L' => 'Ļ',
        'M' => 'Ṁ',
        'N' => 'Ñ',
        'O' => 'Ö',
        'P' => 'Þ',
        'Q' => 'Ǫ',
        'R' => 'Ŕ',
        'S' => 'Š',
        'T' => 'Ţ',
        'U' => 'Û',
        'V' => 'Ṽ',
        'W' => 'Ŵ',
        'X' => 'Ẋ',
        'Y' => 'Ý',
        'Z' => 'Ž',
        other => other,
    }
}

/// Accent the literal text of a message (placeables such as file names stay
/// as they are).
pub(crate) fn accent(text: &str) -> Cow<'_, str> {
    if text.chars().any(|c| c.is_ascii_alphabetic()) {
        Cow::Owned(text.chars().map(accented).collect())
    } else {
        Cow::Borrowed(text)
    }
}

/// Lengthen and frame a whole message.
pub(crate) fn frame(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let letters = text.chars().filter(|c| c.is_alphabetic()).count();
    let padding = letters.div_ceil(5).max(1);
    let mut framed = String::with_capacity(text.len() + padding * 2 + 8);
    framed.push(RLE);
    framed.push('[');
    framed.push_str(text);
    framed.push(' ');
    framed.extend(std::iter::repeat_n('·', padding));
    framed.push(']');
    framed.push(PDF);
    framed
}

/// Remove the pseudo-locale's frame and accents, for tests that compare with
/// English.
pub fn strip(text: &str) -> String {
    let inner = text
        .strip_prefix(RLE)
        .and_then(|rest| rest.strip_suffix(PDF))
        .and_then(|rest| rest.strip_prefix('['))
        .and_then(|rest| rest.strip_suffix(']'))
        .map(|rest| rest.trim_end_matches('·').trim_end())
        .unwrap_or(text);
    inner
        .chars()
        .map(|c| {
            ('a'..='z')
                .chain('A'..='Z')
                .find(|plain| accented(*plain) == c)
                .unwrap_or(c)
        })
        .collect()
}
