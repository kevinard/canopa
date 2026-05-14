use ::kdl::{KdlDocument, KdlEntry, KdlNode};

use super::error::ParseCtx;
use crate::config::ConfigResult;

type RangeMessage<'a> = dyn Fn(i128, u8, u8) -> String + 'a;

#[derive(Default)]
struct Messages<'a> {
    help: Option<String>,
    missing: Option<String>,
    integer: Option<String>,
    string: Option<String>,
    number: Option<String>,
    bool_: Option<String>,
    range: Option<Box<RangeMessage<'a>>>,
}

pub(super) fn field<'a>(
    children: &'a KdlDocument,
    parent_ctx: &'a ParseCtx,
    name: &'a str,
) -> Field<'a> {
    Field {
        children,
        ctx: parent_ctx.child(name),
        name,
        required: true,
        messages: Messages::default(),
    }
}

pub(super) fn property<'a>(
    node: &'a KdlNode,
    parent_ctx: &'a ParseCtx,
    name: &'a str,
) -> Property<'a> {
    Property {
        node,
        ctx: parent_ctx.clone(),
        name,
        required: true,
        messages: Messages::default(),
    }
}

pub(super) struct Field<'a> {
    children: &'a KdlDocument,
    ctx: ParseCtx,
    name: &'a str,
    required: bool,
    messages: Messages<'a>,
}

pub(super) struct Property<'a> {
    node: &'a KdlNode,
    ctx: ParseCtx,
    name: &'a str,
    required: bool,
    messages: Messages<'a>,
}

impl<'a> Field<'a> {
    pub(super) fn optional(mut self) -> Self {
        self.required = false;
        self
    }

    pub(super) fn help(mut self, help: impl Into<String>) -> Self {
        self.messages.help = Some(help.into());
        self
    }

    pub(super) fn integer_message(mut self, message: impl Into<String>) -> Self {
        self.messages.integer = Some(message.into());
        self
    }

    pub(super) fn u64_min(self, min: u64) -> FieldValue<u64> {
        let help = self.messages.help.clone();
        self.parse_optional_node(|node, ctx| {
            let Some(entry) = node.entries().first() else {
                return Err(ctx.node_err(
                    node,
                    format!("missing value; expected integer {min} or greater"),
                    help.clone(),
                ));
            };
            let Some(value) = entry.value().as_integer() else {
                return Err(ctx.entry_err(
                    entry,
                    format!("expected integer {min} or greater"),
                    help.clone(),
                ));
            };
            let value = u64::try_from(value).map_err(|_| {
                ctx.entry_err(entry, format!("value must be at least {min}"), help.clone())
            })?;
            if value < min {
                return Err(ctx.entry_err(
                    entry,
                    format!("value must be at least {min}"),
                    help.clone(),
                ));
            }
            Ok(value)
        })
    }

    pub(super) fn u32(self) -> FieldValue<u32> {
        let help = self.messages.help.clone();
        let message = self
            .messages
            .integer
            .clone()
            .unwrap_or_else(|| "expected a positive integer".into());
        self.parse_optional_node(|node, ctx| {
            let Some(entry) = node.entries().first() else {
                return Err(ctx.node_err(node, message.clone(), help.clone()));
            };
            let Some(value) = entry.value().as_integer() else {
                return Err(ctx.entry_err(entry, message.clone(), help.clone()));
            };
            u32::try_from(value).map_err(|_| ctx.entry_err(entry, message, help.clone()))
        })
    }

    pub(super) fn u8_range(self, min: u8, max: u8) -> FieldValue<u8> {
        let help = self
            .messages
            .help
            .clone()
            .or_else(|| Some(format!("expected integer {min}-{max}")));
        self.parse_optional_node(|node, ctx| {
            let Some(entry) = node.entries().first() else {
                return Err(ctx.node_err(node, "expected an integer value", help.clone()));
            };
            let Some(value) = entry.value().as_integer() else {
                return Err(ctx.entry_err(entry, "expected an integer value", help.clone()));
            };
            let value = u8::try_from(value).map_err(|_| {
                ctx.entry_err(
                    entry,
                    format!("value {value} out of range. Expected: integer {min}-{max}"),
                    None,
                )
            })?;
            if value < min || value > max {
                return Err(ctx.entry_err(
                    entry,
                    format!("value {value} out of range. Expected: integer {min}-{max}"),
                    None,
                ));
            }
            Ok(value)
        })
    }

