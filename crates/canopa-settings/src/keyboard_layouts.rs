//! Keyboard layout names from the XKB registry, so the Keyboard page can show
//! "French" for `fr`. The layout codes stay the stored value; names are only
//! shown. The registry's English descriptions are used as XKB ships them.
use std::path::PathBuf;
use std::sync::OnceLock;

/// A layout code and its description, such as `("fr", "French")`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    pub code: String,
    pub name: String,
}

/// The registry's layouts by description, read once; empty when XKB's rules
/// are not installed.
pub fn all() -> &'static [Layout] {
    static LAYOUTS: OnceLock<Vec<Layout>> = OnceLock::new();
    LAYOUTS.get_or_init(|| {
        registry()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .map(|text| parse(&text))
            .unwrap_or_default()
    })
}

/// The name of `code`, or the code itself when the registry doesn't know it.
pub fn name(code: &str) -> String {
    all()
        .iter()
        .find(|layout| layout.code == code)
        .map_or_else(|| code.to_owned(), |layout| layout.name.clone())
}

/// The codes in a comma-separated layout list, trimmed, empty ones dropped.
pub fn codes(list: &str) -> Vec<&str> {
    list.split(',')
        .map(str::trim)
        .filter(|code| !code.is_empty())
        .collect()
}

/// `list` with `code` appended, unless it is already there.
pub fn with(list: &str, code: &str) -> String {
    let mut codes = codes(list);
    if !codes.contains(&code) {
        codes.push(code);
    }
    codes.join(",")
}

/// `list` without `code`.
pub fn without(list: &str, code: &str) -> String {
    codes(list)
        .into_iter()
        .filter(|kept| *kept != code)
        .collect::<Vec<_>>()
        .join(",")
}

fn registry() -> Option<PathBuf> {
    let roots = std::env::var_os("XKB_CONFIG_ROOT")
        .map(PathBuf::from)
        .into_iter()
        .chain([PathBuf::from("/usr/share/X11/xkb")]);
    for root in roots {
        for rules in ["evdev.xml", "base.xml"] {
            let path = root.join("rules").join(rules);
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}

fn parse(text: &str) -> Vec<Layout> {
    // The registry declares its DTD, which the parser refuses by default.
    let options = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    let Ok(document) = roxmltree::Document::parse_with_options(text, options) else {
        return Vec::new();
    };
    let mut layouts: Vec<Layout> = document
        .descendants()
        .filter(|node| node.has_tag_name("layout"))
        .filter_map(|layout| {
            let item = layout
                .children()
                .find(|node| node.has_tag_name("configItem"))?;
            let field = |tag: &str| {
                item.children()
                    .find(|node| node.has_tag_name(tag))
                    .and_then(|node| node.text())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned)
            };
            Some(Layout {
                code: field("name")?,
                name: field("description")?,
            })
        })
        .collect();
    layouts.sort_by_key(|layout| layout.name.to_lowercase());
    layouts.dedup_by(|a, b| a.code == b.code);
    layouts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_are_read_by_code_with_their_description() {
        let text = r#"<?xml version="1.0"?><!DOCTYPE xkbConfigRegistry SYSTEM "xkb.dtd"><xkbConfigRegistry><layoutList>
            <layout><configItem><name>us</name><description>English (US)</description></configItem>
              <variantList><variant><configItem><name>intl</name><description>English (US, intl.)</description></configItem></variant></variantList></layout>
            <layout><configItem><name>fr</name><description>French</description></configItem></layout>
            </layoutList></xkbConfigRegistry>"#;
        let layouts = parse(text);
        assert_eq!(
            layouts,
            [
                Layout {
                    code: "us".into(),
                    name: "English (US)".into()
                },
                Layout {
                    code: "fr".into(),
                    name: "French".into()
                },
            ]
        );
    }

    #[test]
    fn adding_a_layout_keeps_the_order_and_skips_repeats() {
        assert_eq!(with("us", "fr"), "us,fr");
        assert_eq!(with(" us , fr ", "us"), "us,fr");
        assert_eq!(with("", "de"), "de");
        assert_eq!(codes("us,,fr "), ["us", "fr"]);
        assert_eq!(without("us,fr,de", "fr"), "us,de");
    }
}
