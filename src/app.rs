use git2::Repository;
use crate::git::{BlameLine, CommitDetails, FileCommit, get_blame_info_at_commit, get_file_commits, get_github_commit_url};

pub struct HistoryEntry {
    lines: Vec<BlameLine>,
    selected_line: usize,
    scroll_offset: usize,
    current_commit_label: Option<String>,
    current_view_commit_id: Option<String>,
}

pub struct App {
    pub lines: Vec<BlameLine>,
    pub scroll_offset: usize,
    pub selected_line: usize,
    pub filename: String,
    pub show_commit_details: bool,
    pub commit_details: Option<CommitDetails>,
    pub repo_path: std::path::PathBuf,
    pub relative_file_path: std::path::PathBuf,
    pub history_stack: Vec<HistoryEntry>,
    pub current_commit_label: Option<String>,
    pub show_commit_list: bool,
    pub commit_list: Vec<FileCommit>,
    pub commit_list_selected: usize,
    pub current_view_commit_id: Option<String>,
}

impl App {
    pub fn new(filename: String, lines: Vec<BlameLine>, repo_path: std::path::PathBuf, relative_file_path: std::path::PathBuf) -> Self {
        Self {
            lines,
            scroll_offset: 0,
            selected_line: 0,
            filename,
            show_commit_details: false,
            commit_details: None,
            repo_path,
            relative_file_path,
            history_stack: Vec::new(),
            current_commit_label: None,
            show_commit_list: false,
            commit_list: Vec::new(),
            commit_list_selected: 0,
            current_view_commit_id: None,
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

    pub fn go_back_in_history(&mut self) {
        let full_id = self.lines[self.selected_line].full_commit_id.clone();
        if full_id == "0".repeat(40) { return; }

        let repo = match Repository::open(&self.repo_path) { Ok(r) => r, Err(_) => return };
        let oid = match git2::Oid::from_str(&full_id) { Ok(o) => o, Err(_) => return };
        let commit = match repo.find_commit(oid) { Ok(c) => c, Err(_) => return };

        // Resolve the current view's newest commit ID
        let view_commit_id = match &self.current_view_commit_id {
            Some(id) => id.clone(),
            None => match repo.head().and_then(|h| h.peel_to_commit()) {
                Ok(c) => c.id().to_string(),
                Err(_) => return,
            },
        };

        // If the selected line's commit IS the current view's commit: go to parent (previous change)
        // Otherwise: jump to the selected line's commit (show the version of the file at that commit)
        let target_oid = if full_id == view_commit_id {
            if commit.parent_count() == 0 { return; }
            match commit.parent_id(0) { Ok(id) => id, Err(_) => return }
        } else {
            oid
        };

        let target_commit = match repo.find_commit(target_oid) { Ok(c) => c, Err(_) => return };
        let target_date = chrono::DateTime::from_timestamp(target_commit.time().seconds(), 0)
            .map(|dt| dt.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        let target_title = target_commit.message().unwrap_or("").lines().next().unwrap_or("").to_string();

        let new_lines = match get_blame_info_at_commit(&repo, &self.relative_file_path, target_oid) {
            Ok(l) => l,
            Err(_) => return,
        };

        self.history_stack.push(HistoryEntry {
            lines: std::mem::replace(&mut self.lines, new_lines),
            selected_line: self.selected_line,
            scroll_offset: self.scroll_offset,
            current_commit_label: self.current_commit_label.take(),
            current_view_commit_id: self.current_view_commit_id.take(),
        });

        self.selected_line = self.selected_line.min(self.lines.len().saturating_sub(1));
        self.scroll_offset = self.scroll_offset.min(self.lines.len().saturating_sub(1)).min(self.selected_line);
        self.current_commit_label = Some(format!("{:.8} · {} · {}", target_oid, target_date, target_title));
        self.current_view_commit_id = Some(target_oid.to_string());
        self.show_commit_details = false;
        self.commit_details = None;
    }

    pub fn go_forward_in_history(&mut self) {
        let entry = match self.history_stack.pop() { Some(e) => e, None => return };
        self.lines = entry.lines;
        self.selected_line = entry.selected_line;
        self.scroll_offset = entry.scroll_offset;
        self.current_commit_label = entry.current_commit_label;
        self.current_view_commit_id = entry.current_view_commit_id;
        self.show_commit_details = false;
        self.commit_details = None;
    }

    pub fn toggle_commit_list(&mut self) {
        if self.show_commit_list {
            self.show_commit_list = false;
            return;
        }
        let repo = match Repository::open(&self.repo_path) { Ok(r) => r, Err(_) => return };
        if let Ok(commits) = get_file_commits(&repo, &self.relative_file_path) {
            self.commit_list = commits;
            self.commit_list_selected = 0;
            self.show_commit_list = true;
        }
    }

    pub fn commit_list_up(&mut self) {
        if self.commit_list_selected > 0 {
            self.commit_list_selected -= 1;
        }
    }

    pub fn commit_list_down(&mut self) {
        if self.commit_list_selected + 1 < self.commit_list.len() {
            self.commit_list_selected += 1;
        }
    }

    pub fn jump_to_commit_list_entry(&mut self) {
        let entry = match self.commit_list.get(self.commit_list_selected) {
            Some(e) => e,
            None => return,
        };
        let oid_str = entry.oid.clone();
        let label = format!("{} · {} · {}", entry.short_id, entry.date, entry.summary);

        let repo = match Repository::open(&self.repo_path) { Ok(r) => r, Err(_) => return };
        let oid = match git2::Oid::from_str(&oid_str) { Ok(o) => o, Err(_) => return };

        let new_lines = match get_blame_info_at_commit(&repo, &self.relative_file_path, oid) {
            Ok(l) => l,
            Err(_) => return,
        };

        self.history_stack.push(HistoryEntry {
            lines: std::mem::replace(&mut self.lines, new_lines),
            selected_line: self.selected_line,
            scroll_offset: self.scroll_offset,
            current_commit_label: self.current_commit_label.take(),
            current_view_commit_id: self.current_view_commit_id.take(),
        });

        self.selected_line = self.selected_line.min(self.lines.len().saturating_sub(1));
        self.scroll_offset = self.scroll_offset.min(self.lines.len().saturating_sub(1)).min(self.selected_line);
        self.current_commit_label = Some(label);
        self.current_view_commit_id = Some(oid_str.clone());
        self.show_commit_details = false;
        self.commit_details = None;
        self.show_commit_list = false;
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
