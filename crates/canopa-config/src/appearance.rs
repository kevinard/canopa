use crate::{Code, Diagnostic};
use canopa_theme::{Appearance, ColorScheme, Rgb};
use kdl::KdlDocument;

pub(crate) fn parse(document: &KdlDocument) -> Result<Appearance, Diagnostic> {
    let mut appearance = Appearance::default();
    let Some(block) = document.get("appearance") else {
        return Ok(appearance);
    };
    let mut seen = std::collections::HashSet::new();
    for node in block.children().expect("validated envelope").nodes() {
        let fail = || Diagnostic::new(Code::Appearance, Some(node.span().offset()));
        if node.ty().is_some()
            || node.children().is_some()
            || node.entries().len() != 1
            || !seen.insert(node.name().value())
        {
            return Err(fail());
        }
        let entry = &node.entries()[0];
        if entry.name().is_some() || entry.ty().is_some() {
            return Err(fail());
        }
        let value = entry.value().as_string().ok_or_else(fail)?;
        match node.name().value() {
            "color-scheme" => {
                appearance.color_scheme = match value {
                    "dark" => ColorScheme::Dark,
                    "light" => ColorScheme::Light,
                    _ => return Err(fail()),
                }
            }
            "accent" => appearance.accent = Some(Rgb::parse_hex(value).ok_or_else(fail)?),
            _ => return Err(fail()),
        }
    }
    Ok(appearance)
}
