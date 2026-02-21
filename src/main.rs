use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use git2::{BlameOptions, Repository};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};
use std::io;
use std::path::Path;
use std::process;

#[derive(Parser)]
#[command(name = "blamer")]
#[command(about = "A TUI for viewing git blame information", long_about = None)]
struct Cli {
    /// Path to the file
    filename: String,
}

struct BlameLine {
    commit_sha: String,
    author: String,
    date: String,
    line_num: usize,
    content: String,
    full_commit_id: String,
}

struct CommitDetails {
    sha: String,
    author: String,
    author_email: String,
    date: String,
    message: String,
    github_url: Option<String>,
}

struct App {
    lines: Vec<BlameLine>,
    scroll_offset: usize,
    selected_line: usize,
    filename: String,
    show_commit_details: bool,
    commit_details: Option<CommitDetails>,
    repo_path: std::path::PathBuf,
}

impl App {
    fn new(filename: String, lines: Vec<BlameLine>, repo_path: std::path::PathBuf) -> Self {
        Self {
            lines,
            scroll_offset: 0,
            selected_line: 0,
            filename,
            show_commit_details: false,
            commit_details: None,
            repo_path,
        }
    }

    fn scroll_down(&mut self) {
        if self.selected_line < self.lines.len().saturating_sub(1) {
            self.selected_line += 1;
            // Auto-scroll if selected line goes off screen
            if self.selected_line >= self.scroll_offset + 20 {
                self.scroll_offset = self.selected_line.saturating_sub(19);
            }
        }
    }

    fn scroll_up(&mut self) {
        if self.selected_line > 0 {
            self.selected_line -= 1;
            // Auto-scroll if selected line goes off screen
            if self.selected_line < self.scroll_offset {
                self.scroll_offset = self.selected_line;
            }
        }
    }

    fn page_down(&mut self, page_size: usize) {
        self.selected_line = (self.selected_line + page_size).min(self.lines.len().saturating_sub(1));
        self.scroll_offset = (self.scroll_offset + page_size).min(self.lines.len().saturating_sub(1));
    }

    fn page_up(&mut self, page_size: usize) {
        self.selected_line = self.selected_line.saturating_sub(page_size);
        self.scroll_offset = self.scroll_offset.saturating_sub(page_size);
    }

    fn toggle_commit_details(&mut self) {
        if self.show_commit_details {
            self.show_commit_details = false;
            self.commit_details = None;
        } else {
            // Load commit details for selected line
            if let Some(details) = self.load_commit_details() {
                self.commit_details = Some(details);
                self.show_commit_details = true;
            }
        }
    }

    fn load_commit_details(&self) -> Option<CommitDetails> {
        let line = &self.lines[self.selected_line];

        // Don't show details for uncommitted changes
        if line.full_commit_id == "0000000000000000000000000000000000000000" {
            return None;
        }

        let repo = Repository::open(&self.repo_path).ok()?;
        let oid = git2::Oid::from_str(&line.full_commit_id).ok()?;
        let commit = repo.find_commit(oid).ok()?;

        let author = commit.author();
        let timestamp = commit.time().seconds();
        let datetime = chrono::DateTime::from_timestamp(timestamp, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S %z").to_string())
            .unwrap_or_else(|| "Unknown date".to_string());

        let github_url = get_github_commit_url(&repo, &line.full_commit_id);

        Some(CommitDetails {
            sha: line.full_commit_id.clone(),
            author: author.name().unwrap_or("Unknown").to_string(),
            author_email: author.email().unwrap_or("unknown@email").to_string(),
            date: datetime,
            message: commit.message().unwrap_or("No message").to_string(),
            github_url,
        })
    }
}

fn parse_github_base_url(remote_url: &str) -> Option<String> {
    // HTTPS: https://github.com/owner/repo.git or https://github.com/owner/repo
    if let Some(path) = remote_url.strip_prefix("https://github.com/") {
        let repo_path = path.trim_end_matches(".git");
        return Some(format!("https://github.com/{}", repo_path));
    }
    // SSH: git@github.com:owner/repo.git or git@github.com:owner/repo
    if let Some(path) = remote_url.strip_prefix("git@github.com:") {
        let repo_path = path.trim_end_matches(".git");
        return Some(format!("https://github.com/{}", repo_path));
    }
    None
}

fn get_github_commit_url(repo: &Repository, sha: &str) -> Option<String> {
    let remotes = repo.remotes().ok()?;
    for remote_name in remotes.iter().flatten() {
        if let Ok(remote) = repo.find_remote(remote_name) {
            if let Some(url) = remote.url() {
                if let Some(base) = parse_github_base_url(url) {
                    return Some(format!("{}/commit/{}", base, sha));
                }
            }
        }
    }
    None
}

fn open_url(url: &str) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();

    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();

    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd").args(["/c", "start", url]).spawn();
}

