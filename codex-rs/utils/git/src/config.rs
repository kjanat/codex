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
        // The path can be absolute or relative to the .git file's parent directory
        if git_path.is_file()
            && let Ok(content) = fs::read_to_string(&git_path)
            && let Some(path) = content.trim().strip_prefix("gitdir: ")
        {
            let gitdir_path = Path::new(path);
            if gitdir_path.is_absolute() {
                return Some(gitdir_path.to_path_buf());
            }
            // Relative path: resolve against the .git file's parent directory
            if let Some(parent) = git_path.parent() {
                let resolved = parent.join(gitdir_path);
                // Canonicalize to normalize ".." components, fall back to joined path
                return Some(resolved.canonicalize().unwrap_or(resolved));
            }
            return Some(gitdir_path.to_path_buf());
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

    #[test]
    fn test_find_git_dir_worktree_relative_path() {
        // Simulate a worktree structure:
        // tmp/
        //   main-repo/.git/           (real git dir)
        //   main-repo/.git/worktrees/feature/  (worktree git dir)
        //   worktree/.git             (file with relative gitdir)
        let tmp = TempDir::new().unwrap();
        let main_repo = tmp.path().join("main-repo");
        let main_git = main_repo.join(".git");
        let worktree_git_dir = main_git.join("worktrees/feature");
        let worktree_dir = tmp.path().join("worktree");

        // Create the main repo's git directory structure
        fs::create_dir_all(&worktree_git_dir).unwrap();

        // Create worktree with relative gitdir reference
        fs::create_dir_all(&worktree_dir).unwrap();
        fs::write(
            worktree_dir.join(".git"),
            "gitdir: ../main-repo/.git/worktrees/feature\n",
        )
        .unwrap();

        // Change to worktree directory and find git dir
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&worktree_dir).unwrap();

        let result = find_git_dir();
        std::env::set_current_dir(original_dir).unwrap();

        // Should resolve to the absolute path of the worktree git dir
        let result = result.expect("should find git dir");
        assert!(
            result.ends_with("worktrees/feature"),
            "expected path ending with 'worktrees/feature', got: {result:?}"
        );
        assert!(result.is_absolute(), "result should be absolute path");
    }

    #[test]
    fn test_find_git_dir_worktree_absolute_path() {
        let tmp = TempDir::new().unwrap();
        let main_git = tmp.path().join("main-repo/.git");
        let worktree_git_dir = main_git.join("worktrees/feature");
        let worktree_dir = tmp.path().join("worktree");

        fs::create_dir_all(&worktree_git_dir).unwrap();
        fs::create_dir_all(&worktree_dir).unwrap();

        // Write absolute path
        fs::write(
            worktree_dir.join(".git"),
            format!("gitdir: {}\n", worktree_git_dir.display()),
        )
        .unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&worktree_dir).unwrap();

        let result = find_git_dir();
        std::env::set_current_dir(original_dir).unwrap();

        let result = result.expect("should find git dir");
        assert_eq!(result, worktree_git_dir);
    }
}
