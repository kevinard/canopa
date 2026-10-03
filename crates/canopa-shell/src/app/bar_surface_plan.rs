use super::eligible_outputs::EligibleOutputState;
use super::surfaces::BarSurfaceIdentity;
use crate::config::BarConfig;
use iced_layershell::reexport::{Anchor, Layer, NewLayerShellSettings, OutputOption};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PlannedBarSurface {
    pub(super) identity: BarSurfaceIdentity,
    pub(super) physical: BarSurfacePhysicalPlan,
}

impl PlannedBarSurface {
    pub(super) fn from_bar(bar: &BarConfig, output_name: &str) -> Self {
        Self {
            identity: BarSurfaceIdentity::new(bar.name.clone(), Some(output_name.to_string())),
            physical: BarSurfacePhysicalPlan::from_bar(bar),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BarSurfacePhysicalPlan {
    anchor: Anchor,
    layer: Layer,
    exclusive_zone: i32,
    size: (u32, u32),
    margin: (i32, i32, i32, i32),
}

impl BarSurfacePhysicalPlan {
    pub(super) fn from_bar(bar: &BarConfig) -> Self {
        let size = crate::surface::bar_geometry::BarGeometry::host_size(bar, 0);

        Self {
            anchor: bar.edge.anchor(),
            layer: bar.layer.layer_shell(),
            exclusive_zone: crate::surface::bar_geometry::BarGeometry::reservation(bar),
            size,
            margin: (0, 0, 0, 0),
        }
    }

    pub(super) fn layer_shell_settings(
        &self,
        output_name: &str,
        namespace: String,
    ) -> NewLayerShellSettings {
        NewLayerShellSettings {
            anchor: self.anchor,
            layer: self.layer,
            exclusive_zone: Some(self.exclusive_zone),
            size: Some(self.size),
            margin: Some(self.margin),
            output_option: OutputOption::OutputName(output_name.to_string()),
            namespace: Some(namespace),
            ..Default::default()
        }
    }
}

pub(super) fn plan_bar_surfaces(
    bars: &[BarConfig],
    eligible_outputs: &EligibleOutputState,
) -> Vec<PlannedBarSurface> {
    match eligible_outputs {
        EligibleOutputState::Unknown => Vec::new(),
        EligibleOutputState::Known(outputs) => {
            let mut planned = Vec::new();
            for output_name in outputs {
                let mut matches: Vec<_> = bars
                    .iter()
                    .enumerate()
                    .filter(|(_, bar)| bar.matches_output(output_name))
                    .collect();
                matches.sort_by_key(|(index, bar)| (!bar.edge.is_horizontal(), *index));
                planned.extend(
                    matches
                        .into_iter()
                        .map(|(_, bar)| PlannedBarSurface::from_bar(bar, output_name)),
                );
            }
            planned
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BarConfig, Edge, OutputTarget};

    fn bar(name: &str, edge: Edge, output_target: OutputTarget) -> BarConfig {
        BarConfig {
            name: name.into(),
            edge,
            output_target,
            ..BarConfig::default()
        }
    }

    fn expected_surface(bar: &BarConfig, output_name: &str) -> PlannedBarSurface {
        PlannedBarSurface::from_bar(bar, output_name)
    }

    #[test]
    fn unknown_outputs_plan_no_bar_surfaces() {
        let bars = vec![
            BarConfig {
                name: "top".into(),
                edge: Edge::Top,
                ..BarConfig::default()
            },
            bar(
                "laptop",
                Edge::Bottom,
                OutputTarget::Include(vec!["eDP-1".into()]),
            ),
            bar("left", Edge::Left, OutputTarget::All),
        ];

        let planned = plan_bar_surfaces(&bars, &EligibleOutputState::Unknown);

        assert!(planned.is_empty());
    }

    #[test]
    fn known_outputs_plan_matching_bars_with_horizontal_first_ordering_per_output() {
        let bars = vec![
            bar("left", Edge::Left, OutputTarget::All),
            bar("top", Edge::Top, OutputTarget::All),
            bar(
                "other",
                Edge::Bottom,
                OutputTarget::Include(vec!["HDMI-A-1".into()]),
            ),
        ];

        let planned = plan_bar_surfaces(
            &bars,
            &EligibleOutputState::Known(vec!["DP-1".into(), "HDMI-A-1".into()]),
        );

        assert_eq!(
            planned,
            vec![
                expected_surface(&bars[1], "DP-1"),
                expected_surface(&bars[0], "DP-1"),
                expected_surface(&bars[1], "HDMI-A-1"),
                expected_surface(&bars[2], "HDMI-A-1"),
                expected_surface(&bars[0], "HDMI-A-1"),
            ]
        );
    }

    #[test]
    fn physical_plan_converts_bar_dimensions_to_layer_shell_settings() {
        let bar = BarConfig {
            name: "top".into(),
            edge: Edge::Top,
            layer: crate::config::BarLayer::Background,
            collapsed_width: 32,
            ..BarConfig::default()
        };

        let settings = PlannedBarSurface::from_bar(&bar, "eDP-1")
            .physical
            .layer_shell_settings("eDP-1", "canopa-shell".into());

        assert_eq!(
            settings.anchor,
            iced_layershell::reexport::Anchor::Top
                | iced_layershell::reexport::Anchor::Left
                | iced_layershell::reexport::Anchor::Right
        );
        assert_eq!(settings.layer, iced_layershell::reexport::Layer::Background);
        assert_eq!(settings.exclusive_zone, Some(32));
        assert_eq!(settings.size, Some((0, 32)));
        assert_eq!(settings.margin, Some((0, 0, 0, 0)));
        assert_eq!(
            settings.output_option,
            iced_layershell::reexport::OutputOption::OutputName("eDP-1".into())
        );
        assert_eq!(settings.namespace, Some("canopa-shell".into()));
    }

    #[test]
    fn horizontal_bar_surface_plan_does_not_inset_for_side_bars() {
        let bars = vec![
            BarConfig {
                name: "left".into(),
                edge: Edge::Left,
                collapsed_width: 32,
                ..BarConfig::default()
            },
            BarConfig {
                name: "right".into(),
                edge: Edge::Right,
                collapsed_width: 40,
                ..BarConfig::default()
            },
            BarConfig {
                name: "top".into(),
                edge: Edge::Top,
                collapsed_width: 24,
                ..BarConfig::default()
            },
        ];

        let planned = plan_bar_surfaces(&bars, &EligibleOutputState::Known(vec!["eDP-1".into()]));
        let top = planned
            .iter()
            .find(|surface| surface.identity.bar_name == "top")
            .expect("top Bar Surface should be planned");
        let settings = top
            .physical
            .layer_shell_settings("eDP-1", "canopa-shell".into());

        assert_eq!(settings.margin, Some((0, 0, 0, 0)));
    }

    #[test]
    fn side_bar_surface_plan_does_not_inset_for_horizontal_bars() {
        let bars = vec![
            BarConfig {
                name: "top".into(),
                edge: Edge::Top,
                collapsed_width: 24,
                ..BarConfig::default()
            },
            BarConfig {
                name: "bottom".into(),
                edge: Edge::Bottom,
                collapsed_width: 30,
                ..BarConfig::default()
            },
            BarConfig {
                name: "left".into(),
                edge: Edge::Left,
                collapsed_width: 32,
                ..BarConfig::default()
            },
        ];

        let planned = plan_bar_surfaces(&bars, &EligibleOutputState::Known(vec!["eDP-1".into()]));
        let left = planned
            .iter()
            .find(|surface| surface.identity.bar_name == "left")
            .expect("left Bar Surface should be planned");
        let settings = left
            .physical
            .layer_shell_settings("eDP-1", "canopa-shell".into());

        assert_eq!(settings.margin, Some((0, 0, 0, 0)));
    }
}
