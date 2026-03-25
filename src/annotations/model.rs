#[derive(Debug, Clone)]
pub struct Annotation {
    pub file: String,
    /// Flat line indices (for UI marker matching)
    pub flat_start: usize,
    pub flat_end: usize,
    /// Human-readable line range for markdown output
    pub display_range: String,
    pub diff_context: String,
    pub comment: String,
}
