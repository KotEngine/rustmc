use rustmc::BedrockStatusResponse;

fn sample() -> String {
    [
        "MCPE", "\u{00A7}cA Bedrock Server", "622", "1.20.40", "5", "20", "1234567890123456",
        "Bedrock level", "Survival", "1", "19132", "19133",
    ]
    .join(";")
}

#[test]
fn builds_from_well_formed_response() {
    let resp = BedrockStatusResponse::build(&sample(), 8.0).unwrap();
    assert_eq!(resp.version.brand, "MCPE");
    assert_eq!(resp.version.protocol, 622);
    assert_eq!(resp.version.name, "1.20.40");
    assert_eq!(resp.players.online, 5);
    assert_eq!(resp.players.max, 20);
    assert_eq!(resp.motd.to_plain(), "A Bedrock Server");
    assert_eq!(resp.map_name.as_deref(), Some("Bedrock level"));
    assert_eq!(resp.gamemode.as_deref(), Some("Survival"));
    assert_eq!(resp.latency, 8.0);
}

#[test]
fn missing_trailing_fields_do_not_panic() {
    // Geyser/Waterdog and similar proxies sometimes send fewer than the
    // full 12 fields — only the first 6 are guaranteed.
    let raw = "MCPE;A Server;622;1.20.40;3;20";
    let resp = BedrockStatusResponse::build(raw, 1.0).unwrap();
    assert_eq!(resp.players.online, 3);
    assert_eq!(resp.players.max, 20);
    assert!(resp.map_name.is_none());
    assert!(resp.gamemode.is_none());
}

#[test]
fn missing_edition_field_is_an_error() {
    assert!(BedrockStatusResponse::build("", 0.0).is_err());
}

#[test]
fn missing_protocol_field_is_an_error() {
    assert!(BedrockStatusResponse::build("MCPE;A Server", 0.0).is_err());
}

#[test]
fn non_numeric_protocol_is_an_error() {
    assert!(BedrockStatusResponse::build("MCPE;A Server;not-a-number;1.20.40;0;20", 0.0).is_err());
}

#[test]
fn education_edition_brand_is_preserved() {
    let raw = "MCEE;An EDU Server;622;1.20.40;0;30";
    let resp = BedrockStatusResponse::build(raw, 0.0).unwrap();
    assert_eq!(resp.version.brand, "MCEE");
}
