//! Toolkit-independent lock layout planning. Authentication wins space conflicts.
use anyhow::{Result, bail, ensure};
use std::collections::HashSet;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}
impl Anchor {
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "top-left" => Self::TopLeft,
            "top-center" => Self::TopCenter,
            "top-right" => Self::TopRight,
            "center-left" => Self::CenterLeft,
            "center" => Self::Center,
            "center-right" => Self::CenterRight,
            "bottom-left" => Self::BottomLeft,
            "bottom-center" => Self::BottomCenter,
            "bottom-right" => Self::BottomRight,
            _ => return None,
        })
    }
    fn axes(self) -> (u8, u8) {
        match self {
            Self::TopLeft => (0, 0),
            Self::TopCenter => (1, 0),
            Self::TopRight => (2, 0),
            Self::CenterLeft => (0, 1),
            Self::Center => (1, 1),
            Self::CenterRight => (2, 1),
            Self::BottomLeft => (0, 2),
            Self::BottomCenter => (1, 2),
            Self::BottomRight => (2, 2),
        }
    }
}
#[derive(Clone, Debug)]
pub struct Region {
    pub anchor: Anchor,
    pub widgets: Vec<String>,
    pub row: bool,
    pub x: f32,
    pub y: f32,
}
#[derive(Clone, Debug)]
pub struct Layout {
    pub regions: Vec<Region>,
    pub margin: f32,
    pub gap: f32,
}
impl Default for Layout {
    fn default() -> Self {
        Self {
            margin: 32.,
            gap: 12.,
            regions: vec![
                Region {
                    anchor: Anchor::Center,
                    widgets: ["avatar", "clock", "date", "username", "authentication"]
                        .map(String::from)
                        .into(),
                    row: false,
                    x: 0.,
                    y: 0.,
                },
                Region {
                    anchor: Anchor::BottomCenter,
                    widgets: ["battery", "media", "keyboard", "weather"]
                        .map(String::from)
                        .into(),
                    row: false,
                    x: 0.,
                    y: 0.,
                },
            ],
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
impl Rect {
    fn intersects(self, other: Self) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }
}
#[derive(Clone, Debug)]
pub struct Item {
    pub name: String,
    pub width: f32,
    pub height: f32,
}
#[derive(Clone, Debug)]
pub struct Placement {
    pub name: String,
    pub bounds: Rect,
    pub scale: f32,
}
impl Layout {
    pub fn parse(document: &kdl::KdlDocument) -> Result<Self> {
        let mut layout = Self {
            regions: Vec::new(),
            ..Default::default()
        };
        let mut seen = HashSet::new();
        for node in document.nodes() {
            ensure!(
                node.ty().is_none()
                    && node.children().is_none()
                    && seen.insert(node.name().value()),
                "invalid or duplicate layout field"
            );
            let name = node.name().value();
            if name == "margin" || name == "gap" {
                ensure!(
                    node.entries().len() == 1,
                    "layout dimension requires one number"
                );
                let entry = &node.entries()[0];
                ensure!(
                    entry.ty().is_none() && entry.name().is_none(),
                    "invalid layout dimension"
                );
                let value = entry
                    .value()
                    .as_integer()
                    .filter(|n| (0..=256).contains(n))
                    .ok_or_else(|| anyhow::anyhow!("layout dimension must be 0..256"))?
                    as f32;
                if name == "margin" {
                    layout.margin = value;
                } else {
                    layout.gap = value;
                }
                continue;
            }
            let anchor =
                Anchor::parse(name).ok_or_else(|| anyhow::anyhow!("unknown layout region"))?;
            let mut region = Region {
                anchor,
                widgets: Vec::new(),
                row: false,
                x: 0.,
                y: 0.,
            };
            let mut props = HashSet::new();
            for entry in node.entries() {
                ensure!(entry.ty().is_none(), "annotated layout entry");
                if let Some(property) = entry.name() {
                    ensure!(props.insert(property.value()), "duplicate layout property");
                    match property.value() {
                        "flow" => {
                            region.row = match entry.value().as_string() {
                                Some("row") => true,
                                Some("column") => false,
                                _ => bail!("layout flow must be row or column"),
                            }
                        }
                        "x" | "y" => {
                            let value = entry
                                .value()
                                .as_integer()
                                .filter(|n| (-4096..=4096).contains(n))
                                .ok_or_else(|| {
                                    anyhow::anyhow!("layout offset must be -4096..4096")
                                })? as f32;
                            if property.value() == "x" {
                                region.x = value;
                            } else {
                                region.y = value;
                            }
                        }
                        _ => bail!("unknown layout property"),
                    }
                } else {
                    region.widgets.push(
                        entry
                            .value()
                            .as_string()
                            .ok_or_else(|| anyhow::anyhow!("expected layout widget name"))?
                            .to_owned(),
                    );
                }
            }
            layout.regions.push(region);
        }
        Ok(layout)
    }
    pub fn validate(
        &self,
        enabled: &[String],
        custom: &std::collections::BTreeMap<String, canopa_widgets::script::Script>,
    ) -> Result<()> {
        let mut seen = HashSet::new();
        for name in self.regions.iter().flat_map(|r| &r.widgets) {
            ensure!(
                [
                    "clock",
                    "date",
                    "authentication",
                    "username",
                    "avatar",
                    "keyboard",
                    "battery",
                    "media",
                    "weather"
                ]
                .contains(&name.as_str())
                    || custom.contains_key(name),
                "unknown layout widget"
            );
            ensure!(
                seen.insert(name.as_str()),
                "widget appears more than once in layout"
            );
        }
        ensure!(
            seen.contains("authentication"),
            "layout must contain authentication"
        );
        for widget in enabled {
            ensure!(
                seen.contains(widget.as_str()),
                "enabled widget has no layout placement"
            );
        }
        Ok(())
    }
    pub fn plan(&self, width: f32, height: f32, scale: f32, items: &[Item]) -> Vec<Placement> {
        let margin = (self.margin * scale).min(width / 8.).min(height / 8.);
        let available_w = (width - 2. * margin).max(1.);
        let available_h = (height - 2. * margin).max(1.);
        let mut placements = Vec::new();
        for region in &self.regions {
            let items: Vec<_> = region
                .widgets
                .iter()
                .filter_map(|n| items.iter().find(|i| &i.name == n))
                .collect();
            if items.is_empty() {
                continue;
            }
            let gap = self.gap * scale;
            let (mut w, mut h) = if region.row {
                (
                    items.iter().map(|i| i.width).sum::<f32>() + gap * (items.len() - 1) as f32,
                    items.iter().map(|i| i.height).fold(0., f32::max),
                )
            } else {
                (
                    items.iter().map(|i| i.width).fold(0., f32::max),
                    items.iter().map(|i| i.height).sum::<f32>() + gap * (items.len() - 1) as f32,
                )
            };
            let fit = (available_w / w.max(1.))
                .min(available_h / h.max(1.))
                .min(1.);
            w *= fit;
            h *= fit;
            let (ax, ay) = region.anchor.axes();
            let x = (match ax {
                0 => margin,
                1 => (width - w) / 2.,
                _ => width - margin - w,
            } + region.x * scale)
                .clamp(margin, (width - margin - w).max(margin));
            let y = (match ay {
                0 => margin,
                1 => (height - h) / 2.,
                _ => height - margin - h,
            } + region.y * scale)
                .clamp(margin, (height - margin - h).max(margin));
            let mut cursor = 0.;
            for item in items {
                let iw = item.width * fit;
                let ih = item.height * fit;
                let (ix, iy) = if region.row {
                    (x + cursor, y + (h - ih) / 2.)
                } else {
                    (
                        x + match ax {
                            0 => 0.,
                            1 => (w - iw) / 2.,
                            _ => w - iw,
                        },
                        y + cursor,
                    )
                };
                placements.push(Placement {
                    name: item.name.clone(),
                    bounds: Rect {
                        x: ix,
                        y: iy,
                        width: iw,
                        height: ih,
                    },
                    scale: fit,
                });
                cursor += if region.row {
                    iw + gap * fit
                } else {
                    ih + gap * fit
                };
            }
        }
        if let Some(authentication) = placements
            .iter()
            .find(|p| p.name == "authentication")
            .map(|p| p.bounds)
        {
            placements
                .retain(|p| p.name == "authentication" || !p.bounds.intersects(authentication));
        }
        placements
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordering_alignment_and_authentication_bounds_hold_at_small_and_large_sizes() {
        let document:kdl::KdlDocument = "top-left \"clock\" \"date\"\ncenter \"authentication\"\nbottom-right \"battery\" \"keyboard\" flow=\"row\"\n".parse().unwrap();
        let layout = Layout::parse(&document).unwrap();
        let items = vec![
            Item {
                name: "clock".into(),
                width: 180.,
                height: 80.,
            },
            Item {
                name: "date".into(),
                width: 200.,
                height: 24.,
            },
            Item {
                name: "authentication".into(),
                width: 320.,
                height: 170.,
            },
            Item {
                name: "battery".into(),
                width: 70.,
                height: 24.,
            },
            Item {
                name: "keyboard".into(),
                width: 100.,
                height: 24.,
            },
        ];
        for (w, h) in [(320., 240.), (640., 480.), (1920., 1080.), (3840., 2160.)] {
            let plan = layout.plan(w, h, 1., &items);
            let auth = plan.iter().find(|p| p.name == "authentication").unwrap();
            assert!(
                auth.bounds.x >= 0.
                    && auth.bounds.y >= 0.
                    && auth.bounds.x + auth.bounds.width <= w
                    && auth.bounds.y + auth.bounds.height <= h
            );
            assert!(
                plan.iter()
                    .filter(|p| p.name != "authentication")
                    .all(|p| !p.bounds.intersects(auth.bounds))
            );
            if w >= 640. {
                let clock = plan.iter().find(|p| p.name == "clock").unwrap();
                let date = plan.iter().find(|p| p.name == "date").unwrap();
                assert!(date.bounds.y > clock.bounds.y);
            }
        }
    }
}
