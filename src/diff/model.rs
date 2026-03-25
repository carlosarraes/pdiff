#[derive(Debug, Clone, PartialEq)]
pub enum LineType {
    Context,
    Addition,
    Deletion,
}

impl LineType {
    pub fn prefix(&self) -> &'static str {
        match self {
            LineType::Addition => "+",
            LineType::Deletion => "-",
            LineType::Context => " ",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiffLine {
    pub kind: LineType,
    pub content: String,
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct Hunk {
    pub old_start: u32,
    pub new_start: u32,
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone)]
pub struct DiffFile {
    pub path: String,
    pub old_path: Option<String>,
    pub hunks: Vec<Hunk>,
    pub is_new: bool,
    pub is_deleted: bool,
    pub is_binary: bool,
}

impl DiffFile {
    pub fn line_counts(&self) -> (usize, usize) {
        let mut adds = 0usize;
        let mut dels = 0usize;
        for hunk in &self.hunks {
            for line in &hunk.lines {
                match line.kind {
                    LineType::Addition => adds += 1,
                    LineType::Deletion => dels += 1,
                    LineType::Context => {}
                }
            }
        }
        (adds, dels)
    }
}
