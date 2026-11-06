use super::*;

fn value(value: impl Into<Value<'static>>) -> OwnedValue {
    OwnedValue::try_from(value.into()).unwrap()
}
fn device(id: &str, writable: bool) -> (String, Options, Options) {
    (
        id.into(),
        Options::new(),
        Options::from([("writable".into(), writable.into())]),
    )
}
fn request() -> UsbRequest {
    UsbRequest::parse(
        "example.App".into(),
        "wayland:parent".into(),
        vec![device("one", true), device("two", false)],
        Options::new(),
    )
    .unwrap()
}
fn grant(devices: &[(&str, bool)]) -> Grant {
    Grant {
        devices: devices
            .iter()
            .map(|(id, writable)| SelectedDevice {
                id: (*id).into(),
                writable: *writable,
            })
            .collect(),
    }
}
#[test]
fn results_preserve_opaque_identity_and_cannot_expand_the_offered_authority() {
    let request = request();
    assert!(
        request
            .response(grant(&[("one", true), ("two", false)]))
            .is_some()
    );
    assert!(request.response(grant(&[("one", false)])).is_some());
    assert!(request.response(grant(&[])).is_some());
    assert!(request.response(grant(&[("two", true)])).is_none());
    assert!(request.response(grant(&[("other", false)])).is_none());
    assert!(
        request
            .response(grant(&[("one", false), ("one", false)]))
            .is_none()
    );
}
#[test]
fn sparse_metadata_is_usable_and_encoded_labels_are_plain_text_without_opening_paths() {
    let mut offered = device("opaque-id", false);
    offered.1 = Options::from([
        ("device-file".into(), value("/never/open/this/device")),
        (
            "properties".into(),
            value(Options::from([
                ("ID_MODEL_ENC".into(), value("Game\\x20controller\\x1b")),
                (
                    "ID_VENDOR_FROM_DATABASE".into(),
                    value("Example\u{202e}Vendor"),
                ),
                ("ID_VENDOR_ID".into(), value("1234")),
                ("ID_MODEL_ID".into(), value("aBcD")),
                ("ID_SERIAL_SHORT".into(), value("serial-2")),
            ])),
        ),
    ]);
    let request =
        UsbRequest::parse(String::new(), String::new(), vec![offered], Options::new()).unwrap();
    assert!(request.valid());
    assert_eq!(request.devices[0].name, "Game controller");
    assert_eq!(request.devices[0].vendor, "Example Vendor");
    assert_eq!(request.devices[0].hardware_id, "1234:aBcD");
    assert_eq!(request.devices[0].serial, "serial-2");
    assert_eq!(request.devices[0].location, "/never/open/this/device");
    assert_eq!(self::request().devices[0].name, "USB device");
}
#[test]
fn recognized_wrong_types_duplicates_and_oversized_requests_fail_before_presentation() {
    let parse = |devices| UsbRequest::parse(String::new(), String::new(), devices, Options::new());
    assert!(parse(vec![]).is_err());
    assert!(parse(vec![device("one", false), device("one", true)]).is_err());
    assert!(parse((0..65).map(|i| device(&i.to_string(), false)).collect()).is_err());
    let mut wrong = device("one", false);
    wrong.2.insert("writable".into(), value("true"));
    assert!(parse(vec![wrong]).is_err());
    let mut wrong = device("one", false);
    wrong.1.insert(
        "properties".into(),
        value(Options::from([("ID_MODEL_ENC".into(), 42u32.into())])),
    );
    assert!(parse(vec![wrong]).is_err());
    let mut oversized = device("one", false);
    oversized
        .1
        .insert("device-file".into(), value("x".repeat(FIELD_BYTES + 1)));
    assert!(parse(vec![oversized]).is_err());
    let mut unknown = device("one", false);
    unknown.1.insert("future-information".into(), 42u32.into());
    unknown.2.insert("future-access".into(), value("hint"));
    assert!(parse(vec![unknown]).is_ok());
}
