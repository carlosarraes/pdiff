use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::diff::model::LineType;
use crate::vim::mode::Mode;

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let chunks = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);

    render_diff(frame, chunks[0], app);
    render_status_bar(frame, chunks[1], app);
    render_command_line(frame, chunks[2], app);
}

fn render_diff(frame: &mut Frame, area: Rect, app: &App) {
    if app.show_file_list {
        let flist_width = (area.width as f32 * 0.15).max(16.0).min(30.0) as u16;
        let content_width = area.width.saturating_sub(flist_width + 2);
        let half_content = content_width / 2;

        let chunks = Layout::horizontal([
            Constraint::Length(flist_width),
            Constraint::Length(1),
            Constraint::Length(half_content),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(area);

        render_file_list(frame, chunks[0], app, flist_width);
        render_separator(frame, chunks[1], app);
        render_diff_panels(frame, chunks[2], chunks[3], chunks[4], app);
    } else {
        let half = area.width / 2;
        let chunks = Layout::horizontal([
            Constraint::Length(half),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(area);

        render_diff_panels(frame, chunks[0], chunks[1], chunks[2], app);
    }
}

fn render_file_list(frame: &mut Frame, area: Rect, app: &App, width: u16) {
    let active_file = app.active_file_idx();
    let counts = &app.line_counts;
    let max_w = width as usize;

    let mut lines = Vec::new();

    let total_adds: usize = counts.iter().map(|(a, _)| a).sum();
    let total_dels: usize = counts.iter().map(|(_, d)| d).sum();
    lines.push(Line::from(vec![
        Span::styled(format!("{} files ", app.files.len()), app.theme.file_header),
        Span::styled(format!("+{}", total_adds), app.theme.line_style(&LineType::Addition)),
        Span::styled(" ", Style::default()),
        Span::styled(format!("-{}", total_dels), app.theme.line_style(&LineType::Deletion)),
    ]));
    lines.push(Line::default());

    for (i, file) in app.files.iter().enumerate() {
        let is_active = active_file == Some(i);
        let (adds, dels) = counts[i];

        let style = if is_active {
            app.theme.file_list_active
        } else {
            app.theme.file_list_item
        };

        let display_name = short_filename(&file.path, max_w.saturating_sub(8));
        let marker = if is_active { "▶ " } else { "  " };

        let mut spans = vec![
            Span::styled(marker, style),
            Span::styled(display_name, style),
        ];

        if adds > 0 {
            spans.push(Span::styled(
                format!(" +{}", adds),
                app.theme.line_style(&LineType::Addition),
            ));
        }
        if dels > 0 {
            spans.push(Span::styled(
                format!(" -{}", dels),
                app.theme.line_style(&LineType::Deletion),
            ));
        }

        lines.push(Line::from(spans));
    }

    let para = Paragraph::new(lines).block(Block::default().borders(Borders::NONE));
    frame.render_widget(para, area);
}

fn render_separator(frame: &mut Frame, area: Rect, app: &App) {
    let sep_lines: Vec<Line> = (0..area.height)
        .map(|_| Line::from(Span::styled("│", app.theme.border)))
        .collect();
    frame.render_widget(Paragraph::new(sep_lines), area);
}

fn render_diff_panels(
    frame: &mut Frame,
    left_area: Rect,
    sep_area: Rect,
    right_area: Rect,
    app: &App,
) {
    let viewport_height = left_area.height as usize;
    let selection = app.selection_range();
    let start = app.scroll_offset;

    let mut left_lines = Vec::new();
    let mut right_lines = Vec::new();

    let mut last_file_idx: Option<usize> = None;
    let mut last_hunk_idx: Option<(usize, usize)> = None;
    let mut flat_idx: usize = start;
    let mut rendered_rows: usize = 0;

    while flat_idx < app.flat_lines.len() && rendered_rows < viewport_height {
        let fl = &app.flat_lines[flat_idx];
        let diff_line = match app.get_line(flat_idx) {
            Some(l) => l,
            None => break,
        };

        let is_cursor = flat_idx == app.cursor;
        let is_selected = selection.is_some_and(|(s, e)| flat_idx >= s && flat_idx <= e);

        if last_file_idx != Some(fl.file_idx) {
            if rendered_rows >= viewport_height {
                break;
            }
            let file = &app.files[fl.file_idx];
            let header_text = match &file.old_path {
                Some(old) => format!(" {} → {}", old, file.path),
                None => format!(" {}", file.path),
            };
            left_lines.push(Line::from(Span::styled(header_text.clone(), app.theme.file_header)));
            right_lines.push(Line::from(Span::styled(header_text, app.theme.file_header)));
            rendered_rows += 1;
            last_file_idx = Some(fl.file_idx);
            last_hunk_idx = None;
        }

        if last_hunk_idx != Some((fl.file_idx, fl.hunk_idx)) && fl.line_idx == 0 {
            if rendered_rows >= viewport_height {
                break;
            }
            let hunk = &app.files[fl.file_idx].hunks[fl.hunk_idx];
            left_lines.push(Line::from(Span::styled(hunk.header.clone(), app.theme.hunk_header)));
            right_lines.push(Line::from(Span::styled(hunk.header.clone(), app.theme.hunk_header)));
            rendered_rows += 1;
            last_hunk_idx = Some((fl.file_idx, fl.hunk_idx));
        }

        if rendered_rows >= viewport_height {
            break;
        }

        let line_style = app.theme.line_style(&diff_line.kind);
        let lineno_style = app.theme.lineno_style(&diff_line.kind);

        let has_annotation = app
            .annotations
            .iter()
            .any(|a| flat_idx >= a.flat_start && flat_idx <= a.flat_end);

        let (annotation_marker, marker_style) = if has_annotation {
            ("● ", app.theme.comment_indicator)
        } else {
            ("  ", Style::default())
        };

        let file_path = &app.files[fl.file_idx].path;
        let content_spans = build_content_spans(
            &diff_line.content,
            file_path,
            line_style,
            is_cursor,
            is_selected,
            &app.theme,
            &app.highlighter,
        );

        match diff_line.kind {
            LineType::Context => {
                let old_no = format_lineno(diff_line.old_lineno);
                let new_no = format_lineno(diff_line.new_lineno);

                let mut left_spans = vec![
                    Span::styled(old_no, lineno_style),
                    Span::styled(annotation_marker, marker_style),
                ];
                left_spans.extend(content_spans.clone());
                left_lines.push(Line::from(left_spans));

                let mut right_spans = vec![
                    Span::styled(new_no, lineno_style),
                    Span::styled("  ", Style::default()),
                ];
                right_spans.extend(content_spans);
                right_lines.push(Line::from(right_spans));
            }
            LineType::Deletion => {
                let old_no = format_lineno(diff_line.old_lineno);

                let mut left_spans = vec![
                    Span::styled(old_no, lineno_style),
                    Span::styled(annotation_marker, marker_style),
                ];
                left_spans.extend(content_spans);
                left_lines.push(Line::from(left_spans));
                right_lines.push(Line::default());
            }
            LineType::Addition => {
                let new_no = format_lineno(diff_line.new_lineno);

                left_lines.push(Line::default());
                let mut right_spans = vec![
                    Span::styled(new_no, lineno_style),
                    Span::styled(annotation_marker, marker_style),
                ];
                right_spans.extend(content_spans);
                right_lines.push(Line::from(right_spans));
            }
        }
        rendered_rows += 1;
        flat_idx += 1;
    }

    let left_para = Paragraph::new(left_lines).block(Block::default().borders(Borders::NONE));
    frame.render_widget(left_para, left_area);

    render_separator(frame, sep_area, app);

    let right_para = Paragraph::new(right_lines).block(Block::default().borders(Borders::NONE));
    frame.render_widget(right_para, right_area);
}

fn short_filename(path: &str, max_width: usize) -> String {
    if path.len() <= max_width {
        return path.to_string();
    }
    if let Some(name) = path.rsplit('/').next() {
        if name.len() >= max_width.saturating_sub(1) {
            format!("…{}", &name[name.len().saturating_sub(max_width - 1)..])
        } else {
            format!("…/{}", name)
        }
    } else {
        path[..max_width].to_string()
    }
}

fn render_status_bar(frame: &mut Frame, area: Rect, app: &App) {
    let mode_style = app.theme.mode_style(&app.mode);

    let file_info = app
        .flat_lines
        .get(app.cursor)
        .map(|fl| app.files[fl.file_idx].path.as_str())
        .unwrap_or("");

    let annotations_count = if app.annotations.is_empty() {
        String::new()
    } else {
        format!(" [{}]", app.annotations.len())
    };

    let search_info = if !app.search_query.is_empty() && !app.search_matches.is_empty() {
        let pos = app.search_matches.partition_point(|&m| m <= app.cursor);
        format!(" /{} ({}/{})", app.search_query, pos, app.search_matches.len())
    } else {
        String::new()
    };

    let bar = Line::from(vec![
        Span::styled(format!(" {} ", app.mode.label()), mode_style),
        Span::styled(format!(" {} ", file_info), app.theme.status_bar),
        Span::styled(annotations_count, app.theme.comment_indicator),
        Span::styled(search_info, app.theme.status_bar),
        Span::styled(
            format!(" {}/{} ", app.cursor + 1, app.flat_lines.len()),
            app.theme.status_bar,
        ),
    ]);

    frame.render_widget(Paragraph::new(bar), area);
}

fn render_command_line(frame: &mut Frame, area: Rect, app: &App) {
    let content = match &app.mode {
        Mode::Command => Line::from(vec![
            Span::raw("/"),
            Span::raw(&app.search_query),
            Span::styled("█", Style::default()),
        ]),
        Mode::Comment => Line::from(vec![
            Span::styled("comment: ", app.theme.comment_indicator),
            Span::raw(&app.comment_buf),
            Span::styled("█", Style::default()),
        ]),
        _ => {
            let hints = match &app.mode {
                Mode::Normal => {
                    "q:quit  V:visual  /:search  ]c/[c:hunk  H/L:file  e:filelist  Tab:layout"
                }
                Mode::VisualLine { .. } => "c:comment  Esc:cancel  j/k:extend",
                _ => "",
            };
            Line::from(Span::styled(hints, app.theme.border))
        }
    };

    frame.render_widget(Paragraph::new(content), area);
}

fn build_content_spans(
    content: &str,
    file_path: &str,
    line_style: Style,
    is_cursor: bool,
    is_selected: bool,
    theme: &crate::ui::theme::Theme,
    highlighter: &crate::ui::highlight::Highlighter,
) -> Vec<Span<'static>> {
    if is_cursor {
        vec![Span::styled(
            content.to_string(),
            line_style.add_modifier(Modifier::REVERSED),
        )]
    } else if is_selected {
        vec![Span::styled(content.to_string(), theme.selection)]
    } else {
        let hl_spans = highlighter.highlight_line(content, file_path);
        if hl_spans.is_empty() {
            vec![Span::styled(content.to_string(), line_style)]
        } else {
            hl_spans
                .into_iter()
                .map(|span| {
                    let mut style = span.style;
                    if let Some(bg) = line_style.bg {
                        style = style.bg(bg);
                    }
                    Span::styled(span.content.into_owned(), style)
                })
                .collect()
        }
    }
}

fn format_lineno(lineno: Option<u32>) -> String {
    match lineno {
        Some(n) => format!("{:>4} ", n),
        None => "     ".to_string(),
    }
}
