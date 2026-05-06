use ratatui::style::Color;

// Default Theme Tokens (fallback)
pub const GALACTIC_BLUE: Color = Color::Rgb(0, 0, 255);
pub const NEBULA_CYAN: Color = Color::Rgb(0, 255, 255);
pub const STAR_YELLOW: Color = Color::Rgb(255, 255, 0);

pub struct Theme {
    pub primary: Color,
    pub accent: Color,
    pub highlight: Color,
    pub background: Color,
    pub foreground: Color,
    pub success: Color,
    pub error: Color,
    pub warning: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            primary: GALACTIC_BLUE,
            accent: NEBULA_CYAN,
            highlight: STAR_YELLOW,
            background: Color::Black,
            foreground: Color::White,
            success: Color::Green,
            error: Color::Red,
            warning: Color::Yellow,
        }
    }
}

impl Theme {
    pub fn from_config(config: &serde_json::Value) -> Self {
        let theme_json = &config["general"]["theme"];
        Self {
            primary: parse_hex(theme_json["primary"].as_str()).unwrap_or(GALACTIC_BLUE),
            accent: parse_hex(theme_json["accent"].as_str()).unwrap_or(NEBULA_CYAN),
            highlight: parse_hex(theme_json["highlight"].as_str()).unwrap_or(STAR_YELLOW),
            background: parse_hex(theme_json["background"].as_str()).unwrap_or(Color::Black),
            foreground: parse_hex(theme_json["foreground"].as_str()).unwrap_or(Color::White),
            success: parse_hex(theme_json["success"].as_str()).unwrap_or(Color::Green),
            error: parse_hex(theme_json["error"].as_str()).unwrap_or(Color::Red),
            warning: parse_hex(theme_json["warning"].as_str()).unwrap_or(Color::Yellow),
        }
    }
}

fn parse_hex(hex: Option<&str>) -> Option<Color> {
    let hex = hex?;
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}
