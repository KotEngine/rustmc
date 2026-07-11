use std::collections::HashMap;

use rustmc::{Motd, QueryBasicResponse, QueryResponse};

fn sample_map() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("hostname".to_owned(), "A Query Server".to_owned());
    m.insert("gametype".to_owned(), "SMP".to_owned());
    m.insert("game_id".to_owned(), "MINECRAFT".to_owned());
    m.insert("version".to_owned(), "1.20.1".to_owned());
    m.insert("plugins".to_owned(), "Paper 1.20.1: WorldEdit 7.2; ViaVersion 4.9".to_owned());
    m.insert("map".to_owned(), "world".to_owned());
    m.insert("numplayers".to_owned(), "3".to_owned());
    m.insert("maxplayers".to_owned(), "20".to_owned());
    m.insert("hostport".to_owned(), "25565".to_owned());
    m.insert("hostip".to_owned(), "127.0.0.1".to_owned());
    m
}

#[test]
fn builds_full_stat_from_well_formed_map() {
    let resp = QueryResponse::build(sample_map(), vec!["Alice".into(), "Bob".into()]).unwrap();
    assert_eq!(resp.motd.to_plain(), "A Query Server");
    assert_eq!(resp.map_name, "world");
    assert_eq!(resp.players.online, 3);
    assert_eq!(resp.players.max, 20);
    assert_eq!(resp.players.list, vec!["Alice", "Bob"]);
    assert_eq!(resp.ip, "127.0.0.1");
    assert_eq!(resp.port, 25565);
    assert_eq!(resp.game_type, "SMP");
    assert_eq!(resp.game_id, "MINECRAFT");
    assert_eq!(resp.software.version, "1.20.1");
    assert_eq!(resp.software.brand, "Paper 1.20.1");
    assert_eq!(resp.software.plugins, vec!["WorldEdit 7.2", "ViaVersion 4.9"]);
}

#[test]
fn empty_plugins_field_means_vanilla() {
    let mut m = sample_map();
    m.insert("plugins".to_owned(), String::new());
    let resp = QueryResponse::build(m, vec![]).unwrap();
    assert_eq!(resp.software.brand, "vanilla");
    assert!(resp.software.plugins.is_empty());
}

#[test]
fn plugins_without_colon_is_treated_as_bare_brand() {
    let mut m = sample_map();
    m.insert("plugins".to_owned(), "CraftBukkit".to_owned());
    let resp = QueryResponse::build(m, vec![]).unwrap();
    assert_eq!(resp.software.brand, "CraftBukkit");
    assert!(resp.software.plugins.is_empty());
}

#[test]
fn missing_required_field_is_an_error() {
    let mut m = sample_map();
    m.remove("hostname");
    assert!(QueryResponse::build(m, vec![]).is_err());
}

#[test]
fn missing_game_id_defaults_to_minecraft() {
    let mut m = sample_map();
    m.remove("game_id");
    let resp = QueryResponse::build(m, vec![]).unwrap();
    assert_eq!(resp.game_id, "MINECRAFT");
}

#[test]
fn empty_player_list_is_allowed() {
    let resp = QueryResponse::build(sample_map(), vec![]).unwrap();
    assert!(resp.players.list.is_empty());
}

#[test]
fn basic_response_holds_expected_fields() {
    let basic = QueryBasicResponse {
        motd: Motd::parse("A Query Server", false),
        game_type: "SMP".to_owned(),
        map: "world".to_owned(),
        online: 3,
        max: 20,
        port: 25565,
        host_ip: "127.0.0.1".to_owned(),
    };
    assert_eq!(basic.motd.to_plain(), "A Query Server");
    assert_eq!(basic.online, 3);
    assert_eq!(basic.max, 20);
    assert_eq!(basic.port, 25565);
}
