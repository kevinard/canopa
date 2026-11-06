use super::*;

#[test]
#[ignore = "requires real converter namespaces and the native process observer"]
fn native_namespace_descendants() {
    let mode = std::env::var("CANOPA_PRINT_DESCENDANT_MODE").unwrap();
    assert!(matches!(mode.as_str(), "exit" | "cancel" | "owner-loss"));
    let target = stage().unwrap();
    let source = stage().unwrap();
    let (read, write) = rustix::pipe::pipe().unwrap();
    let mut writer = Some(write);
    let cancel = Cancellation::fixture(read.into());
    let code = r#"
import os, time
ready, notify=os.pipe()
child=os.fork()
if child==0:
    os.setsid()
    grandchild=os.fork()
    if grandchild==0:
        while True: time.sleep(1)
    os.write(notify,b'r')
    while True: time.sleep(1)
os.read(ready,1)
os.write(1,b'ready\n')
if os.environ['MODE']=='exit': time.sleep(.25)
else:
    while True: time.sleep(1)
"#;
    let mut command = sandbox();
    command.args([
        "--setenv",
        "MODE",
        &mode,
        "--",
        "/usr/bin/python3",
        "-c",
        code,
    ]);
    let cancellation = if mode == "cancel" {
        let target = target.try_clone().unwrap();
        let write = writer.take().unwrap();
        Some(std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            while target.metadata().unwrap().len() == 0 {
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(10));
            }
            std::thread::sleep(Duration::from_millis(250));
            drop(write);
        }))
    } else {
        None
    };
    let outcome = run(command, &source, &target, &cancel, &[]);
    if let Some(cancellation) = cancellation {
        cancellation.join().unwrap();
    }
    assert_eq!(outcome.is_err(), mode == "cancel", "{outcome:?}");
    assert_eq!(target.metadata().unwrap().len(), 6);
    drop(writer);
}
