use rustmc::Address;

#[test]
fn parses_plain_host_port() {
    let a = Address::parse("mc.hypixel.net:25565", 25565).unwrap();
    assert_eq!(a.host, "mc.hypixel.net");
    assert_eq!(a.port, 25565);
}

#[test]
fn parses_bare_host_falls_back_to_default_port() {
    let a = Address::parse("mc.hypixel.net", 19132).unwrap();
    assert_eq!(a.host, "mc.hypixel.net");
    assert_eq!(a.port, 19132);
}

#[test]
fn parses_custom_port() {
    let a = Address::parse("play.example.net:12345", 25565).unwrap();
    assert_eq!(a.port, 12345);
}

#[test]
fn parses_ipv4_literal() {
    let a = Address::parse("127.0.0.1:25565", 25565).unwrap();
    assert_eq!(a.host, "127.0.0.1");
    assert_eq!(a.port, 25565);
}

#[test]
fn parses_ipv6_bracketed_with_port() {
    let a = Address::parse("[2001:db8::1]:25565", 25565).unwrap();
    assert_eq!(a.host, "2001:db8::1");
    assert_eq!(a.port, 25565);
}

#[test]
fn parses_ipv6_bracketed_without_port_uses_default() {
    let a = Address::parse("[::1]", 19132).unwrap();
    assert_eq!(a.host, "::1");
    assert_eq!(a.port, 19132);
}

#[test]
fn unterminated_bracket_is_an_error() {
    assert!(Address::parse("[2001:db8::1", 25565).is_err());
}

#[test]
fn empty_address_is_an_error() {
    assert!(Address::parse("", 25565).is_err());
    assert!(Address::parse("   ", 25565).is_err());
}

#[test]
fn trims_whitespace() {
    let a = Address::parse("  mc.hypixel.net:25565  ", 25565).unwrap();
    assert_eq!(a.host, "mc.hypixel.net");
}

#[test]
fn literal_ip_resolves_without_dns() {
    let a = Address::parse("127.0.0.1:25565", 25565).unwrap();
    let ip = a.resolve_ip().unwrap();
    assert_eq!(ip.to_string(), "127.0.0.1");
}
