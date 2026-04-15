use crossterm::style::Color;

#[derive(Debug, Clone)]
pub struct Theme {
    pub toolbar_bg: Color,
    pub toolbar_fg: Color,
    pub toolbar_fg_dim: Color,
    pub status_bg: Color,
    pub status_fg: Color,
    pub dirty_flag: Color,
    pub help_key: Color,
    pub help_label: Color,
    pub prompt_bg: Color,
    pub prompt_fg: Color,
    pub accent: Color,
    pub warning: Color,
    pub error: Color,
    pub empty_space: Color,
    pub text_main: Color,
}

impl Theme {
    pub fn default(is_light_bg: bool) -> Self {
        if is_light_bg {
            Self {
                toolbar_bg: Color::AnsiValue(254),
                toolbar_fg: Color::Black,
                toolbar_fg_dim: Color::DarkGrey,
                status_bg: Color::Blue,
                status_fg: Color::Black,
                dirty_flag: Color::DarkGrey,
                help_key: Color::DarkBlue,
                help_label: Color::Black,
                prompt_bg: Color::AnsiValue(254),
                prompt_fg: Color::Black,
                accent: Color::DarkMagenta,
                warning: Color::DarkYellow,
                error: Color::DarkRed,
                empty_space: Color::AnsiValue(254),
                text_main: Color::Black,
            }
        } else {
            Self {
                toolbar_bg: Color::AnsiValue(236),
                toolbar_fg: Color::White,
                toolbar_fg_dim: Color::DarkGrey,
                status_bg: Color::Blue,
                status_fg: Color::Black,
                dirty_flag: Color::DarkGrey,
                help_key: Color::DarkYellow,
                help_label: Color::White,
                prompt_bg: Color::AnsiValue(236),
                prompt_fg: Color::White,
                accent: Color::DarkCyan,
                warning: Color::DarkYellow,
                error: Color::DarkRed,
                empty_space: Color::AnsiValue(236),
                text_main: Color::White,
            }
        }
    }
}
