use git2::Repository;
use crate::git::{BlameLine, CommitDetails, get_github_commit_url};

pub struct App {
    pub lines: Vec<BlameLine>,
    pub scroll_offset: usize,
    pub selected_line: usize,
    pub filename: String,
    pub show_commit_details: bool,
    pub commit_details: Option<CommitDetails>,
    pub repo_path: std::path::PathBuf,
}

impl App {
    pub fn new(filename: String, lines: Vec<BlameLine>, repo_path: std::path::PathBuf) -> Self {
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

    pub fn scroll_down(&mut self) {
        if self.selected_line < self.lines.len().saturating_sub(1) {
            self.selected_line += 1;
            if self.selected_line >= self.scroll_offset + 20 {
                self.scroll_offset = self.selected_line.saturating_sub(19);
            }
        }
    }

    pub fn scroll_up(&mut self) {
        if self.selected_line > 0 {
            self.selected_line -= 1;
            if self.selected_line < self.scroll_offset {
                self.scroll_offset = self.selected_line;
            }
        }
    }

    pub fn page_down(&mut self, page_size: usize) {
        self.selected_line = (self.selected_line + page_size).min(self.lines.len().saturating_sub(1));
        self.scroll_offset = (self.scroll_offset + page_size).min(self.lines.len().saturating_sub(1));
    }

    pub fn page_up(&mut self, page_size: usize) {
        self.selected_line = self.selected_line.saturating_sub(page_size);
        self.scroll_offset = self.scroll_offset.saturating_sub(page_size);
    }

    pub fn toggle_commit_details(&mut self) {
        if self.show_commit_details {
            self.show_commit_details = false;
            self.commit_details = None;
        } else {
            if let Some(details) = self.load_commit_details() {
                self.commit_details = Some(details);
                self.show_commit_details = true;
            }
        }
    }

    fn load_commit_details(&self) -> Option<CommitDetails> {
        let line = &self.lines[self.selected_line];

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
