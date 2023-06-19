use colored::*;
use git2::{Config, ErrorCode, Repository, Signature, StatusOptions};
use std::env;
use std::io::{self, stdout, Write};
use std::path::Path;
use std::process::{Command, Stdio};

fn stage(repo: &Repository) -> Result<Vec<(String, git2::Status)>, git2::Error> {
    let mut index = repo.index()?;

    let mut options = StatusOptions::new();
    options.include_untracked(true).recurse_untracked_dirs(true);

    let mut files: Vec<(String, git2::Status)> = Vec::new();

    for entry in repo.statuses(Some(&mut options))?.iter() {
        let path = Path::new(std::str::from_utf8(entry.path_bytes()).unwrap());

        match entry.status() {
            status if status.intersects(git2::Status::INDEX_NEW | git2::Status::WT_NEW) => {
                files.push((path.display().to_string(), git2::Status::INDEX_NEW));

                index.add_path(&path)?;
            }
            status
                if status.intersects(git2::Status::INDEX_MODIFIED | git2::Status::WT_MODIFIED) =>
            {
                files.push((path.display().to_string(), git2::Status::INDEX_MODIFIED));

                index.add_path(&path)?;
            }
            status if status.intersects(git2::Status::INDEX_DELETED | git2::Status::WT_DELETED) => {
                // test
                files.push((path.display().to_string(), git2::Status::INDEX_DELETED));

                index.remove_path(&path)?;
            }
            _ => continue,
        }
    }

    index.write()?; // Write the changes to the index

    Ok(files)
}

fn commit(repo: &Repository, message: &str) -> Result<(), git2::Error> {
    let mut index = repo.index()?;
    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    let config = Config::open_default()?;
    let name = config.get_string("user.name")?;
    let email = config.get_string("user.email")?;

    let signature = Signature::now(&name, &email)?;

    let head = repo.head();
    let head = match head {
        Ok(head) => head,
        Err(ref e) if e.code() == ErrorCode::UnbornBranch => {
            repo.commit(Some("HEAD"), &signature, &signature, message, &tree, &[])?;
            return Ok(());
        }
        Err(e) => return Err(e),
    };

    let head_commit = repo.find_commit(head.target().unwrap())?;

    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        message,
        &tree,
        &[&head_commit],
    )?;

    Ok(())
}

fn lines(repo: &Repository) -> Result<(usize, usize), git2::Error> {
    let mut index = repo.index()?;
    let oid = index.write_tree()?;
    let tree = repo.find_tree(oid)?;

    let head_commit = repo.head()?.peel_to_commit()?;
    let head_tree = head_commit.tree()?;

    let diff = repo.diff_tree_to_tree(Some(&head_tree), Some(&tree), None)?;

    Ok((diff.stats()?.insertions(), diff.stats()?.deletions()))
}

fn run_background_process() {
    // push
    let mut child = Command::new("git")
        .arg("push")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|_| {
            eprintln!("{}", "Unable to call 'git push' •◠•".red());
            std::process::exit(1);
        });

    let success = child.wait().expect("Failed to wait on child process");

    if !success.success() {
        eprintln!("\n{}", "Error pushing code •◠•".red());
    } else {
        print!("\n{}", "pushed code 🚀 ".green());
        let _ = Command::new("\n")
            .output()
            .expect("failed to execute process");
    }
}
fn main() {
    if env::var("RUN_BACKGROUND_TASK").is_ok() {
        run_background_process();
        std::process::exit(0);
    }

    //     // Your larger program continues...
    // }
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

    // stage changes
    let files = stage(&repo).unwrap_or_else(|_| {
        eprintln!("{}", "Error staging files •◠•".red());
        std::process::exit(1);
    });
    if files.len() == 0 {
        println!("{}", "No changes to commit •◡•".yellow());
        std::process::exit(0);
    }
    for (path, status) in &files {
        let print_path = path;
        match status {
            &git2::Status::INDEX_NEW => {
                print!("{}", ("+ ".to_owned() + &print_path).green())
            }
            &git2::Status::INDEX_MODIFIED => {
                print!("{}", ("M ".to_owned() + &print_path).yellow())
            }
            &git2::Status::INDEX_DELETED => {
                print!("{}", ("- ".to_owned() + &print_path).red())
            }
            _ => continue,
        }
        println!();
    }

    // commit info
    let (lines_inserted, lines_deleted) = lines(&repo).unwrap_or_else(|_| {
        eprintln!("{}", "Error reading git info •◠•".red());
        std::process::exit(1);
    });
    println!(
        "\n{} files staged, {} lines added, {} lines deleted",
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

    // commit
    commit(&repo, commit_title).unwrap_or_else(|_| {
        eprintln!("{}", "Error committing changes •◠•".red());
        std::process::exit(1);
    });

    let current_exe = env::current_exe().expect("Failed to get current executable");

    Command::new(current_exe)
        .env("RUN_BACKGROUND_TASK", "1")
        .spawn()
        .expect("Failed to start background process");
}
