use ratatui::style::Color;

pub struct Theme {
    pub name: &'static str,
    // Base palette
    pub fg: Color,
    pub fg_dim: Color,
    pub bg: Color,
    pub black: Color,
    pub red: Color,
    pub green: Color,
    pub yellow: Color,
    pub blue: Color,
    pub magenta: Color,
    pub cyan: Color,
    pub gray: Color,
    pub orange: Color,

    // Diff backgrounds
    pub diff_added_bg: Color,
    pub diff_added_bg_bright: Color,
    pub diff_removed_bg: Color,
    pub diff_removed_bg_bright: Color,
    pub diff_hunk_bg: Color,
    pub diff_cursor_bg: Color,
    pub diff_selected_stage_bg: Color,
    pub diff_selected_unstage_bg: Color,
    pub diff_viewport_bg: Color,

    // Conflict
    pub conflict_ours_bg: Color,
    pub conflict_theirs_bg: Color,
    pub conflict_theirs_accent: Color,
    pub conflict_dim_fg: Color,
    pub conflict_dim_border: Color,
    pub conflict_dim_bg: Color,

    // Syntax highlighting theme (syntect)
    pub syntax_theme: String,
}

pub const THEME_NAMES: &[&str] = &["default", "dracula"];

impl Theme {
    pub fn from_name(name: &str) -> Self {
        match name {
            "dracula" => Self::dracula(),
            _ => Self::default_theme(),
        }
    }

    pub fn next_theme_name(current: &str) -> &'static str {
        let idx = THEME_NAMES.iter().position(|&n| n == current).unwrap_or(0);
        THEME_NAMES[(idx + 1) % THEME_NAMES.len()]
    }

    pub fn default_theme() -> Self {
        Self {
            name: "default",
            fg: Color::White,
            fg_dim: Color::DarkGray,
            bg: Color::Reset,
            black: Color::Black,
            red: Color::Red,
            green: Color::Green,
            yellow: Color::Yellow,
            blue: Color::Blue,
            magenta: Color::Magenta,
            cyan: Color::Cyan,
            gray: Color::Gray,
            orange: Color::Rgb(255, 200, 60),

            diff_added_bg: Color::Rgb(10, 40, 10),
            diff_added_bg_bright: Color::Rgb(20, 60, 20),
            diff_removed_bg: Color::Rgb(40, 10, 10),
            diff_removed_bg_bright: Color::Rgb(60, 20, 20),
            diff_hunk_bg: Color::Rgb(30, 30, 50),
            diff_cursor_bg: Color::Rgb(60, 55, 20),
            diff_selected_stage_bg: Color::Rgb(20, 50, 20),
            diff_selected_unstage_bg: Color::Rgb(50, 20, 20),
            diff_viewport_bg: Color::Rgb(30, 30, 40),

            conflict_ours_bg: Color::Rgb(10, 40, 50),
            conflict_theirs_bg: Color::Rgb(40, 10, 50),
            conflict_theirs_accent: Color::Rgb(255, 100, 255),
            conflict_dim_fg: Color::Rgb(100, 100, 100),
            conflict_dim_border: Color::Rgb(70, 70, 70),
            conflict_dim_bg: Color::Rgb(15, 15, 15),

            syntax_theme: "base16-eighties.dark".into(),
        }
    }

    pub fn dracula() -> Self {
        Self {
            name: "dracula",
            fg: Color::Rgb(248, 248, 242),
            fg_dim: Color::Rgb(98, 114, 164),
            bg: Color::Rgb(40, 42, 54),
            black: Color::Rgb(40, 42, 54),
            red: Color::Rgb(255, 85, 85),
            green: Color::Rgb(80, 250, 123),
            yellow: Color::Rgb(241, 250, 140),
            blue: Color::Rgb(189, 147, 249),
            magenta: Color::Rgb(255, 121, 198),
            cyan: Color::Rgb(139, 233, 253),
            gray: Color::Rgb(68, 71, 90),
            orange: Color::Rgb(255, 184, 108),

            diff_added_bg: Color::Rgb(20, 55, 20),
            diff_added_bg_bright: Color::Rgb(30, 75, 30),
            diff_removed_bg: Color::Rgb(55, 20, 20),
            diff_removed_bg_bright: Color::Rgb(75, 30, 30),
            diff_hunk_bg: Color::Rgb(44, 47, 75),
            diff_cursor_bg: Color::Rgb(68, 71, 90),
            diff_selected_stage_bg: Color::Rgb(30, 65, 30),
            diff_selected_unstage_bg: Color::Rgb(65, 30, 30),
            diff_viewport_bg: Color::Rgb(44, 47, 60),

            conflict_ours_bg: Color::Rgb(20, 50, 65),
            conflict_theirs_bg: Color::Rgb(55, 20, 60),
            conflict_theirs_accent: Color::Rgb(255, 121, 198),
            conflict_dim_fg: Color::Rgb(98, 114, 164),
            conflict_dim_border: Color::Rgb(68, 71, 90),
            conflict_dim_bg: Color::Rgb(30, 32, 40),

            syntax_theme: "Dracula".into(),
        }
    }

    pub fn from_env() -> Self {
        match std::env::var("STAGE_RS_THEME").as_deref() {
            Ok("dracula") => Self::dracula(),
            _ => Self::default_theme(),
        }
    }
}
