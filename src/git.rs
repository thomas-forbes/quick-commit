use git2::{DiffOptions, Error, Repository, Status, StatusOptions, StatusShow};
use std::process::Command;

use colored::*;

use crate::ui;

pub fn get_stats(repo: &Repository) -> Result<(usize, usize, Vec<(String, Status)>), Error> {
    let mut index = repo.index()?;
    let oid = index.write_tree()?;
    let tree = repo.find_tree(oid)?;
    let head_commit = repo.head()?.peel_to_commit()?;
    let head_tree = head_commit.tree()?;

    let diff_staged = repo.diff_tree_to_tree(Some(&head_tree), Some(&tree), None)?;
    let mut opts = DiffOptions::new();
    opts.include_untracked(true);
    let diff_unstaged = repo.diff_index_to_workdir(Some(&index), Some(&mut opts))?;

    let insertions = diff_staged.stats()?.insertions() + diff_unstaged.stats()?.insertions();
    let deletions = diff_staged.stats()?.deletions() + diff_unstaged.stats()?.deletions();

    let mut status_opts = StatusOptions::new();
    status_opts.show(StatusShow::IndexAndWorkdir);
    status_opts.include_untracked(true);
    status_opts.include_ignored(false);
    status_opts.renames_head_to_index(true);
    status_opts.renames_index_to_workdir(true);
    let statuses = repo.statuses(Some(&mut status_opts))?;

    let mut files: Vec<(String, Status)> = Vec::new();
    for entry in statuses.iter() {
        let status = entry.status();
        if status.intersects(
            Status::INDEX_NEW
                | Status::INDEX_MODIFIED
                | Status::INDEX_DELETED
                | Status::WT_NEW
                | Status::WT_MODIFIED
                | Status::WT_DELETED,
        ) {
            if let Some(path) = entry.path() {
                files.push((String::from(path), status));
            }
        }
    }

    Ok((insertions, deletions, files))
}

pub fn branch_exists(repo: &Repository, branch_name: &str) -> bool {
    repo.find_branch(branch_name, git2::BranchType::Local).is_ok()
        || repo.find_branch(branch_name, git2::BranchType::Remote).is_ok()
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

    // -- File summary header with per-file stats --
    let numstat = collect_numstat();
    ctx.push_str("== Changed Files ==\n");
    for (path, status) in files {
        let prefix = match *status {
            Status::INDEX_NEW | Status::WT_NEW => "+",
            Status::INDEX_MODIFIED | Status::WT_MODIFIED => "M",
            Status::INDEX_DELETED | Status::WT_DELETED => "-",
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

    // -- Full diff body --
    ctx.push_str("\n== Diff ==\n");

    let staged = Command::new("git")
        .args(["diff", "--cached"])
        .output()
        .expect("failed to get staged diff");
    let unstaged = Command::new("git")
        .args(["diff"])
        .output()
        .expect("failed to get unstaged diff");
    let untracked = Command::new("git")
        .args(["ls-files", "--others", "--exclude-standard"])
        .output()
        .expect("failed to get untracked files");

    ctx.push_str(&String::from_utf8_lossy(&staged.stdout));
    ctx.push_str(&String::from_utf8_lossy(&unstaged.stdout));

    let untracked_files = String::from_utf8_lossy(&untracked.stdout);
    for file in untracked_files.lines() {
        if !file.is_empty() {
            ctx.push_str(&format!("\n--- /dev/null\n+++ b/{}\n", file));
            if let Ok(content) = std::fs::read_to_string(file) {
                for line in content.lines() {
                    ctx.push_str(&format!("+{}\n", line));
                }
            }
        }
    }

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

    for args in [&["diff", "--numstat", "--cached"][..], &["diff", "--numstat"][..]] {
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
    println!("{}", ui::labeled("Created branch", branch, |s| s.green()));
}

pub fn stage_and_commit(message: &str) {
    Command::new("git")
        .args(["add", "--all"])
        .output()
        .expect("failed to stage files");

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
