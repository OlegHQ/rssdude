use ratatui::style::Color;

pub struct Theme {
    pub fg: Color,
    pub fg_dim: Color,
    pub fg_faint: Color,
    pub accent: Color,
    pub accent_secondary: Color,
    pub border: Color,
    pub border_focus: Color,
    pub status_bg_1: Color,
    pub status_bg_2: Color,
    pub status_bg_3: Color,
    pub status_fg: Color,
}

impl Theme {
    pub fn dark() -> Self {
        Theme {
            fg: Color::Rgb(224, 224, 224),
            fg_dim: Color::Rgb(144, 144, 144),
            fg_faint: Color::Rgb(85, 85, 85),
            accent: Color::Rgb(238, 111, 248),
            accent_secondary: Color::Rgb(4, 181, 117),
            border: Color::Rgb(60, 60, 60),
            border_focus: Color::Rgb(125, 86, 244),
            status_bg_1: Color::Rgb(125, 86, 244),
            status_bg_2: Color::Rgb(53, 53, 51),
            status_bg_3: Color::Rgb(97, 36, 223),
            status_fg: Color::Rgb(255, 253, 245),
        }
    }

    pub fn light() -> Self {
        Theme {
            fg: Color::Rgb(15, 23, 42),
            fg_dim: Color::Rgb(71, 85, 105),
            fg_faint: Color::Rgb(100, 116, 139),
            accent: Color::Rgb(67, 56, 202),
            accent_secondary: Color::Rgb(5, 150, 105),
            border: Color::Rgb(148, 163, 184),
            border_focus: Color::Rgb(67, 56, 202),
            status_bg_1: Color::Rgb(67, 56, 202),
            status_bg_2: Color::Rgb(71, 85, 105),
            status_bg_3: Color::Rgb(55, 48, 163),
            status_fg: Color::Rgb(255, 253, 245),
        }
    }

    pub fn solarized() -> Self {
        Theme {
            fg: Color::Rgb(131, 148, 150),
            fg_dim: Color::Rgb(101, 123, 131),
            fg_faint: Color::Rgb(88, 110, 117),
            accent: Color::Rgb(38, 139, 210),
            accent_secondary: Color::Rgb(133, 153, 0),
            border: Color::Rgb(7, 54, 66),
            border_focus: Color::Rgb(38, 139, 210),
            status_bg_1: Color::Rgb(38, 139, 210),
            status_bg_2: Color::Rgb(7, 54, 66),
            status_bg_3: Color::Rgb(42, 161, 152),
            status_fg: Color::Rgb(253, 246, 227),
        }
    }

    /// Build the theme requested by config. `"auto"` (the default) probes the
    /// terminal background and picks light or dark; explicit names always win.
    pub fn from_name(name: &str) -> Self {
        match name {
            "light" => Self::light(),
            "dark" => Self::dark(),
            "solarized" => Self::solarized(),
            // "auto" or any unknown value → detect.
            _ => detect_or_dark(),
        }
    }
}

/// Probe the terminal background; return `light()` if the bg is bright, else
/// `dark()`. Falls back to dark on any error or non-terminal stdout.
fn detect_or_dark() -> Theme {
    // luma() returns 0.0 (pure black) → 1.0 (pure white). 0.5 is the natural
    // boundary for "is the background closer to white than black".
    match terminal_light::luma() {
        Ok(luma) if luma > 0.5 => Theme::light(),
        _ => Theme::dark(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_name_matches_explicit_palettes() {
        // Spot-check distinguishing colors so we know the right palette is built.
        let dark_fg = Theme::from_name("dark").fg;
        let light_fg = Theme::from_name("light").fg;
        let solar_fg = Theme::from_name("solarized").fg;
        assert_ne!(dark_fg, light_fg);
        assert_ne!(dark_fg, solar_fg);
        assert_ne!(light_fg, solar_fg);
    }

    #[test]
    fn unknown_name_falls_back_to_a_real_palette() {
        // "auto" or junk should resolve to either light or dark — both are valid;
        // the only failure mode is a panic or an empty/unset color.
        let t = Theme::from_name("totally-not-a-theme");
        assert!(t.fg == Theme::dark().fg || t.fg == Theme::light().fg);
    }
}
