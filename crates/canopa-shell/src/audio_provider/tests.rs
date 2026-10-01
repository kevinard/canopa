use super::*;
use std::time::Duration;

#[tokio::test]
async fn unavailable_provider_publishes_and_closes_on_a_split_frame() {
    use tokio::io::{AsyncWriteExt, BufReader};
    let (host, provider) = tokio::io::duplex(8192);
    let (input, output) = tokio::io::split(provider);
    let running = tokio::spawn(run(
        BufReader::new(input),
        output,
        Some("unix:/nonexistent-canopa-audio-test".into()),
    ));
    let (input, mut output) = tokio::io::split(host);
    let mut input = BufReader::new(input);
    let response =
        tokio::time::timeout(Duration::from_secs(3), v5::Response::read_async(&mut input))
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    let Content::Rich(content) = response.content else {
        panic!("rich content")
    };
    assert_eq!(content.title, "Sound");
    assert!(content.error.as_deref().unwrap().contains("unavailable"));
    output.write_all(b"{\"type\":\"cl").await.unwrap();
    tokio::time::sleep(Duration::from_millis(75)).await;
    output
        .write_all(b"ose\",\"protocol_version\":5}\n")
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(1), running)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}
