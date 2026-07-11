//! Parsers for the two MOTD source formats: a `§`/`&`-coded plain string,
//! and a Minecraft JSON chat component (object or array).

use serde_json::Value;

use super::component::{McColor, McColorOrHex, MotdComponent};

/// Parses a `§`-formatted (or `&`-formatted, some servers use that as an
/// alias) string into flat styled runs.
///
/// Color codes (`0`-`9`, `a`-`f`) reset all active formatting flags, same
/// as the vanilla client. Format codes (`l`,`o`,`n`,`m`,`k`) only add to
/// the active set. `r` resets color and all formatting.
pub fn parse_legacy(raw: &str) -> Vec<MotdComponent> {
    let mut components = Vec::new();
    let mut current = MotdComponent::default();
    let mut chars = raw.chars().peekable();

    while let Some(c) = chars.next() {
        if (c == '§' || c == '&') && chars.peek().is_some() {
            let code = chars.next().unwrap().to_ascii_lowercase();
            if let Some(color) = McColor::from_code(code) {
                if !current.text.is_empty() {
                    components.push(std::mem::take(&mut current));
                }
                current.color = Some(McColorOrHex::Named(color));
                current.bold = false;
                current.italic = false;
                current.underlined = false;
                current.strikethrough = false;
                current.obfuscated = false;
                continue;
            }
            match code {
                'r' => {
                    if !current.text.is_empty() {
                        components.push(std::mem::take(&mut current));
                    }
                    current = MotdComponent::default();
                }
                'l' => {
                    if !current.text.is_empty() {
                        components.push(current.clone());
                        current.text.clear();
                    }
                    current.bold = true;
                }
                'o' => {
                    if !current.text.is_empty() {
                        components.push(current.clone());
                        current.text.clear();
                    }
                    current.italic = true;
                }
                'n' => {
                    if !current.text.is_empty() {
                        components.push(current.clone());
                        current.text.clear();
                    }
                    current.underlined = true;
                }
                'm' => {
                    if !current.text.is_empty() {
                        components.push(current.clone());
                        current.text.clear();
                    }
                    current.strikethrough = true;
                }
                'k' => {
                    if !current.text.is_empty() {
                        components.push(current.clone());
                        current.text.clear();
                    }
                    current.obfuscated = true;
                }
                _ => {
                    // Unknown/invalid code: keep it as literal text so we
                    // don't silently drop bytes from an odd server.
                    current.text.push(c);
                    current.text.push(code);
                }
            }
            continue;
        }
        current.text.push(c);
    }
    if !current.text.is_empty() {
        components.push(current);
    }
    components
}

/// Parses a Minecraft JSON chat component (`{"text": ..., "extra": [...],
/// "color": ..., "bold": ..., ...}` or a bare array, which is treated as
/// `{"extra": [...]}`) into flat styled runs.
pub fn parse_json(raw: &Value) -> Vec<MotdComponent> {
    let mut out = Vec::new();
    let base = MotdComponent::default();
    walk(raw, &base, &mut out);
    out
}

fn walk(value: &Value, parent: &MotdComponent, out: &mut Vec<MotdComponent>) {
    match value {
        Value::String(s) => {
            let mut c = parent.clone();
            c.text = s.clone();
            if !c.text.is_empty() {
                out.push(c);
            }
        }
        Value::Array(items) => {
            for item in items {
                walk(item, parent, out);
            }
        }
        Value::Object(map) => {
            let mut style = parent.clone();
            style.text.clear();

            if let Some(color_val) = map.get("color").and_then(Value::as_str) {
                style.color = Some(parse_color_field(color_val));
            }
            if let Some(b) = map.get("bold").and_then(Value::as_bool) {
                style.bold = b;
            }
            if let Some(b) = map.get("italic").and_then(Value::as_bool) {
                style.italic = b;
            }
            if let Some(b) = map.get("underlined").and_then(Value::as_bool) {
                style.underlined = b;
            }
            if let Some(b) = map.get("strikethrough").and_then(Value::as_bool) {
                style.strikethrough = b;
            }
            if let Some(b) = map.get("obfuscated").and_then(Value::as_bool) {
                style.obfuscated = b;
            }

            if let Some(text) = map.get("text").and_then(Value::as_str) {
                let mut c = style.clone();
                c.text = text.to_owned();
                if !c.text.is_empty() {
                    out.push(c);
                }
            }

            if let Some(extra) = map.get("extra") {
                walk(extra, &style, out);
            }
        }
        _ => {}
    }
}

fn parse_color_field(value: &str) -> McColorOrHex {
    if let Some(hex) = value.strip_prefix('#') {
        return McColorOrHex::Hex(format!("#{}", hex.to_uppercase()));
    }
    match McColor::from_name(value) {
        Some(c) => McColorOrHex::Named(c),
        None => McColorOrHex::Hex(value.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_no_codes() {
        let c = parse_legacy("A Minecraft Server");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].text, "A Minecraft Server");
    }

    #[test]
    fn color_reset_clears_formatting() {
        let c = parse_legacy("§4§lbold red§rplain");
        assert_eq!(c.len(), 2);
        assert!(c[0].bold);
        assert_eq!(c[0].text, "bold red");
        assert!(!c[1].bold);
        assert_eq!(c[1].text, "plain");
    }

    #[test]
    fn json_text_and_extra_inherit_style() {
        let raw: Value = serde_json::from_str(
            r#"{"text":"Hello ","color":"red","extra":[{"text":"world","bold":true}]}"#,
        )
        .unwrap();
        let c = parse_json(&raw);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].text, "Hello ");
        assert_eq!(c[1].text, "world");
        assert!(c[1].bold);
        assert_eq!(c[1].color.as_ref().unwrap().hex(), McColor::Red.hex());
    }
}
