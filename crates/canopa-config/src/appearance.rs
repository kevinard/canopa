use crate::{Code, Diagnostic};
use canopa_theme::{
    Appearance, ColorRole, ColorScheme, ColorValue, Insets, Pixels, Rgb, StateColors, UiKind,
    UiStyle,
};
use kdl::{KdlDocument, KdlNode};

fn fail(node: &KdlNode) -> Diagnostic {
    Diagnostic::new(Code::Appearance, Some(node.span().offset()))
}
fn block(node: &KdlNode) -> Result<&KdlDocument, Diagnostic> {
    if node.ty().is_some() || !node.entries().is_empty() {
        return Err(fail(node));
    }
    node.children().ok_or_else(|| fail(node))
}
fn values(node: &KdlNode) -> Result<(), Diagnostic> {
    if node.ty().is_some()
        || node.children().is_some()
        || node
            .entries()
            .iter()
            .any(|e| e.name().is_some() || e.ty().is_some())
    {
        return Err(fail(node));
    }
    Ok(())
}
fn string(node: &KdlNode) -> Result<&str, Diagnostic> {
    values(node)?;
    if node.entries().len() != 1 {
        return Err(fail(node));
    }
    node.entries()[0]
        .value()
        .as_string()
        .ok_or_else(|| fail(node))
}
fn unique(document: &KdlDocument) -> Result<(), Diagnostic> {
    let mut seen = std::collections::HashSet::new();
    for node in document.nodes() {
        if !seen.insert(node.name().value()) {
            return Err(fail(node));
        }
    }
    Ok(())
}
pub fn dimension(node: &KdlNode, max: f64) -> Result<Pixels, Diagnostic> {
    let items = dimensions(node, max)?;
    if items.len() != 1 {
        return Err(fail(node));
    }
    Ok(items[0])
}
fn dimensions(node: &KdlNode, max: f64) -> Result<Vec<Pixels>, Diagnostic> {
    values(node)?;
    node.entries()
        .iter()
        .map(|entry| {
            entry
                .value()
                .as_float()
                .or_else(|| entry.value().as_integer().map(|i| i as f64))
                .and_then(|n| Pixels::from_number(n, max))
                .ok_or_else(|| fail(node))
        })
        .collect()
}
pub fn insets(node: &KdlNode) -> Result<Insets, Diagnostic> {
    let v = dimensions(node, 4096.0)?;
    let (top, right, bottom, left) = match v.as_slice() {
        [all] => (*all, *all, *all, *all),
        [v, h] => (*v, *h, *v, *h),
        [t, r, b, l] => (*t, *r, *b, *l),
        _ => return Err(fail(node)),
    };
    Ok(Insets {
        top,
        right,
        bottom,
        left,
    })
}
fn color(node: &KdlNode) -> Result<ColorValue, Diagnostic> {
    ColorValue::parse(string(node)?).ok_or_else(|| fail(node))
}

