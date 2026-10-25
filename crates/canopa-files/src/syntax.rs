//! Lightweight lexical highlighting for bounded, read-only source previews.
use crate::{
    Cancellation,
    preview::{HighlightKind, TextHighlight},
};
use std::io;

pub(crate) fn highlight(
    text: &str,
    language: Option<&str>,
    cancellation: &Cancellation,
) -> io::Result<Vec<TextHighlight>> {
    let Some(language) = language else {
        return Ok(Vec::new());
    };
    let chars: Vec<char> = text.chars().take(128 * 1024).collect();
    let hash = matches!(
        language,
        "py" | "sh" | "rb" | "toml" | "yaml" | "yml" | "ini" | "conf" | "md"
    );
    let slash = matches!(
        language,
        "rs" | "js"
            | "ts"
            | "tsx"
            | "jsx"
            | "css"
            | "c"
            | "cpp"
            | "h"
            | "hpp"
            | "go"
            | "java"
            | "json"
    );
    let dash = matches!(language, "sql" | "lua");
    let markup = matches!(language, "html" | "xml" | "svg");
    let mut spans = Vec::new();
    let mut position = 0;
    while position < chars.len() && spans.len() < 8192 {
        if cancellation.is_cancelled() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "preview cancelled",
            ));
        }
        let start = position;
        let character = chars[position];
        let starts = |pattern: &str| {
            chars[position..]
                .iter()
                .copied()
                .zip(pattern.chars())
                .all(|(a, b)| a == b)
                && chars.len() - position >= pattern.chars().count()
        };
        let kind =
            if (hash && character == '#') || (slash && starts("//")) || (dash && starts("--")) {
                while position < chars.len() && chars[position] != '\n' {
                    position += 1;
                }
                Some(HighlightKind::Comment)
            } else if (slash && starts("/*")) || (markup && starts("<!--")) {
                let end: Vec<_> = if markup { "-->" } else { "*/" }.chars().collect();
                position += if markup { 4 } else { 2 };
                while position < chars.len() {
                    if chars[position..].starts_with(&end) {
                        position += end.len();
                        break;
                    }
                    position += 1;
                }
                Some(HighlightKind::Comment)
            } else if matches!(character, '"' | '\'' | '`')
                && !(language == "rs"
                    && character == '\''
                    && chars.get(position + 2) != Some(&'\'')
                    && chars.get(position + 3) != Some(&'\''))
            {
                position += 1;
                let triple = (language == "py" || language == "toml")
                    && chars.get(position) == Some(&character)
                    && chars.get(position + 1) == Some(&character);
                if triple {
                    position += 2;
                }
                while position < chars.len() {
                    if chars[position] == '\\' {
                        position = (position + 2).min(chars.len());
                        continue;
                    }
                    if chars[position] == character {
                        position += 1;
                        if !triple {
                            break;
                        }
                        if chars.get(position) == Some(&character)
                            && chars.get(position + 1) == Some(&character)
                        {
                            position += 2;
                            break;
                        }
                    } else {
                        if chars[position] == '\n' && !triple && character != '`' {
                            break;
                        }
                        position += 1;
                    }
                }
                Some(HighlightKind::String)
            } else if character.is_ascii_digit()
                && (position == 0
                    || !(chars[position - 1].is_alphanumeric() || chars[position - 1] == '_'))
            {
                position += 1;
                while position < chars.len()
                    && (chars[position].is_ascii_alphanumeric()
                        || matches!(chars[position], '.' | '_'))
                {
                    position += 1;
                }
                Some(HighlightKind::Number)
            } else if character.is_alphabetic() || character == '_' {
                position += 1;
                while position < chars.len()
                    && (chars[position].is_alphanumeric() || chars[position] == '_')
                {
                    position += 1;
                }
                let word: String = chars[start..position].iter().collect();
                matches!(
                    word.as_str(),
                    "as" | "async"
                        | "await"
                        | "break"
                        | "case"
                        | "catch"
                        | "class"
                        | "const"
                        | "continue"
                        | "def"
                        | "del"
                        | "do"
                        | "else"
                        | "elif"
                        | "end"
                        | "enum"
                        | "except"
                        | "export"
                        | "extends"
                        | "false"
                        | "False"
                        | "finally"
                        | "fn"
                        | "for"
                        | "from"
                        | "func"
                        | "function"
                        | "if"
                        | "impl"
                        | "import"
                        | "in"
                        | "interface"
                        | "let"
                        | "local"
                        | "loop"
                        | "match"
                        | "mod"
                        | "mut"
                        | "new"
                        | "nil"
                        | "None"
                        | "null"
                        | "package"
                        | "pass"
                        | "private"
                        | "pub"
                        | "public"
                        | "raise"
                        | "return"
                        | "self"
                        | "Self"
                        | "static"
                        | "struct"
                        | "super"
                        | "switch"
                        | "this"
                        | "throw"
                        | "trait"
                        | "true"
                        | "True"
                        | "try"
                        | "type"
                        | "typeof"
                        | "unsafe"
                        | "use"
                        | "var"
                        | "void"
                        | "where"
                        | "while"
                        | "with"
                        | "yield"
                        | "SELECT"
                        | "FROM"
                        | "WHERE"
                        | "INSERT"
                        | "UPDATE"
                        | "DELETE"
                )
                .then_some(HighlightKind::Keyword)
            } else {
                position += 1;
                None
            };
        if let Some(kind) = kind {
            spans.push(TextHighlight {
                start: start as u32,
                end: position as u32,
                kind,
            });
        }
    }
    Ok(spans)
}