    pub(super) fn bool(self) -> FieldValue<bool> {
        let help = self.messages.help.clone();
        let message = self
            .messages
            .bool_
            .clone()
            .unwrap_or_else(|| "expected #true or #false".into());
        self.parse_optional_node(|node, ctx| {
            let Some(entry) = node.entries().first() else {
                return Err(ctx.node_err(node, message.clone(), help.clone()));
            };
            entry
                .value()
                .as_bool()
                .ok_or_else(|| ctx.entry_err(entry, message, help.clone()))
        })
    }

    pub(super) fn nonempty_string(self) -> FieldValue<String> {
        self.parse_optional_node(parse_nonempty_string_node)
    }

    pub(super) fn string(self) -> FieldValue<String> {
        self.parse_optional_node(parse_string_node)
    }

    pub(super) fn one_of<T>(
        self,
        expected: &str,
        parse: impl FnOnce(&str) -> Option<T>,
    ) -> FieldValue<T> {
        let help = self.messages.help.clone();
        self.parse_optional_node(|node, ctx| {
            let Some(entry) = node.entries().first() else {
                return Err(ctx.node_err(
                    node,
                    format!("expected a string value. Expected: one of: {expected}"),
                    help.clone()
                        .or_else(|| Some(format!("expected one of: {expected}"))),
                ));
            };
            let Some(raw) = entry.value().as_string() else {
                return Err(ctx.entry_err(
                    entry,
                    format!("expected a string value. Expected: one of: {expected}"),
                    help.clone()
                        .or_else(|| Some(format!("expected one of: {expected}"))),
                ));
            };
            parse(raw).ok_or_else(|| {
                ctx.entry_err(
                    entry,
                    format!("unknown value {raw:?}. Expected: one of: {expected}"),
                    help.clone()
                        .or_else(|| Some(format!("expected one of: {expected}"))),
                )
            })
        })
    }

    pub(super) fn positional_strings(self, message: &'static str) -> FieldValue<Vec<String>> {
        self.parse_optional_node(|node, ctx| {
            let mut values = Vec::new();
            for entry in node.entries().iter().filter(|entry| entry.name().is_none()) {
                let Some(value) = entry.value().as_string() else {
                    return Err(ctx.entry_err(entry, message, None));
                };
                values.push(value.to_string());
            }
            Ok(values)
        })
    }

    pub(super) fn required_nonempty_string(self) -> ConfigResult<String> {
        match self.single_node()? {
            Some(node) => parse_nonempty_string_node(node, &self.ctx),
            None => Err(self.ctx.err(
                None,
                "missing required parameter. Expected: a string value",
                None,
            )),
        }
    }

    pub(super) fn required_node(
        self,
        missing_message: impl Into<String>,
        help: Option<String>,
    ) -> ConfigResult<&'a KdlNode> {
        match self.single_node()? {
            Some(node) => Ok(node),
            None => Err(self.ctx.err(None, missing_message, help)),
        }
    }

    pub(super) fn optional_node(self) -> ConfigResult<Option<&'a KdlNode>> {
        self.single_node()
    }

    fn parse_optional_node<T>(
        self,
        parse: impl FnOnce(&KdlNode, &ParseCtx) -> ConfigResult<T>,
    ) -> FieldValue<T> {
        match self.single_node() {
            Ok(Some(node)) => FieldValue::Present(parse(node, &self.ctx)),
            Ok(None) if self.required => FieldValue::Present(Err(self.ctx.err(
                None,
                "missing required parameter",
                self.messages.help.clone(),
            ))),
            Ok(None) => FieldValue::Missing,
            Err(error) => FieldValue::Present(Err(error)),
        }
    }

    fn single_node(&self) -> ConfigResult<Option<&'a KdlNode>> {
        let mut matches = self
            .children
            .nodes()
            .iter()
            .filter(|node| node.name().value() == self.name);
        let first = matches.next();
        if let Some(second) = matches.next() {
            return Err(self.ctx.node_err(
                second,
                format!("duplicate field `{}`", self.name),
                Some(format!("remove one `{}` entry", self.name)),
            ));
        }
        Ok(first)
    }
}

pub(super) enum FieldValue<T> {
    Present(ConfigResult<T>),
    Missing,
}