/// Parse one local UI block with the same grammar as shared theme roles.
pub fn ui_style(node: &KdlNode, kind: UiKind) -> Result<UiStyle, Diagnostic> {
    parse_style(block(node)?, Some(kind))
}
fn parse_style(doc: &KdlDocument, kind: Option<UiKind>) -> Result<UiStyle, Diagnostic> {
    // State nodes repeat by state name, so check their identity separately.
    let mut seen = std::collections::HashSet::new();
    let mut style = UiStyle::default();
    for node in doc.nodes() {
        let name = node.name().value();
        if name != "state" && !seen.insert(name.to_string()) {
            return Err(fail(node));
        }
        match name {
            "background" => style.background = Some(color(node)?),
            "foreground" => style.foreground = Some(color(node)?),
            "padding" => style.padding = Some(insets(node)?),
            "margin"
                if kind.is_some_and(|k| {
                    !matches!(k, UiKind::Bar | UiKind::Button | UiKind::Input)
                }) =>
            {
                style.margin = Some(insets(node)?)
            }
            "gap" if kind.is_some_and(UiKind::has_gap) => {
                style.gap = Some(dimension(node, 4096.0)?)
            }
            "radius" => {
                let v = dimensions(node, 4096.0)?;
                style.radius = Some(match v.as_slice() {
                    [all] => [*all; 4],
                    [tl, tr, br, bl] => [*tl, *tr, *br, *bl],
                    _ => return Err(fail(node)),
                });
            }
            "border" => {
                if node.ty().is_some() || node.children().is_some() || node.entries().is_empty() {
                    return Err(fail(node));
                }
                let mut props = std::collections::HashSet::new();
                for e in node.entries() {
                    let key = e.name().map(|n| n.value()).ok_or_else(|| fail(node))?;
                    if e.ty().is_some() || !props.insert(key) {
                        return Err(fail(node));
                    }
                    match key {
                        "color" => {
                            style.border_color = Some(
                                e.value()
                                    .as_string()
                                    .and_then(ColorValue::parse)
                                    .ok_or_else(|| fail(node))?,
                            )
                        }
                        "width" => {
                            style.border_width = Some(
                                e.value()
                                    .as_float()
                                    .or_else(|| e.value().as_integer().map(|i| i as f64))
                                    .and_then(|n| Pixels::from_number(n, 64.0))
                                    .ok_or_else(|| fail(node))?,
                            )
                        }
                        _ => return Err(fail(node)),
                    }
                }
            }
            "state" if kind.is_some_and(UiKind::is_control) => {
                if node.ty().is_some()
                    || node.entries().len() != 1
                    || node.entries()[0].name().is_some()
                    || node.entries()[0].ty().is_some()
                {
                    return Err(fail(node));
                }
                let name = node.entries()[0]
                    .value()
                    .as_string()
                    .ok_or_else(|| fail(node))?;
                let index = match name {
                    "selected" => 0,
                    "hover" => 1,
                    "pressed" => 2,
                    "disabled" => 3,
                    "focus" => 4,
                    _ => return Err(fail(node)),
                };
                if !seen.insert(format!("state:{name}"))
                    || match kind {
                        Some(UiKind::Input) => matches!(index, 0 | 2),
                        Some(UiKind::Module) => matches!(index, 2 | 3),
                        Some(UiKind::MenuRow) => index == 2,
                        Some(UiKind::Button) => index == 0,
                        _ => true,
                    }
                {
                    return Err(fail(node));
                }
                let children = node.children().ok_or_else(|| fail(node))?;
                unique(children)?;
                let mut state = StateColors::default();
                for child in children.nodes() {
                    match child.name().value() {
                        "background" => state.background = Some(color(child)?),
                        "foreground" => state.foreground = Some(color(child)?),
                        "border-color" => state.border_color = Some(color(child)?),
                        _ => return Err(fail(child)),
                    }
                }
                style.states[index] = state;
            }
            _ => return Err(fail(node)),
        }
    }
    Ok(style)
}

pub(crate) fn theme_reference(document: &KdlDocument) -> Result<Option<&str>, Diagnostic> {
    document
        .get("appearance")
        .and_then(|node| node.children())
        .and_then(|d| d.get("theme"))
        .map(string)
        .transpose()
}

pub(crate) fn parse(
    document: &KdlDocument,
    theme: Option<&KdlDocument>,
) -> Result<Appearance, Diagnostic> {
    let mut appearance = Appearance::default();
    if let Some(theme) = theme {
        apply(&mut appearance, theme, false).map_err(crate::theme_source::theme_error)?;
    }
    if let Some(node) = document.get("appearance") {
        apply(&mut appearance, block(node)?, true)?;
    }
    Ok(appearance)
}

