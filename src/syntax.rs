use ratatui::style::{Color, Style};
use ratatui::text::Span;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style as SynStyle, ThemeSet};
use syntect::parsing::SyntaxSet;

pub struct Highlighter {
    syntax_set: SyntaxSet,
    theme: syntect::highlighting::Theme,
}

impl Highlighter {
    pub fn new(theme_name: &str) -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
        let theme = theme_set
            .themes
            .get(theme_name)
            .unwrap_or(&theme_set.themes["base16-eighties.dark"])
            .clone();
        Highlighter { syntax_set, theme }
    }

    /// Highlight a line of code, returning styled spans.
    /// `bg` is the background color from the diff context (hunk/line highlight).
    /// Returns None if no syntax is found for the extension.
    pub fn highlight_line<'a>(
        &self,
        content: &'a str,
        extension: &str,
        bg: Color,
    ) -> Option<Vec<Span<'a>>> {
        let syntax = self.syntax_set.find_syntax_by_extension(extension)?;
        let mut h = HighlightLines::new(syntax, &self.theme);
        let regions = h.highlight_line(content, &self.syntax_set).ok()?;

        let theme_bg = self.theme.settings.background;
        let theme_fg = self.theme.settings.foreground;

        let spans = regions
            .into_iter()
            .map(|(style, text)| {
                let fg = syn_color_to_ratatui_checked(style, theme_bg, theme_fg);
                Span::styled(text, Style::default().fg(fg).bg(bg))
            })
            .collect();
        Some(spans)
    }
}

/// Convert syntect foreground color to ratatui, substituting the theme's default
/// foreground when the color is too close to the theme background (which would
/// make text invisible). Some syntect themes assign their background color as
/// the foreground for certain tokens (e.g. `)` in JavaScript with base16-eighties.dark).
fn syn_color_to_ratatui_checked(
    style: SynStyle,
    theme_bg: Option<syntect::highlighting::Color>,
    theme_fg: Option<syntect::highlighting::Color>,
) -> Color {
    let c = style.foreground;
    if let Some(bg) = theme_bg {
        let dist_sq = (c.r as i32 - bg.r as i32).pow(2)
            + (c.g as i32 - bg.g as i32).pow(2)
            + (c.b as i32 - bg.b as i32).pow(2);
        if dist_sq < 400 {
            // Too close to background — use theme default foreground
            return theme_fg
                .map(|fc| Color::Rgb(fc.r, fc.g, fc.b))
                .unwrap_or(Color::White);
        }
    }
    Color::Rgb(c.r, c.g, c.b)
}

/// Extract the file extension from a path.
pub fn file_extension(path: &str) -> Option<&str> {
    std::path::Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_extension_rs() {
        assert_eq!(file_extension("src/main.rs"), Some("rs"));
    }

    #[test]
    fn test_file_extension_nested_path() {
        assert_eq!(file_extension("src/git/diff.rs"), Some("rs"));
    }

    #[test]
    fn test_file_extension_no_ext() {
        assert_eq!(file_extension("Makefile"), None);
    }

    #[test]
    fn test_file_extension_hidden_file() {
        assert_eq!(file_extension(".gitignore"), None);
    }

    #[test]
    fn test_file_extension_dotfile_with_ext() {
        assert_eq!(file_extension(".config.toml"), Some("toml"));
    }

    #[test]
    fn test_file_extension_multiple_dots() {
        assert_eq!(file_extension("archive.tar.gz"), Some("gz"));
    }

    #[test]
    fn test_file_extension_js() {
        assert_eq!(file_extension("index.js"), Some("js"));
    }
}
