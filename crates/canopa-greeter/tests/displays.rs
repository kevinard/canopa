use canopa_greeter::displays;

fn render(json: &str) -> anyhow::Result<String> {
    displays::render(&displays::parse(json.as_bytes())?)
}

#[test]
fn renders_only_generated_output_nodes() {
    let text = render(
        r#"{"outputs":[
            {"name":"eDP-1","mode":{"width":2560,"height":1600,"refresh":165.004},
             "scale":1.5,"transform":"90","position":{"x":-1280,"y":0}},
            {"name":"DP-2","mode":{"width":1920,"height":1080,"refresh":60,"custom":true}},
            {"name":"HDMI-A-1","off":true}]}"#,
    )
    .unwrap();
    assert_eq!(
        text,
        "// Written by Canopa Settings for the login screen; replaced on the next change.\n\
         output \"eDP-1\" {\n    mode \"2560x1600@165.004\"\n    scale 1.5\n    transform \"90\"\n    position x=-1280 y=0\n}\n\
         output \"DP-2\" {\n    mode \"1920x1080@60.000\" custom=true\n    scale 1\n    transform \"normal\"\n}\n\
         output \"HDMI-A-1\" {\n    off\n}\n"
    );
}

#[test]
fn refuses_anything_that_could_reach_the_compositor_as_configuration() {
    for request in [
        // Quoting out of the name would let a caller add `spawn-at-startup`.
        r#"{"outputs":[{"name":"X\" { } spawn-at-startup \"sh"}]}"#,
        r#"{"outputs":[{"name":"X\nbinds"}]}"#,
        r#"{"outputs":[{"name":""}]}"#,
        r#"{"outputs":[{"name":"DP-1","transform":"normal\" spawn-at-startup \"sh"}]}"#,
        r#"{"outputs":[{"name":"DP-1","extra":1}]}"#,
        r#"{"outputs":[{"name":"DP-1","scale":0}]}"#,
        r#"{"outputs":[{"name":"DP-1","mode":{"width":0,"height":1,"refresh":60}}]}"#,
        r#"{"outputs":[{"name":"DP-1","mode":{"width":1,"height":1,"refresh":1e9}}]}"#,
        r#"{"outputs":[{"name":"DP-1","position":{"x":2000000,"y":0}}]}"#,
        r#"{"outputs":[{"name":"DP-1"},{"name":"DP-1"}]}"#,
        r#"{"outputs":[{"name":"DP-1","off":true}]}"#,
        r#"{"outputs":[]}"#,
    ] {
        assert!(render(request).is_err(), "accepted {request}");
    }
    assert!(displays::parse(&vec![b' '; displays::MAX_INPUT as usize + 1]).is_err());
}

#[test]
fn apply_replaces_the_file_and_reset_removes_it() {
    let directory = tempfile::tempdir().unwrap();
    let target = directory.path().join("canopa/greeter-displays.kdl");
    let target_text = target.to_str().unwrap().to_owned();
    let run = |operation: &str, input: &str| {
        displays::main(
            &[operation.into(), "--target".into(), target_text.clone()],
            input.as_bytes(),
        )
    };
    run("apply", r#"{"outputs":[{"name":"DP-1"}]}"#).unwrap();
    assert!(
        std::fs::read_to_string(&target)
            .unwrap()
            .contains("output \"DP-1\"")
    );
    // A refused request keeps the previous file.
    assert!(run("apply", r#"{"outputs":[{"name":"bad name"}]}"#).is_err());
    assert!(
        std::fs::read_to_string(&target)
            .unwrap()
            .contains("output \"DP-1\"")
    );
    run("apply", r#"{"outputs":[{"name":"DP-2"}]}"#).unwrap();
    let text = std::fs::read_to_string(&target).unwrap();
    assert!(text.contains("DP-2") && !text.contains("DP-1"));
    assert_eq!(
        std::fs::read_dir(target.parent().unwrap()).unwrap().count(),
        1,
        "no temporary file left"
    );
    run("reset", "").unwrap();
    assert!(!target.exists());
    run("reset", "").unwrap();
    assert!(displays::main(&["apply".into(), "--bogus".into()], &b""[..]).is_err());
}
