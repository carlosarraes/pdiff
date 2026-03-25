use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{DefaultTerminal, Frame};

use crate::annotations::model::Annotation;
use crate::diff::model::{DiffFile, DiffLine, LineType};
use crate::ui::highlight::Highlighter;
use crate::ui::theme::Theme;
use crate::vim::mode::Mode;

/// A flat reference to a line within the diff
#[derive(Debug, Clone, Copy)]
pub struct FlatLine {
    pub file_idx: usize,
    pub hunk_idx: usize,
    pub line_idx: usize,
}

pub enum ViewLayout {
    SideBySide,
    Unified,
}

pub struct App {
    pub files: Vec<DiffFile>,
    pub flat_lines: Vec<FlatLine>,
    pub cursor: usize,
    pub scroll_offset: usize,
    pub mode: Mode,
    pub annotations: Vec<Annotation>,
    pub layout: ViewLayout,
    pub theme: Theme,
    pub highlighter: Highlighter,
    pub should_quit: bool,
    pub comment_buf: String,
    pub search_query: String,
    pub search_matches: Vec<usize>,
    pub search_idx: usize,
    pub pending_keys: Vec<char>,
    pub comment_selection: Option<(usize, usize)>,
}

impl App {
    pub fn new(files: Vec<DiffFile>) -> Self {
        let flat_lines = build_flat_lines(&files);
        Self {
            files,
            flat_lines,
            cursor: 0,
            scroll_offset: 0,
            mode: Mode::Normal,
            annotations: Vec::new(),
            layout: ViewLayout::SideBySide,
            theme: Theme::default(),
            highlighter: Highlighter::new(),
            should_quit: false,
            comment_buf: String::new(),
            search_query: String::new(),
            search_matches: Vec::new(),
            search_idx: 0,
            pending_keys: Vec::new(),
            comment_selection: None,
        }
    }

    pub fn get_line(&self, flat_idx: usize) -> Option<&DiffLine> {
        let fl = self.flat_lines.get(flat_idx)?;
        self.files
            .get(fl.file_idx)?
            .hunks
            .get(fl.hunk_idx)?
            .lines
            .get(fl.line_idx)
    }

    pub fn get_flat(&self, flat_idx: usize) -> Option<&FlatLine> {
        self.flat_lines.get(flat_idx)
    }

    pub fn file_for_line(&self, flat_idx: usize) -> Option<&DiffFile> {
        let fl = self.flat_lines.get(flat_idx)?;
        self.files.get(fl.file_idx)
    }

    pub fn total_lines(&self) -> usize {
        self.flat_lines.len()
    }

    pub fn active_file_idx(&self) -> Option<usize> {
        self.flat_lines.get(self.cursor).map(|fl| fl.file_idx)
    }

    pub fn file_line_counts(&self) -> Vec<(usize, usize)> {
        // Returns (additions, deletions) per file
        self.files
            .iter()
            .map(|file| {
                let mut adds = 0usize;
                let mut dels = 0usize;
                for hunk in &file.hunks {
                    for line in &hunk.lines {
                        match line.kind {
                            crate::diff::model::LineType::Addition => adds += 1,
                            crate::diff::model::LineType::Deletion => dels += 1,
                            _ => {}
                        }
                    }
                }
                (adds, dels)
            })
            .collect()
    }

    fn clamp_cursor(&mut self) {
        let max = if self.total_lines() == 0 {
            0
        } else {
            self.total_lines() - 1
        };
        self.cursor = self.cursor.min(max);
    }

    fn ensure_visible(&mut self, viewport_height: usize) {
        if self.cursor < self.scroll_offset {
            self.scroll_offset = self.cursor;
        } else if self.rendered_rows_between(self.scroll_offset, self.cursor) > viewport_height {
            // Binary search for the smallest scroll_offset where cursor fits.
            // rendered_rows_between(offset, cursor) is monotonically non-increasing
            // as offset grows, so binary search works.
            let mut lo = self.scroll_offset;
            let mut hi = self.cursor;
            while lo < hi {
                let mid = lo + (hi - lo) / 2;
                if self.rendered_rows_between(mid, self.cursor) > viewport_height {
                    lo = mid + 1;
                } else {
                    hi = mid;
                }
            }
            self.scroll_offset = lo;
        }
    }

    fn rendered_rows_between(&self, from: usize, to: usize) -> usize {
        let mut rows = 0;
        let mut last_file: Option<usize> = None;
        let mut last_hunk: Option<(usize, usize)> = None;

        for idx in from..=to {
            if let Some(fl) = self.get_flat(idx) {
                if last_file != Some(fl.file_idx) {
                    rows += 1; // file header
                    last_file = Some(fl.file_idx);
                    last_hunk = None;
                }
                if last_hunk != Some((fl.file_idx, fl.hunk_idx)) && fl.line_idx == 0 {
                    rows += 1; // hunk header
                    last_hunk = Some((fl.file_idx, fl.hunk_idx));
                }
                rows += 1; // the line itself
            }
        }
        rows
    }

