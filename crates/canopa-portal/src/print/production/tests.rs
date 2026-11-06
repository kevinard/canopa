//! Controlled native fixture supplies an approval without running Iced. This
//! exercises the real adapter and helpers, not public/frontend consent.
use super::*;
use crate::print::{
    Format, Layout, PageRange, PageSetup, Pages,
    file_target::{Pinned, Reviewed},
    worker::{Destination, JobOptions, fixture_document},
};
use crate::print_ui::Grant;
use serde::Deserialize;
use std::{
    fs,
    fs::File,
    io::{Seek, SeekFrom},
    os::unix::{fs::PermissionsExt, net::UnixStream},
    path::PathBuf,
};

fn production(executable: PathBuf, recovery: &std::path::Path) -> Production {
    Production::new(
        Arc::new(DialogProcess {
            executable,
            arguments: vec![],
        }),
        crate::print::file_target::RecoveryStore::new(&recovery.join("print")).unwrap(),
    )
}

#[tokio::test]
async fn inconsistent_copy_approval_fails_before_starting_a_helper() {
    let recovery = tempfile::tempdir().unwrap();
    let backend = production("/fixture-only/not-executable".into(), recovery.path());
    let (_cancel, cancelled) = watch::channel(false);
    let result = backend
        .deliver(
            Delivery {
                approval: presentation::Approval {
                    grant: Grant {
                        target: Target::Printer(Destination {
                            queue: "Office".into(),
                            instance: None,
                        }),
                        layout: Layout {
                            copies: 2,
                            ..Default::default()
                        },
                        page_setup: PageSetup::default(),
                        device: JobOptions::default(),
                    },
                    file: None,
                    authentication: None,
                },
                document: fixture_document(b"%PDF-1.7\nfixture"),
                rendering: Rendering::Prepared,
                title: "Fixture".into(),
            },
            cancelled,
        )
        .await;
    assert!(result.unwrap_err().to_string().contains("copy count"));
    backend.shutdown().await;
}

#[tokio::test]
async fn prepared_printer_formats_normalize_one_copy_without_reapplying_layout() {
    for (bytes, expected) in [
        (b"%!PS\nprepared page".as_slice(), "render"),
        (
            b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>".as_slice(),
            "render",
        ),
        (b"%PDF-1.7\nprepared page".as_slice(), "render"),
    ] {
        let fixture = tempfile::tempdir().unwrap();
        let executable = fixture.path().join("capture-worker");
        let captured = fixture.path().join("operation.json");
        fs::write(
            &executable,
            r#"#!/bin/sh
IFS= read -r packet
if [ ! -e "$1" ]; then printf '%s\n' "$packet" > "$1"; fi
printf '%s\n' '{"event":"failed","message":"stopped before rendering or printer I/O"}'
"#,
        )
        .unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let backend = Production::new(
            Arc::new(DialogProcess {
                executable,
                arguments: vec![captured.as_os_str().to_owned()],
            }),
            crate::print::file_target::RecoveryStore::new(&fixture.path().join("recovery"))
                .unwrap(),
        );
        let (_cancel, cancelled) = watch::channel(false);
        let result = backend
            .deliver(
                Delivery {
                    approval: presentation::Approval {
                        grant: Grant {
                            target: Target::Printer(Destination {
                                queue: "Office".into(),
                                instance: None,
                            }),
                            layout: Layout {
                                reverse: true,
                                scale: 75.,
                                number_up: 4,
                                pages: Pages::Ranges,
                                ranges: vec![PageRange { first: 1, last: 2 }],
                                ..Default::default()
                            },
                            page_setup: PageSetup {
                                width: Some(210.),
                                height: Some(297.),
                                ..Default::default()
                            },
                            device: JobOptions {
                                sides: Some(crate::print::worker::Sides::LongEdge),
                                ..Default::default()
                            },
                        },
                        file: None,
                        authentication: None,
                    },
                    document: fixture_document(bytes),
                    rendering: Rendering::Prepared,
                    title: "Already formatted".into(),
                },
                cancelled,
            )
            .await;
        assert!(result.is_err(), "the probe never performs printer I/O");
        backend.shutdown().await;
        let packet: serde_json::Value =
            serde_json::from_slice(&fs::read(captured).unwrap()).unwrap();
        assert_eq!(packet["operation"]["operation"], expected);
        if expected == "render" {
            let request: render::Request =
                serde_json::from_value(packet["operation"]["request"].clone()).unwrap();
            assert_eq!(request.mode, render::Mode::Copies);
            assert_eq!(request.format, Format::Pdf);
            assert_eq!(request.layout, Layout::default());
            assert_eq!(request.page_setup, PageSetup::default());
            assert!(request.duplex);
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    input: PathBuf,
    #[serde(default)]
    input_offset: u64,
    output: PathBuf,
    render: render::Request,
    prepared: bool,
    expected_error: bool,
}

#[tokio::test]
#[ignore = "requires the normal backend executable and a private native fixture"]
async fn native_file_delivery() {
    let recovery = tempfile::tempdir().unwrap();
    let backend = production(
        std::env::var_os("CANOPA_PRINT_BINARY").unwrap().into(),
        recovery.path(),
    );
    let cases: Vec<Case> = serde_json::from_slice(
        &fs::read(std::env::var_os("CANOPA_PRINT_RENDER_CASES").unwrap()).unwrap(),
    )
    .unwrap();
    let (_cancel, cancelled) = watch::channel(false);
    for case in cases {
        let original = fs::read(&case.output).ok();
        let review = Reviewed::open(&case.output).unwrap();
        let selection = review.selection().clone();
        let (send, receive) = UnixStream::pair().unwrap();
        review.send(&send).unwrap();
        drop(send);
        let pin = Pinned::receive(&receive, selection.clone()).unwrap();
        drop(review);
        let mut source = File::open(&case.input).unwrap();
        source.seek(SeekFrom::Start(case.input_offset)).unwrap();
        let document = backend
            .snapshot(source.try_clone().unwrap().into(), cancelled.clone())
            .await
            .unwrap()
            .unwrap();
        let result = backend
            .deliver(
                Delivery {
                    approval: presentation::Approval {
                        grant: Grant {
                            target: Target::File {
                                path: selection.path().to_vec(),
                                format: case.render.format,
                                selection,
                            },
                            device: JobOptions {
                                copies: case.render.layout.copies,
                                ..Default::default()
                            },
                            layout: case.render.layout,
                            page_setup: case.render.page_setup,
                        },
                        file: Some(pin),
                        authentication: None,
                    },
                    document,
                    rendering: if case.prepared {
                        Rendering::Prepared
                    } else {
                        Rendering::Direct
                    },
                    title: "Native renderer fixture".into(),
                },
                cancelled.clone(),
            )
            .await;
        assert_eq!(
            source.stream_position().unwrap(),
            case.input_offset,
            "source cursor changed"
        );
        if case.expected_error {
            assert!(
                result.is_err(),
                "unexpected success for {}",
                case.output.display()
            );
            assert_eq!(fs::read(&case.output).ok(), original);
        } else {
            assert!(
                matches!(result, Ok(Some(()))),
                "{}: {result:?}",
                case.output.display()
            );
            let bytes = fs::read(&case.output).unwrap();
            assert!(!bytes.is_empty());
            if case.prepared {
                assert_eq!(
                    bytes,
                    fs::read(&case.input).unwrap()[case.input_offset as usize..],
                    "prepared body was rendered twice"
                );
            }
        }
    }
    backend.shutdown().await;
}
