//! A correct object generation cannot authorize an unrelated journal step.
use super::*;
#[derive(Serialize)]
struct RawPath(#[serde(with = "crate::path_serde")] PathBuf);

fn unchanged_after_tamper(tamper: &str) {
    for kind in ["copy", "move"] {
        let root = tempfile::Builder::new()
            .prefix("canopa-replacement-restart-")
            .tempdir()
            .unwrap();
        let output = tempfile::Builder::new()
            .prefix("canopa-replacement-output-")
            .tempdir()
            .unwrap();
        let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "operations::replacement_boundary::replacement_crash_helper",
                ])
                .env("CANOPA_REPLACEMENT_ROOT", root.path())
                .env("CANOPA_REPLACEMENT_OUTPUT", output.path())
                .env("CANOPA_REPLACEMENT_KIND", kind)
                .env("CANOPA_REPLACEMENT_SHAPE", "file")
                .env("CANOPA_REPLACEMENT_POINT", "publishing")
                .status()
                .unwrap();
            assert_eq!(status.signal(), Some(9));
            let marker: Marker =
                serde_json::from_slice(&fs::read(root.path().join("marker.json")).unwrap())
                    .unwrap();
            let target = output.path().join(leaf());
            let other = output.path().join("unreviewed-name");
            let previous = marker.directory.join("previous");
            let payload = marker.directory.join("folder");
            let payload_id = Identity::read(&payload).unwrap();
            let connection = rusqlite::Connection::open(root.path().join("history")).unwrap();
            let body: String = connection
                .query_row("SELECT body FROM pending WHERE singleton=1", [], |row| {
                    row.get(0)
                })
                .unwrap();
            let mut body: serde_json::Value = serde_json::from_str(&body).unwrap();
            match tamper {
                "operation" => body["id"] = 999.into(),
                "ordinal" => body["ordinal"] = 99.into(),
                "target" => {
                    let key = if kind == "copy" { "path" } else { "current" };
                    body["change"][key] = serde_json::to_value(RawPath(other.clone())).unwrap();
                }
                "context" => {
                    connection
                        .execute("DELETE FROM pending_bindings", [])
                        .unwrap();
                }
                "older-context" => {
                    let saved: String = connection
                        .query_row(
                            "SELECT body FROM pending_bindings WHERE lane=0",
                            [],
                            |row| row.get(0),
                        )
                        .unwrap();
                    let mut saved: serde_json::Value = serde_json::from_str(&saved).unwrap();
                    saved.as_object_mut().unwrap().remove("forward");
                    connection
                        .execute(
                            "UPDATE pending_bindings SET body=?1 WHERE lane=0",
                            [serde_json::to_string(&saved).unwrap()],
                        )
                        .unwrap();
                }
                "deleted-operation" => {
                    connection
                        .execute("DELETE FROM operations WHERE id=1", [])
                        .unwrap();
                }
                "history" => {
                    connection
                        .execute(
                            "VACUUM INTO ?1",
                            [root.path().join("cloned-history").to_str().unwrap()],
                        )
                        .unwrap();
                }
                "occupied" => {
                    connection
                        .execute(
                            "INSERT INTO changes(operation,ordinal,body) VALUES(?1,?2,?3)",
                            rusqlite::params![
                                1,
                                0,
                                serde_json::to_string(&body["change"]).unwrap()
                            ],
                        )
                        .unwrap();
                }
                _ => unreachable!(),
            }
            connection
                .execute(
                    "UPDATE pending SET body=?1 WHERE singleton=1",
                    [serde_json::to_string(&body).unwrap()],
                )
                .unwrap();
            drop(connection);
            let history = root.path().join(if tamper == "history" {
                "cloned-history"
            } else {
                "history"
            });
            for _ in 0..2 {
                let engine = OperationEngine::open(&history).unwrap();
                assert!(
                    !engine
                        .interrupted_operations(&Cancellation::default())
                        .unwrap()
                        .is_empty()
                );
                assert!(
                    payload_id.matches(&payload),
                    "{kind}/{tamper}: recovery moved the payload before validating its step"
                );
                assert!(marker.previous.matches(&previous));
                assert_eq!(fs::read(&payload).unwrap(), b"Selected incoming bytes");
                assert_eq!(fs::read(&previous).unwrap(), b"Selected previous bytes");
                assert!(
                    fs::symlink_metadata(&target)
                        .is_err_and(|e| e.kind() == io::ErrorKind::NotFound)
                );
                assert!(
                    fs::symlink_metadata(&other)
                        .is_err_and(|e| e.kind() == io::ErrorKind::NotFound)
                );
            }
        }));
        if let Err(panic) = checked {
            eprintln!(
                "journal fixture {kind}/{tamper} retained at {} and {}",
                root.keep().display(),
                output.keep().display()
            );
            std::panic::resume_unwind(panic);
        }
    }
}

#[test]
fn a_missing_operation_cannot_publish_a_valid_replacement_payload() {
    unchanged_after_tamper("operation");
}
#[test]
fn a_different_ordinal_cannot_publish_a_valid_replacement_payload() {
    unchanged_after_tamper("ordinal");
}
#[test]
fn a_changed_target_cannot_publish_a_valid_replacement_payload() {
    unchanged_after_tamper("target");
}
#[test]
fn missing_context_cannot_publish_a_valid_replacement_payload() {
    unchanged_after_tamper("context");
}
#[test]
fn an_occupied_history_slot_cannot_publish_a_valid_replacement_payload() {
    unchanged_after_tamper("occupied");
}

#[test]
fn an_older_context_cannot_grant_a_new_claim_restart_authority() {
    unchanged_after_tamper("older-context");
}

#[test]
fn a_deleted_operation_preserves_its_still_bound_payload() {
    unchanged_after_tamper("deleted-operation");
}

#[test]
fn a_cloned_history_cannot_own_an_original_replacement_claim() {
    unchanged_after_tamper("history");
}
