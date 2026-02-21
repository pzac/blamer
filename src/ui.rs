use std::collections::HashMap;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};
use crate::app::App;
use crate::git::CommitDetails;

const COMMIT_COLORS: &[Color] = &[
    Color::Cyan,
    Color::LightGreen,
    Color::Yellow,
    Color::LightMagenta,
    Color::LightBlue,
    Color::LightRed,
    Color::LightCyan,
    Color::Green,
    Color::LightYellow,
    Color::Magenta,
];

pub fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(f.area());

    // Header
    let header_text = match &app.current_commit_label {
        Some(label) => format!("Git Blame: {} @ {}", app.filename, label),
        None => format!("Git Blame: {}", app.filename),
    };
    let header = Paragraph::new(header_text)
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);

    // Blame content
    let visible_height = chunks[1].height.saturating_sub(2) as usize;
    let end_idx = (app.scroll_offset + visible_height).min(app.lines.len());

    // Assign a stable color to each unique commit (by order of first appearance)
    let mut commit_color_map: HashMap<&str, usize> = HashMap::new();
    for line in &app.lines {
        let next = commit_color_map.len();
        commit_color_map.entry(line.full_commit_id.as_str()).or_insert(next);
    }

    let items: Vec<ListItem> = app.lines[app.scroll_offset..end_idx]
        .iter()
        .enumerate()
        .map(|(idx, blame_line)| {
            let actual_line_idx = app.scroll_offset + idx;
            let is_selected = actual_line_idx == app.selected_line;

            let color_idx = commit_color_map.get(blame_line.full_commit_id.as_str()).copied().unwrap_or(0);
            let commit_color = COMMIT_COLORS[color_idx % COMMIT_COLORS.len()];

            let mut base_style = Style::default().fg(commit_color);
            if is_selected {
                base_style = base_style.bg(Color::DarkGray);
            }

            let initials = author_initials(&blame_line.author);
            let summary = truncate(&blame_line.summary, 30);
            let content_style = if is_selected { Style::default().bg(Color::DarkGray) } else { Style::default() };
            let line_content = vec![
                Span::styled(format!("{:4} ", blame_line.line_num), base_style.fg(Color::DarkGray)),
                Span::styled(format!("{:3} ", initials), base_style),
                Span::styled(format!("{:10} ", blame_line.date), base_style),
                Span::styled(format!("{:30} ", summary), base_style),
                Span::styled(&blame_line.content, content_style),
            ];
            ListItem::new(Line::from(line_content))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Blame"));
    f.render_widget(list, chunks[1]);

    // Footer
    let footer_text = format!(
        "Lines {}-{}/{} | ↑/↓: scroll | ←/→: history | Space: commit details | q: quit",
        app.scroll_offset + 1,
        end_idx,
        app.lines.len()
    );
    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::Gray))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);

    // Commit details popup
    if app.show_commit_details {
        if let Some(details) = &app.commit_details {
            render_commit_popup(f, details);
        }
    }
}

fn render_commit_popup(f: &mut Frame, details: &CommitDetails) {
    let area = f.area();
    let popup_width = area.width.saturating_sub(10).min(100);
    let popup_height = area.height.saturating_sub(10).min(30);

    let popup_area = Rect {
        x: (area.width.saturating_sub(popup_width)) / 2,
        y: (area.height.saturating_sub(popup_height)) / 2,
        width: popup_width,
        height: popup_height,
    };

    f.render_widget(Clear, popup_area);

    let message_lines: Vec<Line> = details.message.lines()
        .map(|line| Line::from(line.to_string()))
        .collect();

    let mut content = vec![
        Line::from(vec![
            Span::styled("Commit: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(&details.sha),
        ]),
        Line::from(vec![
            Span::styled("Author: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!("{} <{}>", details.author, details.author_email)),
        ]),
        Line::from(vec![
            Span::styled("Date: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(&details.date),
        ]),
        Line::from(""),
        Line::from(Span::styled("Message:", Style::default().add_modifier(Modifier::BOLD))),
    ];

    content.extend(message_lines);

    if let Some(url) = &details.github_url {
        content.push(Line::from(""));
        content.push(Line::from(vec![
            Span::styled("GitHub: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(url.as_str(), Style::default().fg(Color::Cyan)),
        ]));
    }

    content.push(Line::from(""));
    let hint = if details.github_url.is_some() {
        "Space/Esc: close | o: open in GitHub"
    } else {
        "Press Space or Esc to close"
    };
    content.push(Line::from(Span::styled(
        hint,
        Style::default().fg(Color::Gray).add_modifier(Modifier::ITALIC),
    )));

    let popup = Paragraph::new(content)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Commit Details")
                .style(Style::default().bg(Color::Black))
        )
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Left);

    f.render_widget(popup, popup_area);
}

fn author_initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|w| w.chars().next())
        .map(|c| c.to_uppercase().next().unwrap_or(c))
        .take(3)
        .collect()
}

fn truncate(s: &str, max_chars: usize) -> String {
    let mut chars = s.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{}…", &truncated[..truncated.len().min(max_chars - 1)])
    } else {
        truncated
    }
}
