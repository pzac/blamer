mod app;
mod git;
mod ui;

use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use git2::Repository;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::path::Path;
use std::process;

use app::App;
use git::get_blame_info;
use ui::ui;

#[derive(Parser)]
#[command(name = "blamer")]
#[command(version)]
#[command(about = "A TUI for viewing git blame information", long_about = None)]
struct Cli {
    /// Path to the file
    filename: String,
}

fn open_url(url: &str) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();

    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();

    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd").args(["/c", "start", url]).spawn();
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &app))?;

        if let Event::Key(key) = event::read()? {
            if app.show_commit_list {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('l') | KeyCode::Char('q') => app.show_commit_list = false,
                    KeyCode::Up | KeyCode::Char('k') => app.commit_list_up(),
                    KeyCode::Down | KeyCode::Char('j') => app.commit_list_down(),
                    KeyCode::Enter => app.jump_to_commit_list_entry(),
                    _ => {}
                }
            } else if app.show_commit_details {
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
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
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
                    KeyCode::Left => app.go_back_in_history(),
                    KeyCode::Right => app.go_forward_in_history(),
                    KeyCode::Char('l') => app.toggle_commit_list(),
                    _ => {}
                }
            }
        }
    }
}

fn main() {
    let cli = Cli::parse();

    let file_path = Path::new(&cli.filename);

    if !file_path.exists() {
        eprintln!("Error: File '{}' does not exist", cli.filename);
        process::exit(1);
    }

    let abs_path = match file_path.canonicalize() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Error: Could not resolve file path: {}", e);
            process::exit(1);
        }
    };

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

    let workdir = match repo.workdir() {
        Some(w) => w,
        None => { eprintln!("Error: Bare repositories are not supported"); process::exit(1); }
    };
    let relative_file_path = match abs_path.strip_prefix(workdir) {
        Ok(p) => p.to_path_buf(),
        Err(e) => { eprintln!("Error: {}", e); process::exit(1); }
    };

    let blame_lines = match get_blame_info(&repo, &abs_path) {
        Ok(lines) => lines,
        Err(e) => {
            eprintln!("Error: Could not get blame information: {}", e);
            process::exit(1);
        }
    };

    let repo_path = repo.path().to_path_buf();

    enable_raw_mode().unwrap();
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture).unwrap();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).unwrap();

    let app = App::new(cli.filename.clone(), blame_lines, repo_path, relative_file_path);
    let res = run_app(&mut terminal, app);

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
