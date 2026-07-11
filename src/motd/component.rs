//! MOTD component types: a MOTD is modeled as a flat sequence of styled
//! text runs (`MotdComponent`), which is enough to render `to_plain`,
//! `to_ansi`, `to_html`, and `to_minecraft` without needing a tree.

use serde::{Deserialize, Serialize};

/// A named Minecraft color (Java and Bedrock share the same 16 codes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum McColor {
    Black,
    DarkBlue,
    DarkGreen,
    DarkAqua,
    DarkRed,
    DarkPurple,
    Gold,
    Gray,
    DarkGray,
    Blue,
    Green,
    Aqua,
    Red,
    LightPurple,
    Yellow,
    White,
}

impl McColor {
    /// Parses a single-character `§` color code (`'0'..='f'`).
    pub fn from_code(c: char) -> Option<Self> {
        use McColor::*;
        Some(match c {
            '0' => Black,
            '1' => DarkBlue,
            '2' => DarkGreen,
            '3' => DarkAqua,
            '4' => DarkRed,
            '5' => DarkPurple,
            '6' => Gold,
            '7' => Gray,
            '8' => DarkGray,
            '9' => Blue,
            'a' => Green,
            'b' => Aqua,
            'c' => Red,
            'd' => LightPurple,
            'e' => Yellow,
            'f' => White,
            _ => return None,
        })
    }

    /// Parses a JSON chat-component color name (`"dark_red"`, `"red"`, ...).
    pub fn from_name(name: &str) -> Option<Self> {
        use McColor::*;
        Some(match name {
            "black" => Black,
            "dark_blue" => DarkBlue,
            "dark_green" => DarkGreen,
            "dark_aqua" => DarkAqua,
            "dark_red" => DarkRed,
            "dark_purple" => DarkPurple,
            "gold" => Gold,
            "gray" | "grey" => Gray,
            "dark_gray" | "dark_grey" => DarkGray,
            "blue" => Blue,
            "green" => Green,
            "aqua" => Aqua,
            "red" => Red,
            "light_purple" => LightPurple,
            "yellow" => Yellow,
            "white" => White,
            _ => return None,
        })
    }

    /// The `§`-code character for this color.
    pub fn code(self) -> char {
        use McColor::*;
        match self {
            Black => '0',
            DarkBlue => '1',
            DarkGreen => '2',
            DarkAqua => '3',
            DarkRed => '4',
            DarkPurple => '5',
            Gold => '6',
            Gray => '7',
            DarkGray => '8',
            Blue => '9',
            Green => 'a',
            Aqua => 'b',
            Red => 'c',
            LightPurple => 'd',
            Yellow => 'e',
            White => 'f',
        }
    }

    /// Standard hex RGB for this color, as used by vanilla clients.
    pub fn hex(self) -> &'static str {
        use McColor::*;
        match self {
            Black => "#000000",
            DarkBlue => "#0000AA",
            DarkGreen => "#00AA00",
            DarkAqua => "#00AAAA",
            DarkRed => "#AA0000",
            DarkPurple => "#AA00AA",
            Gold => "#FFAA00",
            Gray => "#AAAAAA",
            DarkGray => "#555555",
            Blue => "#5555FF",
            Green => "#55FF55",
            Aqua => "#55FFFF",
            Red => "#FF5555",
            LightPurple => "#FF55FF",
            Yellow => "#FFFF55",
            White => "#FFFFFF",
        }
    }

    /// ANSI terminal escape code (foreground) for this color.
    pub fn ansi(self) -> &'static str {
        use McColor::*;
        match self {
            Black => "\x1b[30m",
            DarkBlue => "\x1b[34m",
            DarkGreen => "\x1b[32m",
            DarkAqua => "\x1b[36m",
            DarkRed => "\x1b[31m",
            DarkPurple => "\x1b[35m",
            Gold => "\x1b[33m",
            Gray => "\x1b[37m",
            DarkGray => "\x1b[90m",
            Blue => "\x1b[94m",
            Green => "\x1b[92m",
            Aqua => "\x1b[96m",
            Red => "\x1b[91m",
            LightPurple => "\x1b[95m",
            Yellow => "\x1b[93m",
            White => "\x1b[97m",
        }
    }
}

/// Either a standard named color or a raw `#RRGGBB` web color (used by
/// modern JSON chat components).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MotdColor {
    Named(McColor),
    Hex(String),
}

impl MotdColor {
    pub fn hex(&self) -> String {
        match self {
            MotdColor::Named(c) => c.hex().to_owned(),
            MotdColor::Hex(h) => h.clone(),
        }
    }
}

/// A single run of text sharing the same styling. A MOTD is a `Vec` of
/// these, produced by flattening either the `§`-coded string format or the
/// JSON chat-component tree (`text` + `extra`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MotdComponent {
    pub text: String,
    pub color: Option<McColorOrHex>,
    pub bold: bool,
    pub italic: bool,
    pub underlined: bool,
    pub strikethrough: bool,
    pub obfuscated: bool,
}

/// Color of a run: either one of the 16 named codes or an arbitrary hex
/// value (JSON components only support hex on modern versions).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum McColorOrHex {
    Named(McColor),
    Hex(String),
}

impl McColorOrHex {
    pub fn hex(&self) -> String {
        match self {
            McColorOrHex::Named(c) => c.hex().to_owned(),
            McColorOrHex::Hex(h) => h.clone(),
        }
    }

    pub fn ansi(&self) -> &'static str {
        match self {
            McColorOrHex::Named(c) => c.ansi(),
            // No portable ANSI 24-bit fallback attempt here; treat unknown
            // hex colors as "no color" for terminal output rather than
            // guessing wrong.
            McColorOrHex::Hex(_) => "",
        }
    }

    pub fn minecraft_code(&self) -> Option<char> {
        match self {
            McColorOrHex::Named(c) => Some(c.code()),
            McColorOrHex::Hex(_) => None,
        }
    }
}
