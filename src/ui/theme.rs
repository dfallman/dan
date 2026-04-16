use crossterm::style::Color;

// Note: these are crossterm's colors
// pub enum Color {
// Show 19 variants    
//     Reset,
//     Black,
//     DarkGrey,
//     Red,
//     DarkRed,
//     Green,
//     DarkGreen,
//     Yellow,
//     DarkYellow,
//     Blue,
//     DarkBlue,
//     Magenta,
//     DarkMagenta,
//     Cyan,
//     DarkCyan,
//     White,
//     Grey,
//     Rgb {
//         r: u8,
//         g: u8,
//         b: u8,
//     },
//     AnsiValue(u8),
// }
// Ansi values: https://www.ditig.com/256-colors-cheat-sheet

#[derive(Debug, Clone)]
pub struct Theme {
    pub toolbar_bg: Color,
    pub toolbar_fg: Color,
    pub toolbar_fg_dim: Color,
    pub status_bg: Color,
    pub status_fg: Color,
    pub dirty_flag: Color,
    pub help_label: Color,
    pub prompt_bg: Color,
    pub prompt_fg: Color,
    pub active_row_bg: Color,
    pub mode_edit: Color,
    pub mode_select: Color,
    pub mode_search: Color,
    pub mode_goto: Color,
    pub mode_save: Color,
    pub mode_danger: Color,
    pub mode_replace: Color,
    pub accent: Color,
    pub warning: Color,
    pub error: Color,
    pub empty_space: Color,
    pub text_main: Color,
    // Text Rendering
    pub line_nr: Color,
    pub line_nr_active: Color,
    pub eof_marker: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
    pub match_bg: Color,
    pub match_fg: Color,
    pub active_match_bg: Color,
    pub active_match_fg: Color,

    // Overlays & Prompts
    pub prompt_info: Color,
    pub prompt_warning_bg: Color,
    pub prompt_warning_fg: Color,
    pub prompt_danger_bg: Color,
    pub prompt_danger_fg: Color,
    pub hotkey: Color,
}

impl Theme {
    pub fn default(is_light_bg: bool) -> Self {
        if is_light_bg {
            // Light theme
            Self {
                toolbar_bg: Color::AnsiValue(254),
                toolbar_fg: Color::Black,
                toolbar_fg_dim: Color::DarkGrey,
                status_bg: Color::Blue,
                status_fg: Color::Black,
                dirty_flag: Color::Blue,
                help_label: Color::Black,
                prompt_bg: Color::White,
                prompt_fg: Color::Black,
                active_row_bg: Color::AnsiValue(254),
                mode_edit: Color::Blue,
                mode_select: Color::DarkYellow,
                mode_search: Color::DarkYellow,
                mode_goto: Color::DarkCyan,
                mode_save: Color::DarkGreen,
                mode_danger: Color::DarkRed,
                mode_replace: Color::DarkMagenta,
                accent: Color::DarkMagenta,
                warning: Color::DarkYellow,
                error: Color::DarkRed,
                empty_space: Color::White,
                text_main: Color::Black,

                line_nr: Color::Grey,
                line_nr_active: Color::Blue,
                eof_marker: Color::Grey,
                selection_bg: Color::Cyan,
                selection_fg: Color::Black,
                match_bg: Color::Yellow,
                match_fg: Color::Black,
                active_match_bg: Color::Green,
                active_match_fg: Color::Black,

                prompt_info: Color::DarkGrey,
                prompt_warning_bg: Color::Yellow,
                prompt_warning_fg: Color::Black,
                prompt_danger_bg: Color::Red,
                prompt_danger_fg: Color::Black,
                hotkey: Color::Blue,
            }
        } else {
            // Dark theme
            Self {
                toolbar_bg: Color::AnsiValue(236),
                toolbar_fg: Color::White,
                toolbar_fg_dim: Color::DarkGrey,
                status_bg: Color::Blue,
                status_fg: Color::Black,
                dirty_flag: Color::Blue,                // dirty is file has been edited
                help_label: Color::White,
                prompt_bg: Color::AnsiValue(236),
                prompt_fg: Color::White,
                active_row_bg: Color::AnsiValue(236),
                mode_edit: Color::Blue,
                mode_select: Color::Yellow,
                mode_search: Color::Yellow,
                mode_goto: Color::Cyan,
                mode_save: Color::Green,
                mode_danger: Color::Red,
                mode_replace: Color::Magenta,
                accent: Color::Cyan,
                warning: Color::Yellow,
                error: Color::Red,
                empty_space: Color::AnsiValue(236),
                text_main: Color::White,

                line_nr: Color::DarkGrey,
                line_nr_active: Color::White,
                eof_marker: Color::DarkGrey,
                selection_bg: Color::Cyan,
                selection_fg: Color::Black,
                match_bg: Color::Yellow,
                match_fg: Color::Black,
                active_match_bg: Color::Green,
                active_match_fg: Color::Black,

                prompt_info: Color::DarkGrey,
                prompt_warning_bg: Color::Yellow,
                prompt_warning_fg: Color::Black,
                prompt_danger_bg: Color::Red,
                prompt_danger_fg: Color::Black,
                hotkey: Color::Cyan,
            }
        }
    }
}
