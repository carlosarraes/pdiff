#[derive(Debug, Clone, PartialEq)]
pub enum LineType {
    Context,
    Addition,
    Deletion,
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
    pub old_count: u32,
    pub new_start: u32,
    pub new_count: u32,
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
