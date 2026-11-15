//! Unit tests for InputCapture transport negotiation and bootstrap validation.
use super::*;

#[test]
fn provider_negotiation_keeps_v1_pointer_only_and_declines_unknown_versions_or_touch_only() {
    assert_eq!(provider_capabilities(1, 3).unwrap(), 2);
    assert_eq!(provider_capabilities(2, 7).unwrap(), 3);
    assert_eq!(provider_capabilities(2, 1).unwrap(), 1);
    for (version, mask) in [(0, 3), (3, 3), (1, 1), (2, 4), (2, 0)] {
        assert!(provider_capabilities(version, mask).is_err());
    }
}

#[test]
fn worker_bootstrap_requires_nonempty_approved_input_classes() {
    let mut input = Input {
        compositor: ":1.1".into(),
        compositor_pid: 1,
        ipc_socket: "/tmp/capture-test".into(),
        backend: ":1.2".into(),
        caller: ":1.3".into(),
        grants: 2,
    };
    for grants in [1, 2, 3] {
        input.grants = grants;
        input.validate().unwrap();
    }
    for grants in [0, 4, 7, u32::MAX] {
        input.grants = grants;
        assert!(input.validate().is_err());
    }
}
