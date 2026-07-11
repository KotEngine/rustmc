//! MOTD (message of the day) parsing and rendering.

pub mod component;
pub mod parser;

pub use component::{McColor, McColorOrHex, MotdComponent};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A parsed server MOTD, with the source preserved for anything the
/// renderers below don't cover.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Motd {
    pub parsed: Vec<MotdComponent>,
    pub raw: Value,
    pub bedrock: bool,
}

impl Motd {
    /// Parses a `§`/`&`-coded plain string MOTD (Bedrock, Legacy, and
    /// simple Java responses all use this format).
    pub fn parse(raw: &str, bedrock: bool) -> Self {
        Self {
            parsed: parser::parse_legacy(raw),
            raw: Value::String(raw.to_owned()),
            bedrock,
        }
    }

    /// Parses a Java JSON chat component MOTD (`{"text": ...}` or a bare
    /// array). Falls back to treating `raw` as a plain string if it's
    /// neither an object, array, nor already a JSON string value.
    pub fn parse_json(raw: &Value, bedrock: bool) -> Self {
        let parsed = match raw {
            Value::String(s) => parser::parse_legacy(s),
            _ => parser::parse_json(raw),
        };
        Self { parsed, raw: raw.clone(), bedrock }
    }

    /// Plain text, all formatting stripped.
    pub fn to_plain(&self) -> String {
        self.parsed.iter().map(|c| c.text.as_str()).collect()
    }

    /// ANSI escape codes for terminal output. Bold/italic/underline map to
    /// their standard SGR codes; obfuscated/strikethrough text is rendered
    /// plain (no stable terminal equivalent for the former).
    pub fn to_ansi(&self) -> String {
        let mut out = String::new();
        for c in &self.parsed {
            if let Some(color) = &c.color {
                out.push_str(color.ansi());
            }
            if c.bold {
                out.push_str("\x1b[1m");
            }
            if c.italic {
                out.push_str("\x1b[3m");
            }
            if c.underlined {
                out.push_str("\x1b[4m");
            }
            if c.strikethrough {
                out.push_str("\x1b[9m");
            }
            out.push_str(&c.text);
            out.push_str("\x1b[0m");
        }
        out
    }

    /// HTML with inline `<span style="...">` per run.
    pub fn to_html(&self) -> String {
        let mut out = String::new();
        for c in &self.parsed {
            let mut style = String::new();
            if let Some(color) = &c.color {
                style.push_str(&format!("color:{};", color.hex()));
            }
            if c.bold {
                style.push_str("font-weight:bold;");
            }
            if c.italic {
                style.push_str("font-style:italic;");
            }
            if c.underlined && c.strikethrough {
                style.push_str("text-decoration:underline line-through;");
            } else if c.underlined {
                style.push_str("text-decoration:underline;");
            } else if c.strikethrough {
                style.push_str("text-decoration:line-through;");
            }
            let escaped = html_escape(&c.text);
            if style.is_empty() {
                out.push_str(&escaped);
            } else {
                out.push_str(&format!(r#"<span style="{style}">{escaped}</span>"#));
            }
        }
        out
    }

    /// Re-renders with `§`-formatting codes.
    pub fn to_minecraft(&self) -> String {
        let mut out = String::new();
        for c in &self.parsed {
            if let Some(color) = &c.color {
                if let Some(code) = color.minecraft_code() {
                    out.push('§');
                    out.push(code);
                }
            }
            if c.bold {
                out.push_str("§l");
            }
            if c.italic {
                out.push_str("§o");
            }
            if c.underlined {
                out.push_str("§n");
            }
            if c.strikethrough {
                out.push_str("§m");
            }
            if c.obfuscated {
                out.push_str("§k");
            }
            out.push_str(&c.text);
        }
        out
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_plain() {
        let motd = Motd::parse("§cA Minecraft §lServer", false);
        assert_eq!(motd.to_plain(), "A Minecraft Server");
    }
}