fn apply(
    appearance: &mut Appearance,
    doc: &KdlDocument,
    allow_theme: bool,
) -> Result<(), Diagnostic> {
    unique(doc)?;
    if doc.get("accent").is_some()
        && doc
            .get("palette")
            .and_then(|n| n.children())
            .is_some_and(|d| d.get("accent").is_some())
    {
        return Err(fail(doc.get("accent").unwrap()));
    }
    // Resolve scheme first; remaining fields are independent of KDL node order.
    if let Some(node) = doc.get("color-scheme") {
        appearance.color_scheme = match string(node)? {
            "dark" => ColorScheme::Dark,
            "light" => ColorScheme::Light,
            _ => return Err(fail(node)),
        };
    }
    for node in doc.nodes() {
        match node.name().value() {
            "theme" if allow_theme => {
                string(node)?;
            }
            "color-scheme" => {}
            "accent" => {
                let rgb = Rgb::parse_hex(string(node)?).ok_or_else(|| fail(node))?;
                appearance.accent = Some(rgb);
                // A legacy inline accent must override a theme-file palette accent.
                std::sync::Arc::make_mut(&mut appearance.customization).palette
                    [ColorRole::Accent as usize] = None;
            }
            "palette" => {
                let colors = block(node)?;
                unique(colors)?;
                let mut roles = std::collections::HashSet::new();
                for entry in colors.nodes() {
                    let role = ColorRole::parse(entry.name().value()).ok_or_else(|| fail(entry))?;
                    if !roles.insert(role as usize) {
                        return Err(fail(entry));
                    }
                    let rgb = Rgb::parse_hex(string(entry)?).ok_or_else(|| fail(entry))?;
                    std::sync::Arc::make_mut(&mut appearance.customization).palette
                        [role as usize] = Some(rgb);
                }
            }
            "typography" => {
                let typography = block(node)?;
                unique(typography)?;
                let t = &mut std::sync::Arc::make_mut(&mut appearance.customization).typography;
                for field in typography.nodes() {
                    match field.name().value() {
                        "family" => {
                            values(field)?;
                            if field.entries().is_empty() || field.entries().len() > 8 {
                                return Err(fail(field));
                            }
                            t.family = field
                                .entries()
                                .iter()
                                .map(|e| {
                                    let s = e.value().as_string().ok_or_else(|| fail(field))?;
                                    if s.trim().is_empty() || s.len() > 128 || s.contains('\0') {
                                        return Err(fail(field));
                                    }
                                    Ok(s.to_owned())
                                })
                                .collect::<Result<_, Diagnostic>>()?;
                        }
                        "body-size" | "label-size" | "title-size" => {
                            let size = dimension(field, 256.0)?;
                            if size == Pixels::ZERO {
                                return Err(fail(field));
                            }
                            match field.name().value() {
                                "body-size" => t.body_size = size,
                                "label-size" => t.label_size = size,
                                _ => t.title_size = size,
                            }
                        }
                        _ => return Err(fail(field)),
                    }
                }
            }
            "ui" => {
                let ui = block(node)?;
                unique(ui)?;
                // Group defaults apply before kind overrides regardless of source order.
                for group in ["surface", "control"] {
                    if let Some(node) = ui.get(group) {
                        let style = parse_style(block(node)?, None)?;
                        for kind in UiKind::ALL {
                            if kind.is_control() == (group == "control") {
                                std::sync::Arc::make_mut(&mut appearance.customization).ui
                                    [kind as usize]
                                    .overlay(style);
                            }
                        }
                    }
                }
                for node in ui.nodes() {
                    if matches!(node.name().value(), "surface" | "control") {
                        continue;
                    }
                    let kind = UiKind::parse(node.name().value()).ok_or_else(|| fail(node))?;
                    let style = ui_style(node, kind)?;
                    std::sync::Arc::make_mut(&mut appearance.customization).ui[kind as usize]
                        .overlay(style);
                }
            }
            _ => return Err(fail(node)),
        }
    }
    Ok(())
}
