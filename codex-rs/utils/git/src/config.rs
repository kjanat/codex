//! Lightweight git config file parsing.
//!
//! These functions read git configuration directly from files without
//! spawning git subprocesses, making them suitable for optional features
//! where minimizing overhead is desirable.

use std::fs;
use std::path::Path;
use std::path::PathBuf;

/// Find `.git` directory by walking up from current dir.
///
/// Handles worktrees where `.git` is a file containing `gitdir: /path/to/.git`.
///
/// Returns `None` if no git directory is found.
pub fn find_git_dir() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let git_path = dir.join(".git");
        if git_path.is_dir() {
            return Some(git_path);
        }
        // Worktree: .git is a file containing "gitdir: /path/to/real/.git"
        if git_path.is_file()
            && let Ok(content) = fs::read_to_string(&git_path)
            && let Some(path) = content.trim().strip_prefix("gitdir: ")
        {
            return Some(PathBuf::from(path));
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Parse origin URL from `.git/config`.
///
/// Looks for the `[remote "origin"]` section and extracts the `url` value.
///
/// Returns `None` if the config file doesn't exist or has no origin remote.
pub fn read_origin_url(git_dir: &Path) -> Option<String> {
    let config_path = git_dir.join("config");
    let content = fs::read_to_string(config_path).ok()?;

    let mut in_origin_section = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_origin_section = trimmed == r#"[remote "origin"]"#;
        } else if in_origin_section && trimmed.starts_with("url") {
            // Parse "url = https://..."
            return trimmed.split('=').nth(1).map(|s| s.trim().to_string());
        }
    }
    None
}

/// Read default branch from `.git/refs/remotes/origin/HEAD`.
///
/// Returns `None` if the file doesn't exist or can't be parsed.
pub fn read_default_branch(git_dir: &Path) -> Option<String> {
    let head_path = git_dir.join("refs/remotes/origin/HEAD");
    let content = fs::read_to_string(head_path).ok()?;

    // Format: "ref: refs/remotes/origin/master"
    content
        .trim()
        .strip_prefix("ref: refs/remotes/origin/")
        .map(std::string::ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_read_origin_url_https() {
        let tmp = TempDir::new().unwrap();
        let git_dir = tmp.path();
        fs::write(
            git_dir.join("config"),
            r#"[core]
    repositoryformatversion = 0
[remote "origin"]
    url = https://github.com/user/repo.git
    fetch = +refs/heads/*:refs/remotes/origin/*
[branch "main"]
    remote = origin
"#,
        )
        .unwrap();

        assert_eq!(
            read_origin_url(git_dir),
            Some("https://github.com/user/repo.git".to_string())
        );
    }

    #[test]
    fn test_read_origin_url_ssh() {
        let tmp = TempDir::new().unwrap();
        let git_dir = tmp.path();
        fs::write(
            git_dir.join("config"),
            r#"[remote "origin"]
    url = git@github.com:user/repo.git
"#,
        )
        .unwrap();

        assert_eq!(
            read_origin_url(git_dir),
            Some("git@github.com:user/repo.git".to_string())
        );
    }

    #[test]
    fn test_read_origin_url_no_origin() {
        let tmp = TempDir::new().unwrap();
        let git_dir = tmp.path();
        fs::write(
            git_dir.join("config"),
            r#"[core]
    repositoryformatversion = 0
[remote "upstream"]
    url = https://github.com/other/repo.git
"#,
        )
        .unwrap();

        assert_eq!(read_origin_url(git_dir), None);
    }

    #[test]
    fn test_read_origin_url_no_config() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(read_origin_url(tmp.path()), None);
    }

    #[test]
    fn test_read_default_branch_main() {
        let tmp = TempDir::new().unwrap();
        let git_dir = tmp.path();
        fs::create_dir_all(git_dir.join("refs/remotes/origin")).unwrap();
        fs::write(
            git_dir.join("refs/remotes/origin/HEAD"),
            "ref: refs/remotes/origin/main\n",
        )
        .unwrap();

        assert_eq!(read_default_branch(git_dir), Some("main".to_string()));
    }

    #[test]
    fn test_read_default_branch_master() {
        let tmp = TempDir::new().unwrap();
        let git_dir = tmp.path();
        fs::create_dir_all(git_dir.join("refs/remotes/origin")).unwrap();
        fs::write(
            git_dir.join("refs/remotes/origin/HEAD"),
            "ref: refs/remotes/origin/master\n",
        )
        .unwrap();

        assert_eq!(read_default_branch(git_dir), Some("master".to_string()));
    }

    #[test]
    fn test_read_default_branch_no_file() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(read_default_branch(tmp.path()), None);
    }
}