    pub fn selection_range(&self) -> Option<(usize, usize)> {
        match &self.mode {
            Mode::VisualLine { anchor } => {
                let start = (*anchor).min(self.cursor);
                let end = (*anchor).max(self.cursor);
                Some((start, end))
            }
            _ => None,
        }
    }

    pub fn run(mut self, terminal: &mut DefaultTerminal) -> io::Result<Vec<Annotation>> {
        while !self.should_quit {
            terminal.draw(|frame| self.draw(frame))?;
            if let Event::Key(key) = event::read()? {
                self.handle_key(key, terminal.size()?.height as usize);
            }
        }
        Ok(self.annotations)
    }

    fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();
        // Reserve 1 line for status bar
        let viewport_height = area.height.saturating_sub(2) as usize;
        self.ensure_visible(viewport_height);

        match self.layout {
            ViewLayout::SideBySide => {
                crate::ui::side_by_side::render(frame, area, self);
            }
            ViewLayout::Unified => {
                crate::ui::side_by_side::render(frame, area, self);
                // TODO: unified renderer
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent, viewport_height: usize) {
        let content_height = viewport_height.saturating_sub(2);

        match &self.mode {
            Mode::Comment => self.handle_comment_key(key),
            Mode::Command => self.handle_command_key(key),
            Mode::Normal | Mode::VisualLine { .. } | Mode::VisualBlock { .. } => {
                self.handle_nav_key(key, content_height);
            }
        }
    }

    fn handle_nav_key(&mut self, key: KeyEvent, viewport_height: usize) {
        let half_page = viewport_height / 2;

        // Handle 'g' prefix for gg
        if !self.pending_keys.is_empty() {
            if let KeyCode::Char(c) = key.code {
                let pending = self.pending_keys.clone();
                self.pending_keys.clear();

                if pending == ['g'] && c == 'g' {
                    self.cursor = 0;
                    return;
                }
                if pending == [']'] && c == 'c' {
                    self.jump_next_hunk();
                    return;
                }
                if pending == ['['] && c == 'c' {
                    self.jump_prev_hunk();
                    return;
                }
            } else {
                self.pending_keys.clear();
            }
            return;
        }

        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('j') | KeyCode::Down => {
                if self.cursor < self.total_lines().saturating_sub(1) {
                    self.cursor += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.cursor = self.cursor.saturating_sub(1);
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cursor = (self.cursor + half_page).min(self.total_lines().saturating_sub(1));
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cursor = self.cursor.saturating_sub(half_page);
            }
            KeyCode::Char('G') => {
                self.cursor = self.total_lines().saturating_sub(1);
            }
            KeyCode::Char('g') => {
                self.pending_keys.push('g');
            }
            KeyCode::Char(']') => {
                self.pending_keys.push(']');
            }
            KeyCode::Char('[') => {
                self.pending_keys.push('[');
            }
            KeyCode::Char('V') if matches!(self.mode, Mode::Normal) => {
                self.mode = Mode::VisualLine {
                    anchor: self.cursor,
                };
            }
            KeyCode::Char('c') if matches!(self.mode, Mode::VisualLine { .. }) => {
                self.comment_selection = self.selection_range();
                self.mode = Mode::Comment;
                self.comment_buf.clear();
            }
            KeyCode::Esc => {
                self.mode = Mode::Normal;
            }
            KeyCode::Char('/') if matches!(self.mode, Mode::Normal) => {
                self.mode = Mode::Command;
                self.search_query.clear();
            }
            KeyCode::Char('n') if matches!(self.mode, Mode::Normal) => {
                self.jump_next_search_match();
            }
            KeyCode::Char('N') if matches!(self.mode, Mode::Normal) => {
                self.jump_prev_search_match();
            }
            KeyCode::Tab => {
                self.layout = match self.layout {
                    ViewLayout::SideBySide => ViewLayout::Unified,
                    ViewLayout::Unified => ViewLayout::SideBySide,
                };
            }
            _ => {}
        }
        self.clamp_cursor();
    }

    fn handle_comment_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.comment_buf.clear();
                self.mode = Mode::Normal;
            }
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.comment_buf.push('\n');
            }
            KeyCode::Enter => {
                self.submit_comment();
                self.mode = Mode::Normal;
            }
            KeyCode::Backspace => {
                self.comment_buf.pop();
            }
            KeyCode::Char(c) => {
                self.comment_buf.push(c);
            }
            _ => {}
        }
    }

    fn handle_command_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.search_query.clear();
            }
            KeyCode::Enter => {
                self.build_search_matches();
                self.jump_next_search_match();
                self.mode = Mode::Normal;
            }
            KeyCode::Backspace => {
                self.search_query.pop();
            }
            KeyCode::Char(c) => {
                self.search_query.push(c);
            }
            _ => {}
        }
    }

    fn submit_comment(&mut self) {
        if self.comment_buf.trim().is_empty() {
            return;
        }

        let (start, end) = match self.comment_selection.take() {
            Some(range) => range,
            None => (self.cursor, self.cursor),
        };

        let start_fl = match self.get_flat(start) {
            Some(fl) => *fl,
            None => return,
        };
        let file = match self.files.get(start_fl.file_idx) {
            Some(f) => f.path.clone(),
            None => return,
        };

        // Clamp selection to same file — don't cross file boundaries
        let clamped_end = (start..=end)
            .rev()
            .find(|&i| {
                self.get_flat(i)
                    .is_some_and(|fl| fl.file_idx == start_fl.file_idx)
            })
            .unwrap_or(start);

        let mut context_lines = Vec::new();
        // Build a human-readable display range from the selected lines
        let mut old_lines: Vec<u32> = Vec::new();
        let mut new_lines: Vec<u32> = Vec::new();

        for i in start..=clamped_end {
            if let Some(line) = self.get_line(i) {
                if let Some(n) = line.old_lineno {
                    old_lines.push(n);
                }
                if let Some(n) = line.new_lineno {
                    new_lines.push(n);
                }

                let prefix = match line.kind {
                    LineType::Addition => "+",
                    LineType::Deletion => "-",
                    LineType::Context => " ",
                };
                context_lines.push(format!("{}{}", prefix, line.content));
            }
        }

        let display_range = build_display_range(&old_lines, &new_lines);

        self.annotations.push(Annotation {
            file,
            flat_start: start,
            flat_end: clamped_end,
            display_range,
            diff_context: context_lines.join("\n"),
            comment: self.comment_buf.clone(),
        });

        self.comment_buf.clear();
    }

    fn build_search_matches(&mut self) {
        self.search_matches.clear();
        if self.search_query.is_empty() {
            return;
        }
        let query = self.search_query.to_lowercase();
        for (i, _fl) in self.flat_lines.iter().enumerate() {
            if let Some(line) = self.get_line(i) {
                if line.content.to_lowercase().contains(&query) {
                    self.search_matches.push(i);
                }
            }
        }
        self.search_idx = 0;
    }

    fn jump_next_search_match(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        // Find next match after cursor
        if let Some(pos) = self.search_matches.iter().position(|&m| m > self.cursor) {
            self.search_idx = pos;
            self.cursor = self.search_matches[pos];
        } else {
            // Wrap around
            self.search_idx = 0;
            self.cursor = self.search_matches[0];
        }
    }

    fn jump_prev_search_match(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        if let Some(pos) = self
            .search_matches
            .iter()
            .rposition(|&m| m < self.cursor)
        {
            self.search_idx = pos;
            self.cursor = self.search_matches[pos];
        } else {
            let last = self.search_matches.len() - 1;
            self.search_idx = last;
            self.cursor = self.search_matches[last];
        }
    }

    fn jump_next_hunk(&mut self) {
        if let Some(current) = self.flat_lines.get(self.cursor) {
            let current_file = current.file_idx;
            let current_hunk = current.hunk_idx;

            for (i, fl) in self.flat_lines.iter().enumerate().skip(self.cursor + 1) {
                if fl.file_idx != current_file || fl.hunk_idx != current_hunk {
                    self.cursor = i;
                    return;
                }
            }
        }
    }

    fn jump_prev_hunk(&mut self) {
        if self.cursor == 0 {
            return;
        }
        if let Some(current) = self.flat_lines.get(self.cursor) {
            let current_file = current.file_idx;
            let current_hunk = current.hunk_idx;

            // Go backwards to find start of previous hunk
            let mut target_file = current_file;
            let mut target_hunk = current_hunk;

            for (i, fl) in self.flat_lines[..self.cursor].iter().enumerate().rev() {
                if fl.file_idx != target_file || fl.hunk_idx != target_hunk {
                    target_file = fl.file_idx;
                    target_hunk = fl.hunk_idx;
                    // Now find the start of this hunk
                    for (j, fl2) in self.flat_lines.iter().enumerate() {
                        if fl2.file_idx == target_file && fl2.hunk_idx == target_hunk {
                            self.cursor = j;
                            return;
                        }
                    }
                    self.cursor = i;
                    return;
                }
            }
            self.cursor = 0;
        }
    }
}

fn build_display_range(old_lines: &[u32], new_lines: &[u32]) -> String {
    let old_range = format_line_range(old_lines);
    let new_range = format_line_range(new_lines);

    match (old_range.as_deref(), new_range.as_deref()) {
        (Some(old), Some(new)) if old == new => old.to_string(),
        (Some(old), Some(new)) => format!("L{old}(old) L{new}(new)"),
        (Some(old), None) => format!("L{old}(old)"),
        (None, Some(new)) => format!("L{new}(new)"),
        (None, None) => String::new(),
    }
}

fn format_line_range(lines: &[u32]) -> Option<String> {
    let first = lines.first()?;
    let last = lines.last()?;
    if first == last {
        Some(format!("{first}"))
    } else {
        Some(format!("{first}-{last}"))
    }
}

fn build_flat_lines(files: &[DiffFile]) -> Vec<FlatLine> {
    let mut flat = Vec::new();
    for (fi, file) in files.iter().enumerate() {
        for (hi, hunk) in file.hunks.iter().enumerate() {
            for (li, _line) in hunk.lines.iter().enumerate() {
                flat.push(FlatLine {
                    file_idx: fi,
                    hunk_idx: hi,
                    line_idx: li,
                });
            }
        }
    }
    flat
}