fn get_blame_info(repo: &Repository, file_path: &Path) -> Result<Vec<BlameLine>, Box<dyn std::error::Error>> {
    let workdir = repo.workdir()
        .ok_or("Repository has no working directory")?;

    let relative_path = file_path.strip_prefix(workdir)
        .map_err(|e| format!("File is not in repository working directory: {}", e))?;

    let content = std::fs::read_to_string(file_path)?;
    let file_lines: Vec<&str> = content.lines().collect();

    let mut blame_opts = BlameOptions::new();
    let blame = repo.blame_file(relative_path, Some(&mut blame_opts))?;

    let mut lines = Vec::new();

    // Iterate through each line in the file
    for (idx, line_content) in file_lines.iter().enumerate() {
        let line_num = idx + 1;

        // Try to get blame for this line
        let (sha, full_id, author, date) = match blame.get_line(line_num) {
            Some(hunk) => {
                let full_commit_id = hunk.final_commit_id();
                match repo.find_commit(full_commit_id) {
                    Ok(commit) => {
                        let sha = format!("{:.8}", full_commit_id);
                        let full_id = full_commit_id.to_string();
                        let author = commit.author().name().unwrap_or("Unknown").to_string();
                        let timestamp = commit.time().seconds();
                        let datetime = chrono::DateTime::from_timestamp(timestamp, 0)
                            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                            .unwrap_or_else(|| "Unknown date".to_string());
                        (sha, full_id, author, datetime)
                    }
                    Err(_) => {
                        ("????????".to_string(), "0".repeat(40), "Unknown".to_string(), "Unknown date".to_string())
                    }
                }
            }
            None => {
                // No blame info for this line (uncommitted changes)
                ("Not Committed".to_string(), "0".repeat(40), "You".to_string(), "Working Tree".to_string())
            }
        };

        lines.push(BlameLine {
            commit_sha: sha,
            author,
            date,
            line_num,
            content: line_content.to_string(),
            full_commit_id: full_id,
        });
    }

    Ok(lines)
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(f.area());

    // Header
    let header = Paragraph::new(format!("Git Blame: {}", app.filename))
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);

    // Blame content
    let visible_height = chunks[1].height.saturating_sub(2) as usize;
    let end_idx = (app.scroll_offset + visible_height).min(app.lines.len());

    let items: Vec<ListItem> = app.lines[app.scroll_offset..end_idx]
        .iter()
        .enumerate()
        .map(|(idx, blame_line)| {
            let actual_line_idx = app.scroll_offset + idx;
            let is_selected = actual_line_idx == app.selected_line;

            let mut style = Style::default();
            if is_selected {
                style = style.bg(Color::DarkGray);
            }

            let line_content = vec![
                Span::styled(
                    format!("{:4} ", blame_line.line_num),
                    style.fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("{} ", blame_line.commit_sha),
                    style.fg(Color::Yellow),
                ),
                Span::styled(
                    format!("{:20} ", blame_line.author),
                    style.fg(Color::Green),
                ),
                Span::styled(
                    format!("{:16} ", blame_line.date),
                    style.fg(Color::Blue),
                ),
                Span::styled(&blame_line.content, style),
            ];
            ListItem::new(Line::from(line_content))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Blame"));
    f.render_widget(list, chunks[1]);

    // Footer
    let footer_text = format!(
        "Lines {}-{}/{} | ↑/↓: scroll | Space: commit details | q: quit",
        app.scroll_offset + 1,
        end_idx,
        app.lines.len()
    );
    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::Gray))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);

    // Show commit details popup if requested
    if app.show_commit_details {
        if let Some(details) = &app.commit_details {
            render_commit_popup(f, details);
        }
    }
}

