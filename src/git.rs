use git2::{Error, Repository, Status, StatusOptions, StatusShow};
use std::process::Command;

use colored::*;

use crate::ui;

pub fn stage_all() {
    Command::new("git")
        .args(["add", "--all"])
        .output()
        .expect("failed to stage files");
}

pub fn get_stats(repo: &Repository) -> Result<(usize, usize, Vec<(String, Status)>), Error> {
    // Re-read index after staging
    let mut index = repo.index()?;
    index.read(false)?;
    let oid = index.write_tree()?;
    let tree = repo.find_tree(oid)?;
    let head_commit = repo.head()?.peel_to_commit()?;
    let head_tree = head_commit.tree()?;

    let diff = repo.diff_tree_to_tree(Some(&head_tree), Some(&tree), None)?;
    let insertions = diff.stats()?.insertions();
    let deletions = diff.stats()?.deletions();

    let mut status_opts = StatusOptions::new();
    status_opts.show(StatusShow::Index);
    status_opts.include_ignored(false);
    status_opts.renames_head_to_index(true);
    let statuses = repo.statuses(Some(&mut status_opts))?;

    let mut files: Vec<(String, Status)> = Vec::new();
    for entry in statuses.iter() {
        let status = entry.status();
        if status.intersects(
            Status::INDEX_NEW
                | Status::INDEX_MODIFIED
                | Status::INDEX_DELETED
                | Status::INDEX_RENAMED
                | Status::INDEX_TYPECHANGE,
        ) {
            let path = if status.intersects(Status::INDEX_RENAMED) {
                if let Some(delta) = entry.head_to_index() {
                    let old = delta
                        .old_file()
                        .path()
                        .and_then(|p| p.to_str())
                        .unwrap_or("?");
                    let new = delta
                        .new_file()
                        .path()
                        .and_then(|p| p.to_str())
                        .unwrap_or("?");
                    format!("{} → {}", old, new)
                } else {
                    entry.path().unwrap_or("?").to_string()
                }
            } else {
                match entry.path() {
                    Some(p) => p.to_string(),
                    None => continue,
                }
            };
            files.push((path, status));
        }
    }

    Ok((insertions, deletions, files))
}

pub fn branch_exists(repo: &Repository, branch_name: &str) -> bool {
    repo.find_branch(branch_name, git2::BranchType::Local)
        .is_ok()
        || repo
            .find_branch(branch_name, git2::BranchType::Remote)
            .is_ok()
}

pub fn get_remote_url(repo: &Repository, remote_name: &str) -> Result<String, Error> {
    let remote = repo.find_remote(remote_name)?;
    Ok(remote.url().unwrap_or_default().to_string())
}

/// Build the full context string sent to the LLM: a file summary header
/// with per-file stats, followed by the complete diff.
pub fn build_diff_context(
    files: &[(String, Status)],
    insertions: usize,
    deletions: usize,
) -> String {
    let mut ctx = String::new();

    // -- Repo context (branch + recent commits) --
    let branch = git_command_output(&["rev-parse", "--abbrev-ref", "HEAD"])
        .unwrap_or_else(|| "unknown".to_string());
    let log = git_command_output(&["log", "-n", "5", "--stat", "--oneline"])
        .unwrap_or_else(|| "unavailable".to_string());
    ctx.push_str("== Repo Context ==\n");
    ctx.push_str(&format!("Branch: {}\n", branch));
    ctx.push_str("Recent commits:\n");
    for line in log.lines() {
        ctx.push_str(&format!("{}\n", line));
    }
    ctx.push('\n');

    // -- File summary header with per-file stats --
    let numstat = collect_numstat();
    ctx.push_str("== Changed Files ==\n");
    for (path, status) in files {
        let prefix = match *status {
            Status::INDEX_NEW => "+",
            Status::INDEX_MODIFIED => "M",
            Status::INDEX_DELETED => "-",
            Status::INDEX_RENAMED => "R",
            Status::INDEX_TYPECHANGE => "T",
            _ => "?",
        };
        let stats = numstat
            .get(path.as_str())
            .map(|(a, d)| format!("(+{}, -{})", a, d))
            .unwrap_or_else(|| "(new file)".to_string());
        ctx.push_str(&format!("{}  {:<40} {}\n", prefix, path, stats));
    }
    ctx.push_str(&format!(
        "\nTotal: {} files changed, +{} lines, -{} lines\n",
        files.len(),
        insertions,
        deletions,
    ));

    // -- Full diff body (everything is staged) --
    ctx.push_str("\n== Diff ==\n");

    let staged = Command::new("git")
        .args(["diff", "--cached"])
        .output()
        .expect("failed to get staged diff");

    ctx.push_str(&String::from_utf8_lossy(&staged.stdout));

    const MAX_CONTEXT_LEN: usize = 100_000;
    if ctx.len() > MAX_CONTEXT_LEN {
        ctx.truncate(MAX_CONTEXT_LEN);
        ctx.push_str("\n... (diff truncated)");
    }

    ctx
}

/// Parse `git diff --numstat` (staged + unstaged) into a map of path -> (additions, deletions).
fn collect_numstat() -> std::collections::HashMap<String, (usize, usize)> {
    let mut map = std::collections::HashMap::new();

    for args in [&["diff", "--numstat", "--cached"][..]] {
        let out = Command::new("git")
            .args(args)
            .output()
            .expect("failed to get numstat");
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 3 {
                let add: usize = parts[0].parse().unwrap_or(0);
                let del: usize = parts[1].parse().unwrap_or(0);
                let path = parts[2].to_string();
                let entry = map.entry(path).or_insert((0, 0));
                entry.0 += add;
                entry.1 += del;
            }
        }
    }

    map
}

fn git_command_output(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

pub fn create_branch(repo: &Repository, branch: &str) {
    if branch_exists(repo, branch) {
        eprintln!(
            "{}",
            format!("Branch '{}' already exists •◠•", branch).red()
        );
        std::process::exit(1);
    }
    let out = Command::new("git")
        .args(["checkout", "-b", branch])
        .output()
        .expect("failed to create branch");
    if !out.status.success() {
        eprintln!("{}", String::from_utf8_lossy(&out.stderr).red());
        std::process::exit(1);
    }
}

pub fn stage_and_commit(message: &str) {
    let out = Command::new("git")
        .args(["commit", "-m", message])
        .output()
        .expect("failed to commit");
    if !out.status.success() {
        eprintln!("{}", "Commit failed •◠•".red());
        eprintln!("{}", String::from_utf8_lossy(&out.stderr));
        std::process::exit(1);
    }
}

pub fn push(repo: &Repository, is_new_branch: bool) {
    let push_args = if is_new_branch {
        vec!["push", "-u", "origin", "HEAD"]
    } else {
        vec!["push"]
    };

    let out = Command::new("git")
        .args(&push_args)
        .output()
        .expect("failed to push");

    if !out.status.success() {
        eprintln!("{}", String::from_utf8_lossy(&out.stderr).red());
        std::process::exit(1);
    }

    let remote_url = get_remote_url(repo, "origin").unwrap_or_else(|_| {
        eprintln!("{}", "Error reading remote url •◠•".red());
        std::process::exit(1);
    });

    if is_new_branch {
        println!("{}", ui::labeled("Pushed to", &remote_url, |s| s.purple()));
    } else {
        println!("{}", ui::labeled("Pushed to", &remote_url, |s| s.purple()));
    }
}
