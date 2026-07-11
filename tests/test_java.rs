use rustmc::JavaStatusResponse;
use serde_json::json;

fn sample_json() -> serde_json::Value {
    json!({
        "version": { "name": "1.20.1", "protocol": 763 },
        "players": {
            "online": 5,
            "max": 20,
            "sample": [{ "name": "Notch", "id": "069a79f4-44e9-4726-a5be-fca90e38aaf5" }]
        },
        "description": { "text": "A Minecraft Server" },
        "favicon": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=",
        "enforcesSecureChat": true
    })
}

#[test]
fn builds_from_well_formed_response() {
    let resp = JavaStatusResponse::build(sample_json(), 12.5).unwrap();
    assert_eq!(resp.version.name, "1.20.1");
    assert_eq!(resp.version.protocol, 763);
    assert_eq!(resp.players.online, 5);
    assert_eq!(resp.players.max, 20);
    assert_eq!(resp.motd.to_plain(), "A Minecraft Server");
    assert_eq!(resp.latency, 12.5);
    assert_eq!(resp.enforces_secure_chat, Some(true));
    assert!(resp.dns.is_none());
}

#[test]
fn player_sample_is_parsed() {
    let resp = JavaStatusResponse::build(sample_json(), 0.0).unwrap();
    let sample = resp.players.sample.unwrap();
    assert_eq!(sample.len(), 1);
    assert_eq!(sample[0].name, "Notch");
    assert_eq!(sample[0].uuid(), "069a79f4-44e9-4726-a5be-fca90e38aaf5");
}

#[test]
fn missing_players_field_is_an_error() {
    let raw = json!({ "version": { "name": "1.20.1", "protocol": 763 } });
    assert!(JavaStatusResponse::build(raw, 0.0).is_err());
}

#[test]
fn missing_version_field_is_an_error() {
    let raw = json!({ "players": { "online": 1, "max": 20 } });
    assert!(JavaStatusResponse::build(raw, 0.0).is_err());
}

#[test]
fn missing_description_defaults_to_empty_motd() {
    let raw = json!({
        "version": { "name": "1.20.1", "protocol": 763 },
        "players": { "online": 0, "max": 20 }
    });
    let resp = JavaStatusResponse::build(raw, 0.0).unwrap();
    assert_eq!(resp.motd.to_plain(), "");
}

#[test]
fn decodes_valid_png_favicon() {
    let resp = JavaStatusResponse::build(sample_json(), 0.0).unwrap();
    assert_eq!(resp.icon_mime_type(), Some("image/png"));
    let bytes = resp.icon_bytes().unwrap().unwrap();
    assert!(bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]));
}

#[test]
fn no_favicon_returns_none() {
    let raw = json!({
        "version": { "name": "1.20.1", "protocol": 763 },
        "players": { "online": 0, "max": 20 }
    });
    let resp = JavaStatusResponse::build(raw, 0.0).unwrap();
    assert!(resp.icon_bytes().is_none());
}

#[test]
fn malformed_favicon_png_signature_is_an_error() {
    let raw = json!({
        "version": { "name": "1.20.1", "protocol": 763 },
        "players": { "online": 0, "max": 20 },
        "favicon": "data:image/png;base64,aGVsbG8="
    });
    let resp = JavaStatusResponse::build(raw, 0.0).unwrap();
    assert!(resp.icon_bytes().unwrap().is_err());
}

#[test]
fn legacy_string_description_is_parsed_as_legacy_motd() {
    let raw = json!({
        "version": { "name": "1.8.9", "protocol": 47 },
        "players": { "online": 0, "max": 20 },
        "description": "\u{00A7}cRed Server"
    });
    let resp = JavaStatusResponse::build(raw, 0.0).unwrap();
    assert_eq!(resp.motd.to_plain(), "Red Server");
}
