use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EligibleOutputState {
    Unknown,
    Known(Vec<String>),
}

pub(crate) fn resolve_eligible_outputs(
    observed_outputs: Option<Vec<String>>,
    excluded_outputs: &[String],
) -> EligibleOutputState {
    let Some(observed_outputs) = observed_outputs else {
        return EligibleOutputState::Unknown;
    };

    let excluded: HashSet<&str> = excluded_outputs.iter().map(String::as_str).collect();
    let mut eligible_outputs: Vec<String> = observed_outputs
        .into_iter()
        .filter(|output| !output.is_empty())
        .filter(|output| !excluded.contains(output.as_str()))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    eligible_outputs.sort();

    EligibleOutputState::Known(eligible_outputs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_observation_is_unknown() {
        assert_eq!(
            resolve_eligible_outputs(None, &[]),
            EligibleOutputState::Unknown
        );
    }

    #[test]
    fn empty_observation_is_known_empty() {
        assert_eq!(
            resolve_eligible_outputs(Some(Vec::new()), &[]),
            EligibleOutputState::Known(Vec::new())
        );
    }

    #[test]
    fn resolver_drops_empty_names_deduplicates_filters_and_sorts() {
        assert_eq!(
            resolve_eligible_outputs(
                Some(vec![
                    "eDP-1".into(),
                    String::new(),
                    "DP-2".into(),
                    "HDMI-A-1".into(),
                    "DP-2".into(),
                ]),
                &["HDMI-A-1".into(), "missing".into()],
            ),
            EligibleOutputState::Known(vec!["DP-2".into(), "eDP-1".into()])
        );
    }
}
