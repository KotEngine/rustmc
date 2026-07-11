use rustmc::Motd;

#[test]
fn plain_string_no_codes() {
    let motd = Motd::parse("A Minecraft Server", false);
    assert_eq!(motd.to_plain(), "A Minecraft Server");
}

#[test]
fn strips_color_codes_in_plain() {
    let motd = Motd::parse("\u{00A7}4Red \u{00A7}9Blue", false);
    assert_eq!(motd.to_plain(), "Red Blue");
}

#[test]
fn color_reset_clears_bold() {
    let motd = Motd::parse("\u{00A7}l\u{00A7}4bold red\u{00A7}rplain", false);
    let plain = motd.to_plain();
    assert_eq!(plain, "bold redplain");
}

#[test]
fn to_minecraft_round_trips_color_code() {
    let motd = Motd::parse("\u{00A7}cHello", false);
    let rendered = motd.to_minecraft();
    assert!(rendered.contains('c'));
    assert!(rendered.contains("Hello"));
}

#[test]
fn to_html_wraps_colored_text_in_span() {
    let motd = Motd::parse("\u{00A7}4Red", false);
    let html = motd.to_html();
    assert!(html.contains("<span"));
    assert!(html.contains("Red"));
}

#[test]
fn to_ansi_contains_reset_codes() {
    let motd = Motd::parse("\u{00A7}4Red", false);
    let ansi = motd.to_ansi();
    assert!(ansi.contains("\x1b["));
    assert!(ansi.contains("Red"));
}

#[test]
fn json_component_text_and_extra() {
    let raw: serde_json::Value = serde_json::from_str(
        r#"{"text":"Hello ","color":"red","extra":[{"text":"world","bold":true}]}"#,
    )
    .unwrap();
    let motd = Motd::parse_json(&raw, false);
    assert_eq!(motd.to_plain(), "Hello world");
}

#[test]
fn json_component_bare_array() {
    let raw: serde_json::Value = serde_json::from_str(r#"[{"text":"A"},{"text":"B"}]"#).unwrap();
    let motd = Motd::parse_json(&raw, false);
    assert_eq!(motd.to_plain(), "AB");
}

#[test]
fn json_component_bare_string() {
    let raw: serde_json::Value = serde_json::from_str(r#""Just a plain string""#).unwrap();
    let motd = Motd::parse_json(&raw, false);
    assert_eq!(motd.to_plain(), "Just a plain string");
}

#[test]
fn json_component_hex_color() {
    let raw: serde_json::Value =
        serde_json::from_str(r##"{"text":"custom","color":"#AB00FF"}"##).unwrap();
    let motd = Motd::parse_json(&raw, false);
    assert_eq!(motd.to_plain(), "custom");
}

#[test]
fn empty_motd_produces_no_components() {
    let motd = Motd::parse("", false);
    assert_eq!(motd.to_plain(), "");
}
