use super::eligible_outputs::EligibleOutputState;
use super::surfaces::BarSurfaceIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PlannedBarSurface {
    pub(super) identity: BarSurfaceIdentity,
    pub(super) bar_index: usize,
}

pub(super) fn plan_bar_surfaces(
    bars: &[crate::config::BarConfig],
    eligible_outputs: &EligibleOutputState,
) -> Vec<PlannedBarSurface> {
    match eligible_outputs {
        EligibleOutputState::Unknown => bars
            .iter()
            .enumerate()
            .filter(|(_, bar)| bar.targets_all_outputs())
            .map(|(bar_index, bar)| PlannedBarSurface {
                identity: BarSurfaceIdentity::new(bar.name.clone(), None),
                bar_index,
            })
            .collect(),
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
                        .map(|(bar_index, bar)| PlannedBarSurface {
                            identity: BarSurfaceIdentity::new(
                                bar.name.clone(),
                                Some(output_name.clone()),
                            ),
                            bar_index,
                        }),
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

    #[test]
    fn unknown_outputs_plan_default_and_explicit_all_target_bars_without_output_identity() {
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

        assert_eq!(
            planned,
            vec![
                PlannedBarSurface {
                    identity: BarSurfaceIdentity::new("top", None),
                    bar_index: 0,
                },
                PlannedBarSurface {
                    identity: BarSurfaceIdentity::new("left", None),
                    bar_index: 2,
                },
            ]
        );
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
                PlannedBarSurface {
                    identity: BarSurfaceIdentity::new("top", Some("DP-1".into())),
                    bar_index: 1,
                },
                PlannedBarSurface {
                    identity: BarSurfaceIdentity::new("left", Some("DP-1".into())),
                    bar_index: 0,
                },
                PlannedBarSurface {
                    identity: BarSurfaceIdentity::new("top", Some("HDMI-A-1".into())),
                    bar_index: 1,
                },
                PlannedBarSurface {
                    identity: BarSurfaceIdentity::new("other", Some("HDMI-A-1".into())),
                    bar_index: 2,
                },
                PlannedBarSurface {
                    identity: BarSurfaceIdentity::new("left", Some("HDMI-A-1".into())),
                    bar_index: 0,
                },
            ]
        );
    }
}
