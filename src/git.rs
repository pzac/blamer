use git2::{BlameOptions, Oid, Repository};
use std::path::Path;

pub struct BlameLine {
    pub author: String,
    pub date: String,
    pub summary: String,
    pub line_num: usize,
    pub content: String,
    pub full_commit_id: String,
}

pub struct CommitDetails {
    pub sha: String,
    pub author: String,
    pub author_email: String,
    pub date: String,
    pub message: String,
    pub github_url: Option<String>,
}

pub fn get_blame_info(repo: &Repository, file_path: &Path) -> Result<Vec<BlameLine>, Box<dyn std::error::Error>> {
    let workdir = repo.workdir()
        .ok_or("Repository has no working directory")?;

    let relative_path = file_path.strip_prefix(workdir)
        .map_err(|e| format!("File is not in repository working directory: {}", e))?;

    let content = std::fs::read_to_string(file_path)?;
    let file_lines: Vec<&str> = content.lines().collect();

    let mut blame_opts = BlameOptions::new();
    let blame = repo.blame_file(relative_path, Some(&mut blame_opts))?;

    let mut lines = Vec::new();

    for (idx, line_content) in file_lines.iter().enumerate() {
        let line_num = idx + 1;

        let (full_id, author, date, summary) = match blame.get_line(line_num) {
            Some(hunk) => {
                let full_commit_id = hunk.final_commit_id();
                match repo.find_commit(full_commit_id) {
                    Ok(commit) => {
                        let author = commit.author().name().unwrap_or("Unknown").to_string();
                        let date = chrono::DateTime::from_timestamp(commit.time().seconds(), 0)
                            .map(|dt| dt.format("%Y-%m-%d").to_string())
                            .unwrap_or_else(|| "Unknown".to_string());
                        let summary = commit.message().unwrap_or("").lines().next().unwrap_or("").to_string();
                        (full_commit_id.to_string(), author, date, summary)
                    }
                    Err(_) => ("0".repeat(40), "Unknown".to_string(), String::new(), String::new()),
                }
            }
            None => ("0".repeat(40), "You".to_string(), String::new(), "Working Tree".to_string()),
        };

        lines.push(BlameLine {
            author,
            date,
            summary,
            line_num,
            content: line_content.to_string(),
            full_commit_id: full_id,
        });
    }

    Ok(lines)
}

pub fn get_blame_info_at_commit(
    repo: &Repository,
    relative_path: &Path,
    commit_oid: Oid,
) -> Result<Vec<BlameLine>, Box<dyn std::error::Error>> {
    let spec = format!("{}:{}", commit_oid, relative_path.display());
    let object = repo.revparse_single(&spec)
        .map_err(|e| format!("File not found at commit {}: {}", commit_oid, e))?;
    let blob = repo.find_blob(object.id())?;
    let content = std::str::from_utf8(blob.content())
        .map_err(|e| format!("File is not valid UTF-8: {}", e))?;
    let file_lines: Vec<&str> = content.lines().collect();

    let mut blame_opts = BlameOptions::new();
    blame_opts.newest_commit(commit_oid);
    let blame = repo.blame_file(relative_path, Some(&mut blame_opts))?;

    let mut lines = Vec::new();
    for (idx, line_content) in file_lines.iter().enumerate() {
        let line_num = idx + 1;
        let (full_id, author, date, summary) = match blame.get_line(line_num) {
            Some(hunk) => {
                let full_commit_id = hunk.final_commit_id();
                match repo.find_commit(full_commit_id) {
                    Ok(commit) => {
                        let author = commit.author().name().unwrap_or("Unknown").to_string();
                        let date = chrono::DateTime::from_timestamp(commit.time().seconds(), 0)
                            .map(|dt| dt.format("%Y-%m-%d").to_string())
                            .unwrap_or_else(|| "Unknown".to_string());
                        let summary = commit.message().unwrap_or("").lines().next().unwrap_or("").to_string();
                        (full_commit_id.to_string(), author, date, summary)
                    }
                    Err(_) => ("0".repeat(40), "Unknown".to_string(), String::new(), String::new()),
                }
            }
            None => ("0".repeat(40), "You".to_string(), String::new(), "Working Tree".to_string()),
        };
        lines.push(BlameLine { author, date, summary, line_num, content: line_content.to_string(), full_commit_id: full_id });
    }
    Ok(lines)
}

pub struct FileCommit {
    pub oid: String,
    pub short_id: String,
    pub author: String,
    pub date: String,
    pub summary: String,
}

pub fn get_file_commits(repo: &Repository, relative_path: &Path) -> Result<Vec<FileCommit>, Box<dyn std::error::Error>> {
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)?;

    let mut commits = Vec::new();
    let path_str = relative_path.to_str().unwrap_or("");

    for oid_result in revwalk {
        let oid = oid_result?;
        let commit = repo.find_commit(oid)?;
        let commit_tree = commit.tree()?;

        let touches_file = if commit.parent_count() == 0 {
            commit_tree.get_path(relative_path).is_ok()
        } else {
            let parent_tree = commit.parent(0)?.tree()?;
            let mut diff_opts = git2::DiffOptions::new();
            diff_opts.pathspec(path_str);
            let diff = repo.diff_tree_to_tree(Some(&parent_tree), Some(&commit_tree), Some(&mut diff_opts))?;
            diff.deltas().count() > 0
        };

        if touches_file {
            let author = commit.author().name().unwrap_or("Unknown").to_string();
            let date = chrono::DateTime::from_timestamp(commit.time().seconds(), 0)
                .map(|dt| dt.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| "Unknown".to_string());
            let summary = commit.message().unwrap_or("").lines().next().unwrap_or("").to_string();
            commits.push(FileCommit {
                oid: oid.to_string(),
                short_id: format!("{:.8}", oid),
                author,
                date,
                summary,
            });
        }
    }

    Ok(commits)
}

pub fn get_github_commit_url(repo: &Repository, sha: &str) -> Option<String> {
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
