mod tests {
    use ::kdl::KdlDocument;

    use super::super::{ParseCtx, parse_tooltip_field_ctx};
    use crate::config::{ConfigSource, ConfigSourceName};

    fn parse(src: &str) -> KdlDocument {
        src.parse().expect("kdl parse")
    }

    fn ctx() -> ParseCtx {
        ParseCtx::root(ConfigSource::new(
            ConfigSourceName::test("<formats test>"),
            "",
        ))
        .child("m")
    }

    fn allowed() -> &'static [&'static str] {
        &["icon", "value", "state"]
    }

    #[test]
    fn tooltip_absent_returns_default_setting() {
        let doc = parse("");
        let s = parse_tooltip_field_ctx(&doc, &ctx(), allowed(), &[]).unwrap();
        assert!(matches!(s, crate::config::TooltipSetting::Default));
    }

    #[test]
    fn tooltip_string_returns_custom() {
        let doc = parse(r#"tooltip "{value}%""#);
        let s = parse_tooltip_field_ctx(&doc, &ctx(), allowed(), &[]).unwrap();
        assert!(matches!(s, crate::config::TooltipSetting::Custom(_)));
    }

    #[test]
    fn tooltip_false_returns_disabled() {
        let doc = parse("tooltip #false");
        let s = parse_tooltip_field_ctx(&doc, &ctx(), allowed(), &[]).unwrap();
        assert!(matches!(s, crate::config::TooltipSetting::Disabled));
    }

    #[test]
    fn tooltip_true_errors() {
        let doc = parse("tooltip #true");
        let err = parse_tooltip_field_ctx(&doc, &ctx(), allowed(), &[])
            .unwrap_err()
            .to_string();
        assert!(err.contains("tooltip true is not allowed"), "got: {err}");
    }

    #[test]
    fn tooltip_block_returns_custom() {
        let doc = parse(
            r#"tooltip {
                segment "{icon}"
                segment "{value}"
            }"#,
        );
        let s = parse_tooltip_field_ctx(&doc, &ctx(), allowed(), &[]).unwrap();
        match s {
            crate::config::TooltipSetting::Custom(t) => {
                assert_eq!(
                    t.render_segments(&[("icon", "i".into()), ("value", "v".into())]),
                    vec!["i".to_string(), "v".to_string()]
                );
            }
            _ => panic!("expected Custom"),
        }
    }
}
