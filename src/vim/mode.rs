#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Normal,
    VisualLine { anchor: usize },
    VisualBlock { anchor: (usize, usize) },
    Comment,
    Command,
}

impl Mode {
    pub fn label(&self) -> &str {
        match self {
            Mode::Normal => "NORMAL",
            Mode::VisualLine { .. } => "V-LINE",
            Mode::VisualBlock { .. } => "V-BLOCK",
            Mode::Comment => "COMMENT",
            Mode::Command => "COMMAND",
        }
    }
}
