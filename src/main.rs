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
}

struct App {
    lines: Vec<BlameLine>,
    scroll_offset: usize,
    filename: String,
}

impl App {
    fn new(filename: String, lines: Vec<BlameLine>) -> Self {
        Self {
            lines,
            scroll_offset: 0,
            filename,
        }
    }

    fn scroll_down(&mut self) {
        if self.scroll_offset < self.lines.len().saturating_sub(1) {
            self.scroll_offset += 1;
        }
    }

    fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    fn page_down(&mut self, page_size: usize) {
        self.scroll_offset = (self.scroll_offset + page_size).min(self.lines.len().saturating_sub(1));
    }

    fn page_up(&mut self, page_size: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(page_size);
    }
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
        let (sha, author, date) = match blame.get_line(line_num) {
            Some(hunk) => {
                match repo.find_commit(hunk.final_commit_id()) {
                    Ok(commit) => {
                        let sha = format!("{:.8}", hunk.final_commit_id());
                        let author = commit.author().name().unwrap_or("Unknown").to_string();
                        let timestamp = commit.time().seconds();
                        let datetime = chrono::DateTime::from_timestamp(timestamp, 0)
                            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                            .unwrap_or_else(|| "Unknown date".to_string());
                        (sha, author, datetime)
                    }
                    Err(_) => {
                        ("????????".to_string(), "Unknown".to_string(), "Unknown date".to_string())
                    }
                }
            }
            None => {
                // No blame info for this line (uncommitted changes)
                ("Not Committed".to_string(), "You".to_string(), "Working Tree".to_string())
            }
        };

        lines.push(BlameLine {
            commit_sha: sha,
            author,
            date,
            line_num,
            content: line_content.to_string(),
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
        .map(|blame_line| {
            let line_content = vec![
                Span::styled(
                    format!("{:4} ", blame_line.line_num),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("{} ", blame_line.commit_sha),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(
                    format!("{:20} ", blame_line.author),
                    Style::default().fg(Color::Green),
                ),
                Span::styled(
                    format!("{:16} ", blame_line.date),
                    Style::default().fg(Color::Blue),
                ),
                Span::raw(&blame_line.content),
            ];
            ListItem::new(Line::from(line_content))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Blame"));
    f.render_widget(list, chunks[1]);

    // Footer
    let footer_text = format!(
        "Lines {}-{}/{} | ↑/↓: scroll | PgUp/PgDn: page | q: quit",
        app.scroll_offset + 1,
        end_idx,
        app.lines.len()
    );
    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::Gray))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &app))?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Down | KeyCode::Char('j') => app.scroll_down(),
                KeyCode::Up | KeyCode::Char('k') => app.scroll_up(),
                KeyCode::PageDown => app.page_down(20),
                KeyCode::PageUp => app.page_up(20),
                KeyCode::Home => app.scroll_offset = 0,
                KeyCode::End => app.scroll_offset = app.lines.len().saturating_sub(1),
                _ => {}
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

    // Setup terminal
    enable_raw_mode().unwrap();
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture).unwrap();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).unwrap();

    // Create app and run
    let app = App::new(cli.filename.clone(), blame_lines);
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
