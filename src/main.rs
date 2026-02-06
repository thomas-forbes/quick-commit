mod ai;
mod git;
mod ui;

use colored::*;
use git2::Repository;

fn main() {
    let repo = Repository::discover(".").unwrap_or_else(|_| {
        eprintln!("{}", "Error opening git repo •◠•".red());
        std::process::exit(1);
    });

    let repo_name = repo
        .path()
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("no name");
    ui::print_repo_name(repo_name);

    let (insertions, deletions, files) = git::get_stats(&repo).unwrap_or_else(|_| {
        eprintln!("{}", "Error reading git info •◠•".red());
        std::process::exit(1);
    });
    if files.is_empty() {
        println!("{}", "No changes to commit •◡•".yellow());
        std::process::exit(0);
    }
    ui::print_changes(&files, insertions, deletions);

    let create_new_branch = ui::prompt_input("\nCreate new branch? (y/N): ")
        .eq_ignore_ascii_case("y");

    let diff = git::build_diff_context(&files, insertions, deletions);

    let spinner = ui::Spinner::start("Generating commit message...");
    let result = ai::generate_commit_info(&diff, create_new_branch);
    spinner.stop();

    let (commit_message, branch_name) = result.unwrap_or_else(|e| {
        eprintln!("{}", format!("AI generation failed: {}", e).red());
        eprintln!("{}", "Falling back to manual input.".yellow());
        (String::new(), if create_new_branch { Some(String::new()) } else { None })
    });

    // Present AI-generated text as editable — just press Enter to accept, or edit inline
    let final_message = ui::editable_prompt("Commit: ", &commit_message);
    let final_branch = if create_new_branch {
        Some(ui::editable_prompt("Branch: ", &branch_name.unwrap_or_default()))
    } else {
        None
    };

    println!("\n");
    if let Some(ref branch) = final_branch {
        git::create_branch(&repo, branch);
    }

    git::stage_and_commit(&final_message);

    let push_input = ui::prompt_input("\nPush to remote? (Y/n): ");
    if !push_input.eq_ignore_ascii_case("n") {
        git::push(&repo, final_branch.is_some());
    }
}
