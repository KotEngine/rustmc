use rustmc::LegacyStatusResponse;

#[test]
fn builds_from_modern_fields() {
    // fields = [protocol, version, motd, online, max] — caller (protocol
    // layer) has already stripped the leading "§1" marker.
    let fields = vec![
        "5".to_owned(),
        "1.4.2".to_owned(),
        "A Minecraft Server".to_owned(),
        "3".to_owned(),
        "20".to_owned(),
    ];
    let resp = LegacyStatusResponse::build(&fields, 15.0).unwrap();
    assert_eq!(resp.version.protocol, 5);
    assert_eq!(resp.version.name, "1.4.2");
    assert_eq!(resp.motd.to_plain(), "A Minecraft Server");
    assert_eq!(resp.players.online, 3);
    assert_eq!(resp.players.max, 20);
    assert_eq!(resp.latency, 15.0);
}

#[test]
fn builds_from_pre_1_4_reconstructed_fields() {
    // Reconstructed by the protocol layer for servers with no "§1" marker:
    // protocol="-1", version="<1.4".
    let fields = vec![
        "-1".to_owned(),
        "<1.4".to_owned(),
        "A Minecraft Server".to_owned(),
        "3".to_owned(),
        "20".to_owned(),
    ];
    let resp = LegacyStatusResponse::build(&fields, 0.0).unwrap();
    assert_eq!(resp.version.protocol, -1);
    assert_eq!(resp.version.name, "<1.4");
}

#[test]
fn colored_motd_is_parsed() {
    let fields = vec![
        "5".to_owned(),
        "1.4.2".to_owned(),
        "\u{00A7}4Red Server".to_owned(),
        "0".to_owned(),
        "20".to_owned(),
    ];
    let resp = LegacyStatusResponse::build(&fields, 0.0).unwrap();
    assert_eq!(resp.motd.to_plain(), "Red Server");
}

#[test]
fn missing_field_is_an_error() {
    let fields = vec!["5".to_owned(), "1.4.2".to_owned()];
    assert!(LegacyStatusResponse::build(&fields, 0.0).is_err());
}

#[test]
fn non_numeric_protocol_is_an_error() {
    let fields = vec![
        "not-a-number".to_owned(),
        "1.4.2".to_owned(),
        "motd".to_owned(),
        "0".to_owned(),
        "20".to_owned(),
    ];
    assert!(LegacyStatusResponse::build(&fields, 0.0).is_err());
}

#[test]
fn non_numeric_player_counts_default_to_zero_rather_than_error() {
    // Player counts are best-effort — some proxies send garbage here, and
    // failing the whole response over an unparseable player count would
    // throw away an otherwise-valid MOTD/version.
    let fields = vec![
        "5".to_owned(),
        "1.4.2".to_owned(),
        "motd".to_owned(),
        "??".to_owned(),
        "??".to_owned(),
    ];
    let resp = LegacyStatusResponse::build(&fields, 0.0).unwrap();
    assert_eq!(resp.players.online, 0);
    assert_eq!(resp.players.max, 0);
}
