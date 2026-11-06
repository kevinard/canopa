use super::*;

// Reuse the normal workspace's compiled harness for native parent-side review;
// the Python controller drives the real Iced helper on its private compositor.
#[tokio::test]
#[ignore = "requires the private Print dialog fixture and its owned input/output files"]
async fn native_review_fixture() {
    let input = std::env::var_os("CANOPA_PRINT_REVIEW_INPUT").expect("fixture input path");
    let output = std::env::var_os("CANOPA_PRINT_REVIEW_OUTPUT").expect("fixture output path");
    let binary = std::env::var_os("CANOPA_PRINT_REVIEW_BINARY").expect("normal portal executable");
    let config = std::env::var_os("CANOPA_PRINT_REVIEW_CONFIG").expect("fixture configuration");
    let request = serde_json::from_slice(&std::fs::read(input).unwrap()).unwrap();
    let (cancel, cancelled) = watch::channel(false);
    let process = DialogProcess {
        executable: binary.into(),
        arguments: vec!["--canopa-config".into(), config],
    };
    let reviewed = async {
        let approval = review(&process, request, cancelled.clone()).await?;
        if let Some(approval) = &approval
            && let Some(pin) = &approval.file
        {
            let client = crate::print::worker::Client::new(process.clone());
            let result = client.verify_file(pin, cancelled).await;
            client.shutdown().await;
            anyhow::ensure!(result?.is_some(), "File review verification cancelled");
        }
        Ok::<_, anyhow::Error>(approval)
    }
    .await;
    if std::env::var_os("CANOPA_PRINT_REVIEW_EXPECT_REJECT").is_some() {
        assert!(reviewed.is_err(), "changed destination was not rejected");
        std::fs::write(
            output,
            br#"{"grant":null,"file_pin":false,"rejected":true}"#,
        )
        .unwrap();
        return;
    }
    let approval = reviewed.unwrap();
    let result = match approval {
        Some(approval) => {
            if let Some(file) = &approval.file {
                file.verify().unwrap();
            }
            serde_json::json!({"grant": approval.grant, "file_pin": approval.file.is_some()})
        }
        None => serde_json::json!({"grant": null, "file_pin": false}),
    };
    drop(cancel);
    std::fs::write(output, serde_json::to_vec(&result).unwrap()).unwrap();
}