impl<T> FieldValue<T> {
    pub(super) fn map<U>(self, f: impl FnOnce(T) -> U) -> FieldValue<U> {
        match self {
            Self::Present(value) => FieldValue::Present(value.map(f)),
            Self::Missing => FieldValue::Missing,
        }
    }

    pub(super) fn required(self) -> ConfigResult<T> {
        match self {
            Self::Present(value) => value,
            Self::Missing => unreachable!("required field values are represented as errors"),
        }
    }

    pub(super) fn unwrap_or(self, default: T) -> ConfigResult<T> {
        match self {
            Self::Present(value) => value,
            Self::Missing => Ok(default),
        }
    }

    pub(super) fn transpose(self) -> ConfigResult<Option<T>> {
        match self {
            Self::Present(value) => value.map(Some),
            Self::Missing => Ok(None),
        }
    }
}

impl<'a> Property<'a> {
    pub(super) fn optional(mut self) -> Self {
        self.required = false;
        self
    }

    pub(super) fn missing_message(mut self, message: impl Into<String>) -> Self {
        self.messages.missing = Some(message.into());
        self
    }

    pub(super) fn integer_message(mut self, message: impl Into<String>) -> Self {
        self.messages.integer = Some(message.into());
        self
    }

    pub(super) fn string_message(mut self, message: impl Into<String>) -> Self {
        self.messages.string = Some(message.into());
        self
    }

    pub(super) fn number_message(mut self, message: impl Into<String>) -> Self {
        self.messages.number = Some(message.into());
        self
    }

    pub(super) fn range_message(mut self, message: impl Fn(i128, u8, u8) -> String + 'a) -> Self {
        self.messages.range = Some(Box::new(message));
        self
    }

    pub(super) fn u8_range(self, min: u8, max: u8) -> FieldValue<u8> {
        let integer_message = self.messages.integer.clone();
        let range_message = self.messages.range.as_deref();
        match self.entry() {
            Some(entry) => FieldValue::Present(parse_u8_range_entry(
                entry,
                &self.ctx,
                min,
                max,
                integer_message.as_deref(),
                range_message,
            )),
            None if self.required => FieldValue::Present(Err(self.ctx.node_err(
                self.node,
                self.messages
                    .missing
                    .clone()
                    .unwrap_or_else(|| format!("missing `{}` property", self.name)),
                None,
            ))),
            None => FieldValue::Missing,
        }
    }

    pub(super) fn string(self) -> FieldValue<String> {
        let message = self.messages.string.clone();
        self.parse_optional_entry(|entry, ctx| parse_string_entry(entry, ctx, message.as_deref()))
    }

    pub(super) fn finite_number(self) -> FieldValue<f64> {
        let message = self.messages.number.clone();
        self.parse_optional_entry(|entry, ctx| {
            parse_finite_number_entry(entry, ctx, message.as_deref())
        })
    }

    fn parse_optional_entry<T>(
        self,
        parse: impl FnOnce(&KdlEntry, &ParseCtx) -> ConfigResult<T>,
    ) -> FieldValue<T> {
        match self.entry() {
            Some(entry) => FieldValue::Present(parse(entry, &self.ctx)),
            None if self.required => FieldValue::Present(Err(self.ctx.node_err(
                self.node,
                self.messages
                    .missing
                    .clone()
                    .unwrap_or_else(|| format!("missing `{}` property", self.name)),
                None,
            ))),
            None => FieldValue::Missing,
        }
    }

    fn entry(&self) -> Option<&'a KdlEntry> {
        self.node
            .entries()
            .iter()
            .find(|entry| entry.name().map(|entry_name| entry_name.value()) == Some(self.name))
    }
}

fn parse_u8_range_entry(
    entry: &KdlEntry,
    ctx: &ParseCtx,
    min: u8,
    max: u8,
    integer_message: Option<&str>,
    range_message: Option<&dyn Fn(i128, u8, u8) -> String>,
) -> ConfigResult<u8> {
    let Some(value) = entry.value().as_integer() else {
        return Err(ctx.entry_err(
            entry,
            integer_message.unwrap_or("expected an integer value"),
            None,
        ));
    };
    let value = u8::try_from(value).map_err(|_| {
        ctx.entry_err(
            entry,
            range_message.map_or_else(
                || format!("value {value} out of range. Expected: integer {min}-{max}"),
                |message| message(value, min, max),
            ),
            None,
        )
    })?;
    if value < min || value > max {
        return Err(ctx.entry_err(
            entry,
            range_message.map_or_else(
                || format!("value {value} out of range. Expected: integer {min}-{max}"),
                |message| message(i128::from(value), min, max),
            ),
            None,
        ));
    }
    Ok(value)
}