fn render_commit_popup(f: &mut Frame, details: &CommitDetails) {
    use ratatui::layout::{Alignment, Rect};
    use ratatui::widgets::{Clear, Wrap};

    // Create a centered rectangle for the popup
    let area = f.area();
    let popup_width = area.width.saturating_sub(10).min(100);
    let popup_height = area.height.saturating_sub(10).min(30);

    let popup_area = Rect {
        x: (area.width.saturating_sub(popup_width)) / 2,
        y: (area.height.saturating_sub(popup_height)) / 2,
        width: popup_width,
        height: popup_height,
    };

    // Clear the area and render the popup
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

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &app))?;

        if let Event::Key(key) = event::read()? {
            // If popup is open, some keys should close it
            if app.show_commit_details {
                match key.code {
                    KeyCode::Char(' ') | KeyCode::Esc => {
                        app.show_commit_details = false;
                        app.commit_details = None;
                    }
                    KeyCode::Char('o') => {
                        if let Some(details) = &app.commit_details {
                            if let Some(url) = &details.github_url {
                                open_url(url);
                            }
                        }
                    }
                    _ => {}
                }
            } else {
                // Normal navigation
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Esc => return Ok(()),
                    KeyCode::Down | KeyCode::Char('j') => app.scroll_down(),
                    KeyCode::Up | KeyCode::Char('k') => app.scroll_up(),
                    KeyCode::PageDown => app.page_down(20),
                    KeyCode::PageUp => app.page_up(20),
                    KeyCode::Home => {
                        app.selected_line = 0;
                        app.scroll_offset = 0;
                    }
                    KeyCode::End => {
                        app.selected_line = app.lines.len().saturating_sub(1);
                        app.scroll_offset = app.lines.len().saturating_sub(1);
                    }
                    KeyCode::Char(' ') => app.toggle_commit_details(),
                    _ => {}
                }
            }
        }
    }
}

fn main() {
    let cli = Cli::parse();

    let file_path = Path::new(&cli.filename);

    // Check if file exists
    if !file_path.exists() {
        eprintln!("Error: File '{}' does not exist", cli.filename);
        process::exit(1);
    }

    // Get the absolute path
    let abs_path = match file_path.canonicalize() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Error: Could not resolve file path: {}", e);
            process::exit(1);
        }
    };

    // Try to find a git repository containing this file
    let repo = match Repository::discover(&abs_path) {
        Ok(repo) => repo,
        Err(e) => {
            eprintln!(
                "Error: File '{}' is not part of a git repository: {}",
                cli.filename, e
            );
            process::exit(1);
        }
    };

    // Get blame information
    let blame_lines = match get_blame_info(&repo, &abs_path) {
        Ok(lines) => lines,
        Err(e) => {
            eprintln!("Error: Could not get blame information: {}", e);
            process::exit(1);
        }
    };

    // Get the repository path
    let repo_path = repo.path().to_path_buf();

    // Setup terminal
    enable_raw_mode().unwrap();
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture).unwrap();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).unwrap();

    // Create app and run
    let app = App::new(cli.filename.clone(), blame_lines, repo_path);
    let res = run_app(&mut terminal, app);

    // Restore terminal
    disable_raw_mode().unwrap();
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .unwrap();
    terminal.show_cursor().unwrap();

    if let Err(err) = res {
        eprintln!("Error: {}", err);
        process::exit(1);
    }
}
