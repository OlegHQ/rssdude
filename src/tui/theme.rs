use ratatui::style::Color;

pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub fg_dim: Color,
    pub fg_faint: Color,
    pub accent: Color,
    pub accent_dim: Color,
    pub accent_secondary: Color,
    pub border: Color,
    pub border_focus: Color,
    pub error: Color,
    pub status_bg_1: Color,
    pub status_bg_2: Color,
    pub status_bg_3: Color,
    pub status_fg: Color,
}

impl Theme {
    pub fn dark() -> Self {
        Theme {
            bg: Color::Reset,
            fg: Color::Rgb(224, 224, 224),
            fg_dim: Color::Rgb(144, 144, 144),
            fg_faint: Color::Rgb(85, 85, 85),
            accent: Color::Rgb(238, 111, 248),
            accent_dim: Color::Rgb(173, 88, 180),
            accent_secondary: Color::Rgb(4, 181, 117),
            border: Color::Rgb(60, 60, 60),
            border_focus: Color::Rgb(125, 86, 244),
            error: Color::Rgb(255, 95, 86),
            status_bg_1: Color::Rgb(125, 86, 244),
            status_bg_2: Color::Rgb(53, 53, 51),
            status_bg_3: Color::Rgb(97, 36, 223),
            status_fg: Color::Rgb(255, 253, 245),
        }
    }

    pub fn light() -> Self {
        Theme {
            bg: Color::Reset,
            fg: Color::Rgb(26, 26, 46),
            fg_dim: Color::Rgb(102, 102, 128),
            fg_faint: Color::Rgb(160, 160, 176),
            accent: Color::Rgb(67, 56, 202),
            accent_dim: Color::Rgb(99, 102, 241),
            accent_secondary: Color::Rgb(5, 150, 105),
            border: Color::Rgb(209, 213, 219),
            border_focus: Color::Rgb(67, 56, 202),
            error: Color::Rgb(220, 38, 38),
            status_bg_1: Color::Rgb(67, 56, 202),
            status_bg_2: Color::Rgb(229, 231, 235),
            status_bg_3: Color::Rgb(55, 48, 163),
            status_fg: Color::Rgb(255, 253, 245),
        }
    }

    pub fn solarized() -> Self {
        Theme {
            bg: Color::Reset,
            fg: Color::Rgb(131, 148, 150),
            fg_dim: Color::Rgb(101, 123, 131),
            fg_faint: Color::Rgb(88, 110, 117),
            accent: Color::Rgb(38, 139, 210),
            accent_dim: Color::Rgb(42, 161, 152),
            accent_secondary: Color::Rgb(133, 153, 0),
            border: Color::Rgb(7, 54, 66),
            border_focus: Color::Rgb(38, 139, 210),
            error: Color::Rgb(220, 50, 47),
            status_bg_1: Color::Rgb(38, 139, 210),
            status_bg_2: Color::Rgb(7, 54, 66),
            status_bg_3: Color::Rgb(42, 161, 152),
            status_fg: Color::Rgb(253, 246, 227),
        }
    }

    pub fn from_name(name: &str) -> Self {
        match name {
            "light" => Self::light(),
            "solarized" => Self::solarized(),
            _ => Self::dark(),
        }
    }
}
