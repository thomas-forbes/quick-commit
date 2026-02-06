use colored::*;
use git2::{
    Config, DiffOptions, Error, ErrorCode, Repository, Signature, Status, StatusOptions, StatusShow,
};
use std::env;
use std::io::{self, stdout, Write};
use std::path::Path;
use std::process::{Command, Stdio};

fn get_stats(repo: &Repository) -> Result<(usize, usize, Vec<(String, Status)>), Error> {
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

    // Get file statuses for both staged and unstaged changes
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
        // Check for both index (staged) and workdir (unstaged) changes
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

fn branch_exists(repo: &Repository, branch_name: &str) -> Result<bool, Error> {
    // Try to find the branch locally
    match repo.find_branch(branch_name, git2::BranchType::Local) {
        Ok(_) => return Ok(true),
        Err(_) => {
            // If not found locally, try to find it remotely
            match repo.find_branch(branch_name, git2::BranchType::Remote) {
                Ok(_) => return Ok(true),
                Err(_) => return Ok(false),
            }
        }
    }
}

fn get_remote_url(repo: &Repository, remote_name: &str) -> Result<String, Error> {
    let remote = repo.find_remote(remote_name)?;
    Ok(remote.url().unwrap_or_default().to_string())
}

fn main() {
    let repo = Repository::discover(".").unwrap_or_else(|_| {
        eprintln!("{}", "Error opening git repo •◠•".red());
        std::process::exit(1);
    });
    println!(
        "{}",
        repo.path()
            .parent()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("no name")
            .italic()
            .cyan()
    );

    // commit info
    let (lines_inserted, lines_deleted, files) = get_stats(&repo).unwrap_or_else(|_| {
        eprintln!("{}", "Error reading git info •◠•".red());
        std::process::exit(1);
    });
    if files.len() == 0 {
        println!("{}", "No changes to commit •◡•".yellow());
        std::process::exit(0);
    }
    for (path, status) in &files {
        let print_path = path;
        match status {
            &Status::INDEX_NEW | &Status::WT_NEW => {
                print!("{}", ("+ ".to_owned() + &print_path).green())
            }
            &Status::INDEX_MODIFIED | &Status::WT_MODIFIED => {
                print!("{}", ("M ".to_owned() + &print_path).yellow())
            }
            &Status::INDEX_DELETED | &Status::WT_DELETED => {
                print!("{}", ("- ".to_owned() + &print_path).red())
            }
            _ => continue,
        }
        println!();
    }
    println!(
        "\n{} files changed, {} lines added, {} lines deleted",
        files.len().to_string().yellow(),
        ("+".to_owned() + &lines_inserted.to_string()).green(),
        ("-".to_owned() + &lines_deleted.to_string()).red(),
    );

    // commit message
    print!("{}", ": ".cyan());
    stdout().flush().unwrap();
    let mut commit_title = String::new();
    io::stdin()
        .read_line(&mut commit_title)
        .expect("Failed to read input");
    let commit_title = commit_title.trim();

    let branch_name = commit_title.replace(" ", "-");

    if branch_exists(&repo, &branch_name).unwrap() {
        eprintln!("{}", "Branch already exists •◠•".red());
        std::process::exit(1);
    }

    Command::new("git")
        .arg("checkout")
        .arg("-b")
        .arg(&branch_name)
        .output()
        .expect("failed to execute process");

    Command::new("git")
        .arg("add")
        .arg("--all")
        .output()
        .expect("failed to execute process");

    Command::new("git")
        .arg("commit")
        .arg("-m")
        .arg(&commit_title)
        .output()
        .expect("failed to execute process");

    // ask if they want to push
    print!("{}", "Push to remote? (Y/n): ".cyan());
    stdout().flush().unwrap();
    let mut push = String::new();
    io::stdin()
        .read_line(&mut push)
        .expect("Failed to read input");
    let push = push.trim();
    if push == "n" {
        std::process::exit(0);
    }

    let out = Command::new("git")
        .arg("push")
        .output()
        .expect("failed to push");
    print!("{}", String::from_utf8_lossy(&out.stdout).cyan());

    let remote_url = get_remote_url(&repo, "origin").unwrap_or_else(|_| {
        eprintln!("{}", "Error reading remote url •◠•".red());
        std::process::exit(1);
    });

    println!(
        "{}",
        format!(
            "Branch {} created and pushed to {}",
            branch_name, remote_url
        )
        .purple()
    );
}
