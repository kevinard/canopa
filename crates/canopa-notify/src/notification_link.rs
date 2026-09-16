use serde::{Deserialize, Serialize};

/// A bounded absolute web URI. Construction and deserialization share the same
/// policy, so stored content cannot bypass validation when it is reopened.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct NotificationLink(String);

impl NotificationLink {
    pub fn parse(value: &str) -> Result<Self, &'static str> {
        const INVALID: &str = "unsupported notification link";
        if value.len() > 2048
            || value.chars().any(|ch| ch.is_whitespace() || ch.is_control()
                || ch == '\\' || matches!(ch, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'))
        {
            return Err(INVALID);
        }
        let (scheme, rest) = value.split_once("://").ok_or(INVALID)?;
        if !(scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https"))
            || rest.starts_with('/')
            || rest.is_empty()
        {
            return Err(INVALID);
        }
        let bytes = value.as_bytes();
        for (index, byte) in bytes.iter().enumerate() {
            if *byte == b'%' {
                let pair = bytes.get(index + 1..index + 3).ok_or(INVALID)?;
                let high = (pair[0] as char).to_digit(16).ok_or(INVALID)?;
                let low = (pair[1] as char).to_digit(16).ok_or(INVALID)?;
                let decoded = (high * 16 + low) as u8;
                if decoded.is_ascii_control() || decoded == b'\\' {
                    return Err(INVALID);
                }
            }
        }
        let uri = url::Url::parse(value).map_err(|_| INVALID)?;
        if uri.host_str().is_none()
            || !uri.username().is_empty()
            || uri.password().is_some()
            || uri.as_str().len() > 2048
        {
            return Err(INVALID);
        }
        Ok(Self(uri.into()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for NotificationLink {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<NotificationLink> for String {
    fn from(value: NotificationLink) -> Self {
        value.0
    }
}
