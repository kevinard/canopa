use super::*;

#[test]
fn capture_controls_accept_only_the_selected_input_classes_and_name_them_truthfully() {
    for (devices, label) in [
        (1, "Keyboard input · Screen edges"),
        (2, "Pointer input · Screen edges"),
        (3, "Keyboard and pointer input · Screen edges"),
    ] {
        for clipboard in [false, true] {
            let input = Input::for_capture("app", devices, clipboard).unwrap();
            assert!(input.capture_input && input.sources.is_empty());
            assert_eq!(input.devices, devices);
            assert_eq!(input.clipboard, clipboard);
            assert_eq!(input.control_label(), label);
            input.validate().unwrap();
        }
    }
    for devices in [0, 4, 5, 7, u32::MAX] {
        assert!(Input::for_capture("app", devices, false).is_err());
        assert!(Input::for_capture("app", devices, true).is_err());
    }
}
use std::path::PathBuf;

struct Helper {
    _root: tempfile::TempDir,
    process: DialogProcess,
    pid_file: PathBuf,
}
impl Helper {
    fn new(mode: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        let pid_file = root.path().join("pid");
        let script = r#"
import os,sys,time
from pathlib import Path
sys.stdin.buffer.readline()
Path(sys.argv[1]).write_text(str(os.getpid()))
mode=sys.argv[2]
if mode=='pending':
    time.sleep(30)
elif mode=='bad':
    print('{"ready":true,"unexpected":true}',flush=True)
    sys.stdin.buffer.read()
else:
    print('{"ready":true}',flush=True)
    if mode=='stubborn':
        time.sleep(30)
    elif mode in ('exit','extra'):
        while not Path(sys.argv[1]+'.trigger').exists(): time.sleep(.01)
        if mode=='extra':
            print('unexpected',flush=True)
            sys.stdin.buffer.read()
    else:
        sys.stdin.buffer.read()
"#;
        Self {
            process: DialogProcess {
                executable: "python3".into(),
                arguments: vec![
                    "-c".into(),
                    script.into(),
                    pid_file.clone().into_os_string(),
                    mode.into(),
                ],
            },
            _root: root,
            pid_file,
        }
    }
    async fn pid(&self) -> u32 {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if let Ok(value) = std::fs::read_to_string(&self.pid_file)
                    && let Ok(pid) = value.parse()
                {
                    break pid;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap()
    }
    async fn retired(&self) {
        let pid = self.pid().await;
        tokio::time::timeout(Duration::from_secs(5), async {
            while std::path::Path::new(&format!("/proc/{pid}")).exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("sharing helper must be reaped");
    }
    fn trigger(&self) {
        std::fs::write(self.pid_file.with_extension("trigger"), "").unwrap();
    }
}
fn input() -> Input {
    Input::new("app", vec!["Window · Editor".into()]).unwrap()
}
async fn closed(mut signal: watch::Receiver<bool>) {
    tokio::time::timeout(Duration::from_secs(4), stopped_source(&mut signal))
        .await
        .unwrap();
}

#[tokio::test]
async fn completed_start_watch_does_not_own_active_controls_and_source_close_retires_them() {
    let helper = Helper::new("lease");
    let (source, ended) = watch::channel(false);
    let (request, cancelled) = watch::channel(false);
    let indicator = Indicator::start(&helper.process, input(), ended, cancelled)
        .await
        .unwrap();
    drop(request);
    let mut signal = indicator.closed();
    assert!(
        tokio::time::timeout(Duration::from_millis(50), stopped_source(&mut signal))
            .await
            .is_err()
    );
    source.send_replace(true);
    closed(indicator.closed()).await;
    indicator.stop().await.unwrap();
    helper.retired().await;
}

#[tokio::test]
async fn explicit_stop_and_drop_close_the_actual_helper_lease() {
    for explicit in [false, true] {
        let helper = Helper::new("lease");
        let (_source, ended) = watch::channel(false);
        let (_request, cancelled) = watch::channel(false);
        let indicator = Indicator::start(&helper.process, input(), ended, cancelled)
            .await
            .unwrap();
        if explicit {
            indicator.stop().await.unwrap();
        } else {
            drop(indicator);
        }
        helper.retired().await;
    }
}

#[tokio::test]
async fn helper_exit_or_unexpected_output_revokes_ready_controls() {
    for mode in ["exit", "extra"] {
        let helper = Helper::new(mode);
        let (_source, ended) = watch::channel(false);
        let (_request, cancelled) = watch::channel(false);
        let indicator = Indicator::start(&helper.process, input(), ended, cancelled)
            .await
            .unwrap();
        helper.pid().await;
        helper.trigger();
        closed(indicator.closed()).await;
        indicator.stop().await.unwrap();
        helper.retired().await;
    }
}

#[tokio::test]
async fn cancellation_during_readiness_and_malformed_ready_never_grant_controls() {
    let helper = Helper::new("pending");
    let (_source, ended) = watch::channel(false);
    let (request, cancelled) = watch::channel(false);
    let opening = Indicator::start(&helper.process, input(), ended, cancelled);
    tokio::pin!(opening);
    tokio::select! {
        _ = helper.pid() => {},
        _ = &mut opening => panic!("helper is not ready"),
    }
    request.send_replace(true);
    assert!(opening.await.is_err());
    helper.retired().await;
    let helper = Helper::new("bad");
    let (_source, ended) = watch::channel(false);
    let (_request, cancelled) = watch::channel(false);
    assert!(
        Indicator::start(&helper.process, input(), ended, cancelled)
            .await
            .is_err()
    );
    helper.retired().await;
}

#[tokio::test]
async fn noncooperative_helper_is_killed_and_reaped_after_stop_grace() {
    let helper = Helper::new("stubborn");
    let (_source, ended) = watch::channel(false);
    let (_request, cancelled) = watch::channel(false);
    let indicator = Indicator::start(&helper.process, input(), ended, cancelled)
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), indicator.stop())
        .await
        .unwrap()
        .unwrap();
    helper.retired().await;
}

#[test]
fn source_labels_are_bounded_and_invalid_private_inputs_are_rejected() {
    let valid = Input::new("app\nname", vec!["x".repeat(1000)]).unwrap();
    assert_eq!(valid.requester, "appname");
    assert_eq!(valid.sources[0].len(), 120);
    assert!(Input::new("app", vec![]).is_err());
    assert!(Input::new("app", vec!["source".into(); 17]).is_err());
    assert!(
        Input {
            requester: "app".into(),
            sources: vec!["bad\nlabel".into()],
            devices: 0,
            clipboard: false,
            capture_input: false,
        }
        .validate()
        .is_err()
    );
}

#[test]
fn controls_describe_the_actual_device_and_source_grant() {
    let keyboard = Input::for_remote("app", vec![], 1).unwrap();
    assert_eq!(keyboard.heading(), "Keyboard control is active");
    assert_eq!(keyboard.stop_label(), "Stop control");
    let combined = Input::for_remote("app", vec!["Window · Editor".into()], 1).unwrap();
    assert_eq!(combined.heading(), "Sharing and control are active");
    assert_eq!(combined.stop_label(), "Stop sharing and control");
    assert!(Input::for_remote("app", vec![], 0).is_err());
    assert_eq!(
        Input::for_remote("app", vec![], 2).unwrap().heading(),
        "Pointer control is active"
    );
    assert_eq!(
        Input::for_remote("app", vec![], 3).unwrap().heading(),
        "Keyboard and pointer control are active"
    );
    assert!(Input::for_remote("app", vec![], 4).is_err());
    let old: Input =
        serde_json::from_str(r#"{"requester":"app","sources":["Window · Editor"]}"#).unwrap();
    old.validate().unwrap();
    assert_eq!(old.devices, 0);
    assert_eq!(old.title(), "Screen sharing");
}

#[test]
fn clipboard_controls_describe_the_actual_independent_grant() {
    let clipboard = Input::for_remote_clipboard("app", vec![], 0, true).unwrap();
    assert_eq!(clipboard.title(), "Clipboard sharing");
    assert_eq!(clipboard.heading(), "Clipboard sharing is active");
    assert_eq!(clipboard.stop_label(), "Stop sharing");
    let mixed = Input::for_remote_clipboard("app", vec![], 1, true).unwrap();
    assert_eq!(mixed.title(), "Desktop sharing");
    assert_eq!(mixed.stop_label(), "Stop all sharing");
}

#[test]
fn touch_controls_name_the_grant_and_require_shared_sources() {
    let touch = Input::for_remote("app", vec!["Window · Editor".into()], 4).unwrap();
    assert_eq!(
        touch.control_label(),
        "Touch control · Shared screen or window"
    );
    assert!(Input::for_remote("app", vec![], 4).is_err());
    assert!(Input::for_capture("app", 4, false).is_err());
    assert!(Input::for_remote("app", vec!["Window · Editor".into()], 8).is_err());
}
