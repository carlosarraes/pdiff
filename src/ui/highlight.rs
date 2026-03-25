use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use syntect::easy::HighlightLines;
use syntect::highlighting::{self, ThemeSet};
use syntect::parsing::SyntaxSet;


pub struct Highlighter {
    syntax_set: SyntaxSet,
    theme: highlighting::Theme,
}

impl Highlighter {
    pub fn new() -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
        let theme = theme_set.themes["base16-eighties.dark"].clone();
        Self { syntax_set, theme }
    }

    /// Highlight a single line of code, returning styled spans.
    /// Falls back to unstyled if the extension is unknown.
    pub fn highlight_line(&self, content: &str, file_path: &str) -> Vec<Span<'static>> {
        let ext = file_path.rsplit('.').next().unwrap_or("");
        let syntax = self
            .syntax_set
            .find_syntax_by_extension(ext)
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let mut h = HighlightLines::new(syntax, &self.theme);

        // syntect needs a trailing newline
        let line_with_nl = format!("{}\n", content);
        let regions = match h.highlight_line(&line_with_nl, &self.syntax_set) {
            Ok(r) => r,
            Err(_) => return vec![Span::raw(content.to_string())],
        };

        regions
            .into_iter()
            .filter_map(|(style, text)| {
                let text = text.trim_end_matches('\n');
                if text.is_empty() {
                    return None;
                }
                Some(Span::styled(
                    text.to_string(),
                    syntect_to_ratatui_style(style),
                ))
            })
            .collect()
    }
}

fn syntect_to_ratatui_style(style: highlighting::Style) -> Style {
    let fg = Color::Rgb(
        style.foreground.r,
        style.foreground.g,
        style.foreground.b,
    );

    let mut s = Style::default().fg(fg);
    if style.font_style.contains(highlighting::FontStyle::BOLD) {
        s = s.add_modifier(Modifier::BOLD);
    }
    if style.font_style.contains(highlighting::FontStyle::ITALIC) {
        s = s.add_modifier(Modifier::ITALIC);
    }
    if style.font_style.contains(highlighting::FontStyle::UNDERLINE) {
        s = s.add_modifier(Modifier::UNDERLINED);
    }
    s
}
