use ::kdl::{KdlDocument, KdlValue};

pub(super) fn first_child_value<'a>(children: &'a KdlDocument, name: &str) -> Option<&'a KdlValue> {
    children
        .get(name)
        .and_then(|node| node.entries().first())
        .map(|entry| entry.value())
}

pub(super) fn required_string_child(
    children: &KdlDocument,
    name: &str,
    path: &str,
) -> anyhow::Result<String> {
    match first_child_value(children, name).and_then(|v| v.as_string()) {
        Some(s) => Ok(s.to_string()),
        None => anyhow::bail!("{path}: expected a string value."),
    }
}

pub(super) fn optional_nonempty_string_child(
    children: &KdlDocument,
    name: &str,
    default: String,
    path: &str,
    expected: &str,
) -> anyhow::Result<String> {
    match children.get(name) {
        None => Ok(default),
        Some(node) => match node.entries().first().map(|entry| entry.value()) {
            Some(v) => match v.as_string() {
                Some("") => anyhow::bail!("{path}: value must not be empty. Expected: {expected}."),
                Some(s) => Ok(s.to_string()),
                None => anyhow::bail!("{path}: expected a string value."),
            },
            None => anyhow::bail!("{path}: expected a string value."),
        },
    }
}

pub(super) fn optional_u8_range_child(
    children: &KdlDocument,
    name: &str,
    default: u8,
    min: u8,
    max: u8,
    path: &str,
) -> anyhow::Result<u8> {
    match children.get(name) {
        None => Ok(default),
        Some(node) => match node.entries().first().map(|entry| entry.value()) {
            Some(v) => match v.as_integer() {
                Some(n) => match u8::try_from(n) {
                    Ok(n) if n >= min && n <= max => Ok(n),
                    Ok(n) => anyhow::bail!(
                        "{path}: value {n} out of range. Expected: integer {min}–{max}."
                    ),
                    Err(_) => anyhow::bail!(
                        "{path}: value {n} out of range. Expected: integer {min}–{max}."
                    ),
                },
                None => anyhow::bail!("{path}: expected an integer value."),
            },
            None => anyhow::bail!("{path}: expected an integer value."),
        },
    }
}
