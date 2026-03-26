use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{DefaultTerminal, Frame};

use crate::annotations::model::Annotation;
use crate::diff::model::{DiffFile, DiffLine};
use crate::ui::highlight::Highlighter;
use crate::ui::theme::Theme;
use crate::vim::mode::Mode;

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Side {
    Left,
    Right,
}

pub struct App {
    pub files: Vec<DiffFile>,
    pub flat_lines: Vec<FlatLine>,
    pub file_starts: Vec<usize>,
    pub line_counts: Vec<(usize, usize)>,
    pub cursor: usize,
    pub scroll_offset: usize,
    pub mode: Mode,
    pub focus_side: Side,
    pub annotations: Vec<Annotation>,
    pub layout: ViewLayout,
    pub theme: Theme,
    pub highlighter: Highlighter,
    pub should_quit: bool,
    pub comment_buf: String,
    pub search_query: String,
    pub search_matches: Vec<usize>,
    pub pending_keys: Vec<char>,
    pub comment_selection: Option<(usize, usize)>,
    pub editing_annotation: Option<usize>,
    pub show_file_list: bool,
    pub show_comments: bool,
    pub focus_mode: bool,
}

impl App {
    pub fn new(files: Vec<DiffFile>) -> Self {
        let flat_lines = build_flat_lines(&files);
        let file_starts = build_file_starts(&flat_lines);
        let line_counts = files.iter().map(|f| f.line_counts()).collect();
        let highlighter = Highlighter::new(&files);
        Self {
            files,
            flat_lines,
            file_starts,
            line_counts,
            cursor: 0,
            scroll_offset: 0,
            mode: Mode::Normal,
            focus_side: Side::Right,
            annotations: Vec::new(),
            layout: ViewLayout::SideBySide,
            theme: Theme::default(),
            highlighter,
            should_quit: false,
            comment_buf: String::new(),
            search_query: String::new(),
            search_matches: Vec::new(),
            pending_keys: Vec::new(),
            comment_selection: None,
            editing_annotation: None,
            show_file_list: true,
            show_comments: false,
            focus_mode: false,
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

    pub fn active_file_idx(&self) -> Option<usize> {
        self.flat_lines.get(self.cursor).map(|fl| fl.file_idx)
    }

    fn clamp_cursor(&mut self) {
        self.cursor = self.cursor.min(self.flat_lines.len().saturating_sub(1));
    }

    fn ensure_visible(&mut self, viewport_height: usize) {
        if self.cursor < self.scroll_offset {
            self.scroll_offset = self.cursor;
        } else if self.rendered_rows_between(self.scroll_offset, self.cursor) > viewport_height {
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

    pub fn rendered_rows_between(&self, from: usize, to: usize) -> usize {
        if self.flat_lines.is_empty() || from >= self.flat_lines.len() {
            return 0;
        }
        let mut rows = 0;
        let mut last_file: Option<usize> = None;
        let mut last_hunk: Option<(usize, usize)> = None;
        let end = to.min(self.flat_lines.len() - 1);

        for (i, fl) in self.flat_lines[from..=end].iter().enumerate() {
            let flat_idx = from + i;
            if last_file != Some(fl.file_idx) {
                rows += 1;
                last_file = Some(fl.file_idx);
                last_hunk = None;
            }
            if last_hunk != Some((fl.file_idx, fl.hunk_idx)) && fl.line_idx == 0 {
                rows += 1;
                last_hunk = Some((fl.file_idx, fl.hunk_idx));
            }
            rows += 1;

            // Count expanded comment rows (only for lines before the target,
            // since comments render after their line)
            if self.show_comments && flat_idx < to {
                if let Some(ann) = self.annotations.iter().find(|a| flat_idx >= a.flat_start && flat_idx <= a.flat_end) {
                    if flat_idx == ann.flat_end {
                        rows += ann.comment.lines().count();
                    }
                }
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
        let viewport_height = area.height.saturating_sub(2) as usize;
        self.ensure_visible(viewport_height);

        crate::ui::side_by_side::render(frame, area, self);
    }

    fn handle_key(&mut self, key: KeyEvent, viewport_height: usize) {
        let content_height = viewport_height.saturating_sub(2);

        match &self.mode {
            Mode::CommentInsert => self.handle_comment_insert_key(key),
            Mode::CommentNormal => self.handle_comment_normal_key(key),
            Mode::Command => self.handle_command_key(key),
            _ => self.handle_nav_key(key, content_height),
        }
    }

    fn handle_nav_key(&mut self, key: KeyEvent, viewport_height: usize) {
        let half_page = viewport_height / 2;

        if !self.pending_keys.is_empty() {
            if let KeyCode::Char(c) = key.code {
                let first = self.pending_keys[0];
                self.pending_keys.clear();
                if first == 'g' && c == 'g' {
                    self.cursor = 0;
                }
            } else {
                self.pending_keys.clear();
            }
            return;
        }

        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('j') | KeyCode::Down => {
                if self.cursor < self.flat_lines.len().saturating_sub(1) {
                    self.cursor += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.cursor = self.cursor.saturating_sub(1);
            }
            KeyCode::Char('h') => self.focus_side = Side::Left,
            KeyCode::Char('l') => self.focus_side = Side::Right,
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cursor =
                    (self.cursor + half_page).min(self.flat_lines.len().saturating_sub(1));
                self.center_scroll(viewport_height);
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cursor = self.cursor.saturating_sub(half_page);
                self.center_scroll(viewport_height);
            }
            KeyCode::Char('G') => {
                self.cursor = self.flat_lines.len().saturating_sub(1);
            }
            KeyCode::Char('L') if matches!(self.mode, Mode::Normal) => {
                self.jump_next_file();
                self.center_scroll(viewport_height);
            }
            KeyCode::Char('H') if matches!(self.mode, Mode::Normal) => {
                self.jump_prev_file();
                self.center_scroll(viewport_height);
            }
            KeyCode::Char('e') if matches!(self.mode, Mode::Normal) => {
                self.show_file_list = !self.show_file_list;
            }
            KeyCode::Char('F') if matches!(self.mode, Mode::Normal) => {
                self.focus_mode = !self.focus_mode;
                if self.focus_mode {
                    self.snap_cursor_to_visible_line();
                }
            }
            KeyCode::Char('g') => {
                self.pending_keys.push('g');
            }
            KeyCode::Char(']') => self.jump_next_hunk(),
            KeyCode::Char('[') => self.jump_prev_hunk(),
            KeyCode::Char('V') if matches!(self.mode, Mode::Normal) => {
                self.mode = Mode::VisualLine {
                    anchor: self.cursor,
                };
            }
            KeyCode::Char('c')
                if matches!(self.mode, Mode::Normal | Mode::VisualLine { .. }) =>
            {
                let range = if matches!(self.mode, Mode::VisualLine { .. }) {
                    self.selection_range().unwrap_or((self.cursor, self.cursor))
                } else {
                    (self.cursor, self.cursor)
                };

                // Check if an existing annotation covers this range
                // Only edit if the selection exactly matches an existing annotation
                let existing = self.annotations.iter().position(|ann| {
                    ann.flat_start == range.0 && ann.flat_end == range.1
                });
                if let Some(idx) = existing {
                    self.comment_buf = self.annotations[idx].comment.clone();
                    self.comment_selection =
                        Some((self.annotations[idx].flat_start, self.annotations[idx].flat_end));
                    self.editing_annotation = Some(idx);
                } else {
                    self.comment_buf.clear();
                    self.comment_selection = Some(range);
                    self.editing_annotation = None;
                }
                self.mode = Mode::CommentInsert;
            }
            KeyCode::Char('E') if matches!(self.mode, Mode::Normal) => {
                self.show_comments = !self.show_comments;
            }
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Char('/') if matches!(self.mode, Mode::Normal) => {
                self.mode = Mode::Command;
                self.search_query.clear();
            }
            KeyCode::Char('n') if matches!(self.mode, Mode::Normal) => {
                self.jump_next_search_match();
                self.center_scroll(viewport_height);
            }
            KeyCode::Char('N') if matches!(self.mode, Mode::Normal) => {
                self.jump_prev_search_match();
                self.center_scroll(viewport_height);
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

    fn handle_comment_insert_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                if self.comment_buf.is_empty() {
                    self.mode = Mode::Normal;
                } else {
                    self.mode = Mode::CommentNormal;
                }
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
            KeyCode::Char(c) => self.comment_buf.push(c),
            _ => {}
        }
    }

    fn handle_comment_normal_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.comment_buf.clear();
                self.mode = Mode::Normal;
            }
            KeyCode::Char('a') | KeyCode::Char('i') => {
                self.mode = Mode::CommentInsert;
            }
            KeyCode::Enter | KeyCode::Char('c') => {
                self.submit_comment();
                self.mode = Mode::Normal;
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
            KeyCode::Char(c) => self.search_query.push(c),
            _ => {}
        }
    }

    fn submit_comment(&mut self) {
        if self.comment_buf.trim().is_empty() {
            // If editing and user cleared the comment, delete the annotation
            if let Some(idx) = self.editing_annotation.take() {
                if idx < self.annotations.len() {
                    self.annotations.remove(idx);
                }
            }
            self.comment_buf.clear();
            return;
        }

        // If editing an existing annotation, update in place
        if let Some(idx) = self.editing_annotation.take() {
            if idx < self.annotations.len() {
                self.annotations[idx].comment = self.comment_buf.clone();
                self.comment_buf.clear();
                return;
            }
        }

        let (start, end) = match self.comment_selection.take() {
            Some(range) => range,
            None => (self.cursor, self.cursor),
        };

        let start_fl = match self.flat_lines.get(start) {
            Some(fl) => *fl,
            None => return,
        };
        let file = match self.files.get(start_fl.file_idx) {
            Some(f) => f.path.clone(),
            None => return,
        };

        let clamped_end = (start..=end)
            .rev()
            .find(|&i| {
                self.flat_lines
                    .get(i)
                    .is_some_and(|fl| fl.file_idx == start_fl.file_idx)
            })
            .unwrap_or(start);

        let mut context_lines = Vec::new();
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
                context_lines.push(format!("{}{}", line.kind.prefix(), line.content));
            }
        }

        self.annotations.push(Annotation {
            file,
            flat_start: start,
            flat_end: clamped_end,
            display_range: build_display_range(&old_lines, &new_lines),
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
        for (i, fl) in self.flat_lines.iter().enumerate() {
            if let Some(content) = self
                .files
                .get(fl.file_idx)
                .and_then(|f| f.hunks.get(fl.hunk_idx))
                .and_then(|h| h.lines.get(fl.line_idx))
                .map(|l| &l.content)
            {
                if content.to_lowercase().contains(&query) {
                    self.search_matches.push(i);
                }
            }
        }
    }

    fn jump_next_search_match(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        let pos = self
            .search_matches
            .partition_point(|&m| m <= self.cursor);
        let idx = if pos < self.search_matches.len() {
            pos
        } else {
            0
        };
        self.cursor = self.search_matches[idx];
    }

    fn jump_prev_search_match(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        let pos = self.search_matches.partition_point(|&m| m < self.cursor);
        let idx = if pos > 0 {
            pos - 1
        } else {
            self.search_matches.len() - 1
        };
        self.cursor = self.search_matches[idx];
    }

    fn jump_next_hunk(&mut self) {
        if let Some(current) = self.flat_lines.get(self.cursor) {
            let (cf, ch) = (current.file_idx, current.hunk_idx);
            for (i, fl) in self.flat_lines.iter().enumerate().skip(self.cursor + 1) {
                if fl.file_idx != cf || fl.hunk_idx != ch {
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
        let Some(current) = self.flat_lines.get(self.cursor) else {
            return;
        };
        let (cf, ch) = (current.file_idx, current.hunk_idx);

        let mut prev_end = self.cursor - 1;
        while prev_end > 0 {
            let fl = &self.flat_lines[prev_end];
            if fl.file_idx != cf || fl.hunk_idx != ch {
                break;
            }
            prev_end -= 1;
        }
        let target = &self.flat_lines[prev_end];
        let (tf, th) = (target.file_idx, target.hunk_idx);
        let start = self.flat_lines[..=prev_end]
            .iter()
            .rposition(|fl| fl.file_idx != tf || fl.hunk_idx != th)
            .map(|i| i + 1)
            .unwrap_or(0);
        self.cursor = start;
    }

    fn jump_next_file(&mut self) {
        let pos = self.file_starts.partition_point(|&s| s <= self.cursor);
        if pos < self.file_starts.len() {
            self.cursor = self.file_starts[pos];
        }
    }

    fn jump_prev_file(&mut self) {
        let pos = self.file_starts.partition_point(|&s| s <= self.cursor);
        if pos >= 2 {
            self.cursor = self.file_starts[pos - 2];
        } else if pos == 1 {
            self.cursor = self.file_starts[0];
        }
    }

    fn snap_cursor_to_visible_line(&mut self) {
        use crate::diff::model::LineType;
        let is_left = self.focus_side == Side::Left;

        if let Some(line) = self.get_line(self.cursor) {
            let hidden = (is_left && line.kind == LineType::Addition)
                || (!is_left && line.kind == LineType::Deletion);
            if hidden {
                // Search forward first, then backward
                let forward = (self.cursor + 1..self.flat_lines.len()).find(|&i| {
                    self.get_line(i).is_some_and(|l| {
                        !((is_left && l.kind == LineType::Addition)
                            || (!is_left && l.kind == LineType::Deletion))
                    })
                });
                let backward = (0..self.cursor).rev().find(|&i| {
                    self.get_line(i).is_some_and(|l| {
                        !((is_left && l.kind == LineType::Addition)
                            || (!is_left && l.kind == LineType::Deletion))
                    })
                });
                self.cursor = forward.or(backward).unwrap_or(self.cursor);
            }
        }
    }

    fn center_scroll(&mut self, viewport_height: usize) {
        let half = viewport_height / 2;
        self.scroll_offset = self.cursor.saturating_sub(half);
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
            for li in 0..hunk.lines.len() {
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

fn build_file_starts(flat_lines: &[FlatLine]) -> Vec<usize> {
    let mut starts = Vec::new();
    let mut last_file = None;
    for (i, fl) in flat_lines.iter().enumerate() {
        if last_file != Some(fl.file_idx) {
            starts.push(i);
            last_file = Some(fl.file_idx);
        }
    }
    starts
}
