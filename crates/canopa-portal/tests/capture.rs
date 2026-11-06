//! Native transport fixture only. The script supplies a disposable compositor;
//! ordinary integration checks never capture the user's desktop.
use canopa_portal::capture::Captures;
use std::io::{BufRead, Write};

fn report(
    captured: &canopa_portal::capture::Captured,
    directory: &std::path::Path,
    sequence: usize,
) {
    let frames: Vec<_> = captured.outputs().iter().enumerate().map(|(index, frame)| {
        let name = format!("capture-{sequence}-{index}.png");
        frame.pixels.save(directory.join(&name)).unwrap();
        serde_json::json!({"file":name, "name":frame.name, "logical":[frame.logical.x, frame.logical.y, frame.logical.width, frame.logical.height], "size":[frame.pixels.width(),frame.pixels.height()]})
    }).collect();
    println!("CAPTURE {}", serde_json::json!({"frames":frames}));
}

#[tokio::test]
#[ignore = "requires scripts/test-screenshot-capture.py and its private compositor"]
async fn native_capture_fixture() {
    assert_eq!(
        std::env::var("CANOPA_CAPTURE_NATIVE_FIXTURE").as_deref(),
        Ok("1")
    );
    let directory = std::path::PathBuf::from(
        std::env::var_os("CANOPA_CAPTURE_FIXTURE_OUTPUT").expect("private fixture output"),
    );
    assert!(directory.is_absolute() && directory.is_dir());
    let captures = Captures::from_environment();
    let (_sender, cancelled) = tokio::sync::watch::channel(false);
    let mut held = Vec::new();
    for (sequence, line) in std::io::stdin().lock().lines().enumerate() {
        let command: serde_json::Value = serde_json::from_str(&line.unwrap()).unwrap();
        if command["command"] == "stop" {
            break;
        }
        if command["command"] == "release" {
            held.clear();
            println!("CAPTURE {{\"released\":true}}");
        } else if command["command"] == "held" {
            report(&held[0], &directory, sequence);
        } else {
            match captures.capture(cancelled.clone()).await {
                Ok(captured) => {
                    report(&captured, &directory, sequence);
                    if command["hold"] == true {
                        held.push(captured);
                    }
                }
                Err(error) => {
                    println!("CAPTURE {}", serde_json::json!({"error":error.to_string()}))
                }
            }
        }
        std::io::stdout().flush().unwrap();
    }
}
