use git2::{BlameOptions, Oid, Repository};
use std::path::Path;

pub struct BlameLine {
    pub commit_sha: String,
    pub author: String,
    pub date: String,
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
                    Err(_) => ("????????".to_string(), "0".repeat(40), "Unknown".to_string(), "Unknown date".to_string()),
                }
            }
            None => ("Not Committed".to_string(), "0".repeat(40), "You".to_string(), "Working Tree".to_string()),
        };
        lines.push(BlameLine { commit_sha: sha, author, date, line_num, content: line_content.to_string(), full_commit_id: full_id });
    }
    Ok(lines)
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