fn parse_string_entry(
    entry: &KdlEntry,
    ctx: &ParseCtx,
    message: Option<&str>,
) -> ConfigResult<String> {
    entry
        .value()
        .as_string()
        .map(ToString::to_string)
        .ok_or_else(|| ctx.entry_err(entry, message.unwrap_or("expected a string value"), None))
}

fn parse_finite_number_entry(
    entry: &KdlEntry,
    ctx: &ParseCtx,
    message: Option<&str>,
) -> ConfigResult<f64> {
    entry
        .value()
        .as_float()
        .or_else(|| entry.value().as_integer().map(|number| number as f64))
        .filter(|number| number.is_finite())
        .ok_or_else(|| ctx.entry_err(entry, message.unwrap_or("expected a finite number"), None))
}

fn parse_string_node(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<String> {
    let Some(entry) = node.entries().first() else {
        return Err(ctx.node_err(node, "expected a string value", None));
    };
    entry
        .value()
        .as_string()
        .map(ToString::to_string)
        .ok_or_else(|| ctx.entry_err(entry, "expected a string value", None))
}

fn parse_nonempty_string_node(node: &KdlNode, ctx: &ParseCtx) -> ConfigResult<String> {
    let Some(entry) = node.entries().first() else {
        return Err(ctx.node_err(node, "expected a string value", None));
    };
    match entry.value().as_string() {
        Some("") => Err(ctx.entry_err(entry, "value must not be empty", None)),
        Some(value) => Ok(value.to_string()),
        None => Err(ctx.entry_err(entry, "expected a string value", None)),
    }
}

#[cfg(test)]
mod tests {
    use ::kdl::KdlDocument;

    use super::super::error::ParseCtx;
    use crate::config::{ConfigSource, ConfigSourceName};

    fn ctx() -> ParseCtx {
        ParseCtx::root(ConfigSource::new(
            ConfigSourceName::test("<fields test>"),
            "",
        ))
    }

    fn doc(input: &str) -> KdlDocument {
        input.parse().unwrap()
    }

    #[test]
    fn optional_u64_min_returns_default_when_field_absent() {
        let doc = doc("");
        let value = super::field(&doc, &ctx(), "interval")
            .optional()
            .u64_min(1)
            .unwrap_or(5)
            .unwrap();

        assert_eq!(value, 5);
    }

    #[test]
    fn optional_u64_min_reports_duplicate_field_at_field_path() {
        let doc = doc("interval 5\ninterval 10");
        let err = super::field(&doc, &ctx(), "interval")
            .optional()
            .u64_min(1)
            .unwrap_or(5)
            .unwrap_err();

        let diagnostic = err.as_diagnostic().expect("diagnostic");
        assert_eq!(diagnostic.path(), Some("interval"));
        assert!(diagnostic.to_string().contains("duplicate"));
        assert!(diagnostic.has_span());
    }

    #[test]
    fn required_nonempty_string_reports_missing_field_at_field_path() {
        let doc = doc("");
        let err = super::field(&doc, &ctx(), "label")
            .required_nonempty_string()
            .unwrap_err();

        let diagnostic = err.as_diagnostic().expect("diagnostic");
        assert_eq!(diagnostic.path(), Some("label"));
        assert!(
            diagnostic
                .to_string()
                .contains("missing required parameter")
        );
    }

    #[test]
    fn required_node_returns_node_for_command_handoff() {
        let doc = doc("command \"date\"");
        let ctx = ctx();
        let node = super::field(&doc, &ctx, "command")
            .required_node("missing command", None)
            .unwrap();

        assert_eq!(node.name().value(), "command");
    }

    #[test]
    fn optional_node_returns_none_when_field_absent() {
        let doc = doc("");
        let ctx = ctx();
        let node = super::field(&doc, &ctx, "tooltip").optional_node().unwrap();

        assert!(node.is_none());
    }

    #[test]
    fn optional_bool_reports_wrong_type_at_field_path() {
        let doc = doc("allow-shell \"yes\"");
        let err = super::field(&doc, &ctx(), "allow-shell")
            .optional()
            .bool()
            .unwrap_or(true)
            .unwrap_err();

        let diagnostic = err.as_diagnostic().expect("diagnostic");
        assert_eq!(diagnostic.path(), Some("allow-shell"));
        assert!(diagnostic.to_string().contains("#true or #false"));
        assert!(diagnostic.has_span());
    }

    #[test]
    fn optional_nonempty_string_returns_configured_value() {
        let doc = doc(r#"time-format "%H:%M""#);
        let value = super::field(&doc, &ctx(), "time-format")
            .optional()
            .nonempty_string()
            .unwrap_or("%I:%M %p".to_string())
            .unwrap();

        assert_eq!(value, "%H:%M");
    }

    #[test]
    fn optional_string_allows_empty_value() {
        let doc = doc(r#"label """#);
        let value = super::field(&doc, &ctx(), "label")
            .optional()
            .string()
            .transpose()
            .unwrap();

        assert_eq!(value, Some(String::new()));
    }

    #[test]
    fn optional_u8_range_rejects_out_of_range_value() {
        let doc = doc("step 101");
        let err = super::field(&doc, &ctx(), "step")
            .optional()
            .u8_range(1, 100)
            .unwrap_or(5)
            .unwrap_err();

        let diagnostic = err.as_diagnostic().expect("diagnostic");
        assert_eq!(diagnostic.path(), Some("step"));
        assert!(diagnostic.to_string().contains("out of range"));
        assert!(diagnostic.has_span());
    }

    #[test]
    fn optional_u32_reports_negative_value_at_field_path() {
        let doc = doc("expand-ms -1");
        let err = super::field(&doc, &ctx(), "expand-ms")
            .optional()
            .u32()
            .unwrap_or(280)
            .unwrap_err();

        let diagnostic = err.as_diagnostic().expect("diagnostic");
        assert_eq!(diagnostic.path(), Some("expand-ms"));
        assert!(diagnostic.to_string().contains("positive integer"));
        assert!(diagnostic.has_span());
    }

    #[test]
    fn optional_enum_reports_unknown_value_at_field_path() {
        let doc = doc(r#"edge "diagonal""#);
        let err = super::field(&doc, &ctx(), "edge")
            .optional()
            .one_of("left, right, top, bottom", |value| match value {
                "left" => Some("left"),
                "right" => Some("right"),
                "top" => Some("top"),
                "bottom" => Some("bottom"),
                _ => None,
            })
            .unwrap_or("left")
            .unwrap_err();

        let diagnostic = err.as_diagnostic().expect("diagnostic");
        assert_eq!(diagnostic.path(), Some("edge"));
        assert!(diagnostic.to_string().contains("diagonal"));
        assert!(diagnostic.has_span());
    }

    #[test]
    fn optional_positional_strings_reads_all_values() {
        let doc = doc(r#"output "DP-1" "HDMI-A-1""#);
        let values = super::field(&doc, &ctx(), "output")
            .optional()
            .positional_strings("expected string Output Target entries")
            .unwrap_or(Vec::new())
            .unwrap();

        assert_eq!(values, vec!["DP-1", "HDMI-A-1"]);
    }

    #[test]
    fn required_property_u8_range_reads_named_property() {
        let doc = doc(r#"state "low" threshold=20"#);
        let ctx = ctx();
        let node = doc.get("state").unwrap();
        let threshold = super::property(node, &ctx, "threshold")
            .u8_range(0, 100)
            .required()
            .unwrap();

        assert_eq!(threshold, 20);
    }

    #[test]
    fn optional_property_string_reads_named_property() {
        let doc = doc(r#"icon "wifi" state="full""#);
        let ctx = ctx();
        let node = doc.get("icon").unwrap();
        let state = super::property(node, &ctx, "state")
            .optional()
            .string()
            .transpose()
            .unwrap();

        assert_eq!(state.as_deref(), Some("full"));
    }

    #[test]
    fn optional_property_finite_number_reads_integer_property() {
        let doc = doc(r#"icon "mid" at=50"#);
        let ctx = ctx();
        let node = doc.get("icon").unwrap();
        let at = super::property(node, &ctx, "at")
            .optional()
            .finite_number()
            .transpose()
            .unwrap();

        assert_eq!(at, Some(50.0));
    }
}
