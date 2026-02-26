mod ai;
mod config;
mod git;
mod ui;

use colored::*;
use git2::Repository;
use std::thread;

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

    git::stage_all();
    let (insertions, deletions, files) = git::get_stats(&repo).unwrap_or_else(|_| {
        eprintln!("{}", "Error reading git info •◠•".red());
        std::process::exit(1);
    });
    if files.is_empty() {
        println!("{}", "No changes to commit •◡•".yellow());
        std::process::exit(0);
    }
    ui::print_changes(&files, insertions, deletions);

    // Resolve config before spawning (may prompt user on first run)
    let api_key = config::get_api_key().unwrap_or_else(|e| {
        eprintln!("{}", format!("Error: {}", e).red());
        std::process::exit(1);
    });
    let model = config::get_model().unwrap_or_else(|e| {
        eprintln!("{}", format!("Error: {}", e).red());
        std::process::exit(1);
    });

    let semantic_types = config::get_semantic_types().unwrap_or_else(|e| {
        eprintln!("{}", format!("Error: {}", e).red());
        std::process::exit(1);
    });

    let diff = git::build_diff_context(&files, insertions, deletions);

    // Start AI generation in background — always request branch name so we
    // don't block on the user's branch choice before firing the request.
    let ai_handle = thread::spawn(move || {
        ai::generate_commit_info(&diff, true, &api_key, &model, &semantic_types)
    });

    // While AI is working, ask the user about creating a new branch
    let create_new_branch =
        ui::prompt_input("\nCreate new branch? (y/N): ").eq_ignore_ascii_case("y");

    // Wait for the AI result (spinner shown only while still pending)
    let spinner = ui::Spinner::start("Generating commit message...");
    let result = ai_handle
        .join()
        .unwrap_or_else(|_| Err("AI thread panicked".to_string()));
    spinner.stop();

    let (commit_message, branch_name) = result.unwrap_or_else(|e| {
        eprintln!("{}", format!("AI generation failed: {}", e).red());
        eprintln!("{}", "Falling back to manual input.".yellow());
        (String::new(), Some(String::new()))
    });

    // Present AI-generated text as editable — just press Enter to accept, or edit inline
    let final_message = ui::editable_prompt("Commit: ", &commit_message.trim())
        .trim()
        .to_string();
    let final_branch = if create_new_branch {
        Some(
            ui::editable_prompt("Branch: ", &branch_name.unwrap_or_default().trim())
                .trim()
                .to_string(),
        )
    } else {
        None
    };

    if let Some(ref branch) = final_branch {
        git::create_branch(&repo, branch);
    }

    git::stage_and_commit(&final_message);

    let push_input = ui::prompt_input("Push to remote? (Y/n): ");
    if !push_input.eq_ignore_ascii_case("n") {
        git::push(&repo, final_branch.is_some());
    }
}
