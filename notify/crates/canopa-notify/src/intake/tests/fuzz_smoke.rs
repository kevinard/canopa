//! Deterministic mutation smoke coverage in every core CI run; not a claim of exhaustive fuzzing.
use super::super::*;

#[test]
fn body_parser_mutation_smoke() {
    let seeds = [
        "plain text",
        "<b>bold <i>italic</i></b>&amp;&#x1f332;",
        "<a href=\"https://example.test/a?b=1&amp;c=2\">read <u>this</u></a>",
        "<a href=\"javascript:bad\"><a href=\"https://example.test\">nested</a></a>",
        "<img src=\"file:///tmp/fixture.png\" alt=\"a &amp; b\"/>",
        "<!-- incomplete <b>\0\u{202e}",
        "<!DOCTYPE x [<!ENTITY a 'repeated'>]><x>&a;</x>",
        "<b><b><b></u>&#999999999999999999999999999;",
    ];
    let insertions = [
        '<', '>', '&', ';', '/', '=', '\'', '"', '\0', '\n', '\u{202e}', '🌲',
    ];
    let mut random = 0x14d9_2718_53a0_6b21u64;
    for iteration in 0..2048 {
        let mut input: Vec<char> = seeds[iteration % seeds.len()].chars().collect();
        for _ in 0..16 {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            let at = random as usize % (input.len() + 1);
            if random & 1 == 0 && at < input.len() {
                input.remove(at);
            } else {
                input.insert(at, insertions[(random >> 16) as usize % insertions.len()]);
            }
        }
        let input: String = input.into_iter().collect();
        let content = interpreted_body_content(&input);
        assert!(
            content.segments.len() <= MAX_BODY_CONTENT_SEGMENTS,
            "seed {iteration}"
        );
        let encoded = serde_json::to_vec(&content).unwrap();
        let restored: NotificationBodyContent = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(restored, content, "seed {iteration}");
        for segment in content.segments {
            if let NotificationBodySegment::Link { uri, .. } = segment {
                assert!(
                    NotificationLink::parse(uri.as_str()).is_ok(),
                    "seed {iteration}"
                );
            }
        }
    }
    for input in ["<b>".repeat(4096), "<img alt='x'/>".repeat(4096)] {
        assert!(interpreted_body_content(&input).segments.len() <= MAX_BODY_CONTENT_SEGMENTS);
    }
}
