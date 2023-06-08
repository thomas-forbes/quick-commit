use colored::*;
use git2::{Config, Cred, Direction, ErrorCode, PushOptions, Repository, Signature, StatusOptions};
use std::io::{self, stdout, Write};
use std::path::Path;
use std::process::Command;

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

fn push(repo: &Repository) -> Result<(), git2::Error> {
    Command::new("git")
        .arg("push")
        .output()
        .expect("failed to execute process");

    Ok(())
}
fn main() {
    let repo = Repository::open(".").expect("Failed to open repository");

    match stage(&repo) {
        Ok(files) => {
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
            println!(
                "\n{} files staged, {} added, {} lines deleted",
                files.len().to_string().cyan(),
                "+0".green(),
                "-0".red(),
            );

            // commit message
            print!("{}", ": ".magenta());
            stdout().flush().unwrap();
            let mut commit_title = String::new();
            io::stdin()
                .read_line(&mut commit_title)
                .expect("Failed to read input");
            let commit_title = commit_title.trim();

            // commit
            match commit(&repo, commit_title) {
                // push
                Ok(()) => match push(&repo) {
                    Ok(()) => {
                        println!("{}", "Success!".green());
                    }
                    Err(e) => eprintln!("Error: {}", e),
                },
                Err(e) => eprintln!("Error: {}", e),
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
