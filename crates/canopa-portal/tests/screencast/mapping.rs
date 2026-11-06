use super::*;
use canopa_portal::screencast::StreamMapping;

fn mapped(mut stream: Stream, id: &str, size: (i32, i32)) -> Stream {
    stream.mapping = Some(StreamMapping {
        id: id.into(),
        size,
    });
    stream
}
fn window() -> Stream {
    Stream {
        node: 102,
        serial: 9002,
        source: Source::Window(42),
        geometry: None,
        mapping: None,
    }
}
fn options() -> Options {
    Options::from([
        ("types".into(), value(3u32)),
        ("multiple".into(), value(true)),
    ])
}

#[tokio::test]
async fn mapping_ids_and_logical_sizes_pair_public_streams_without_compositor_paths() {
    let mut f = Fixture::new().await;
    assert_eq!(f.create("mapped").await.0, 0);
    f.select("mapped", options()).await.unwrap();
    let started = f.start("mapped");
    let request = f.presentation().await;
    let (capture, _source, stops) = grant(vec![
        mapped(stream(101, 9001), "display-map", (640, 480)),
        mapped(window(), "window-map", (400, 300)),
    ]);
    request.reply.send(Ok(Some(capture))).ok().unwrap();
    let (code, mut values) = started.await.unwrap().unwrap();
    assert_eq!(code, 0);
    let rows: Vec<(u32, Options)> = values.remove("streams").unwrap().try_into().unwrap();
    for ((node, props), id, size, expected_keys) in [
        (
            &rows[0],
            "display-map",
            (640, 480),
            vec![
                "mapping_id",
                "pipewire-serial",
                "position",
                "size",
                "source_type",
            ],
        ),
        (
            &rows[1],
            "window-map",
            (400, 300),
            vec!["mapping_id", "pipewire-serial", "size", "source_type"],
        ),
    ] {
        assert_eq!(props["mapping_id"].value_signature().to_string(), "s");
        assert_eq!(
            String::try_from(props["mapping_id"].try_clone().unwrap()).unwrap(),
            id
        );
        let actual: (i32, i32) = props["size"].try_clone().unwrap().try_into().unwrap();
        assert_eq!(actual, size);
        let mut keys: Vec<_> = props.keys().map(String::as_str).collect();
        keys.sort();
        assert_eq!(keys, expected_keys);
        assert!([101, 102].contains(node));
    }
    assert!(!rows[1].1.contains_key("position"));
    assert_eq!(stops.load(Ordering::SeqCst), 0);
    f.portal.shutdown().await.unwrap();
    assert_eq!(stops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn malformed_or_aliased_mapping_metadata_cannot_publish_a_started_capture() {
    let mut f = Fixture::new().await;
    let cases = vec![
        vec![mapped(stream(101, 9001), "", (640, 480))],
        vec![mapped(stream(101, 9001), "bad\nmap", (640, 480))],
        vec![mapped(stream(101, 9001), &"x".repeat(257), (640, 480))],
        vec![mapped(stream(101, 9001), "map", (0, 480))],
        vec![mapped(stream(101, 9001), "map", (641, 480))],
        vec![mapped(window(), "map", (400, 32769))],
        vec![
            mapped(stream(101, 9001), "same", (640, 480)),
            mapped(window(), "same", (400, 300)),
        ],
    ];
    for (index, streams) in cases.into_iter().enumerate() {
        let name = format!("badmap{index}");
        assert_eq!(f.create(&name).await.0, 0);
        f.select(&name, options()).await.unwrap();
        let started = f.start(&name);
        let request = f.presentation().await;
        let (capture, _source, stops) = grant(streams);
        request.reply.send(Ok(Some(capture))).ok().unwrap();
        assert_eq!(started.await.unwrap().unwrap().0, 2);
        retired(&f.client, path(&name)).await;
        assert_eq!(stops.load(Ordering::SeqCst), 1);
    }
    f.portal.shutdown().await.unwrap();
}
