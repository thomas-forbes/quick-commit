use colored::*;
use git2::Status;
use rustyline::DefaultEditor;
use std::io::{self, stdout, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Format "Label: value" with only "Label" colored using the given color.
pub fn labeled(label: &str, value: &str, color_fn: fn(&str) -> ColoredString) -> String {
    format!("{}: {}", color_fn(label), value)
}

pub fn prompt_input(label: &str) -> String {
    if let Some(pos) = label.find(':') {
        let colored_part = &label[..pos];
        let rest = &label[pos..];
        print!("{}{}", colored_part.cyan(), rest);
    } else {
        print!("{}", label.cyan());
    }
    stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .expect("Failed to read input");
    input.trim().to_string()
}

/// Wrap a label in cyan ANSI codes with \x01/\x02 markers so rustyline
/// doesn't count the escape bytes as visible width.
/// Only the part before the first ':' is colored.
fn colored_label(label: &str) -> String {
    if let Some(pos) = label.find(':') {
        let colored_part = &label[..pos];
        let rest = &label[pos..];
        format!("\x01\x1b[36m\x02{}\x01\x1b[0m\x02{}", colored_part, rest)
    } else {
        format!("\x01\x1b[36m\x02{}\x01\x1b[0m\x02", label)
    }
}

/// Shows `label` (colored) with `initial` pre-filled as editable text.
/// The user can edit inline, press Enter to accept, or Ctrl+C to cancel.
pub fn editable_prompt(label: &str, initial: &str) -> String {
    let prompt = colored_label(label);
    let mut rl = DefaultEditor::new().expect("Failed to initialize line editor");
    match rl.readline_with_initial(&prompt, (initial, "")) {
        Ok(line) => line.trim().to_string(),
        Err(_) => {
            println!("\n{}", "Cancelled.".yellow());
            std::process::exit(0);
        }
    }
}

pub fn print_repo_name(name: &str) {
    println!("{}", name.italic().cyan());
}

pub fn print_changes(files: &[(String, Status)], insertions: usize, deletions: usize) {
    for (path, status) in files {
        match *status {
            Status::INDEX_NEW | Status::WT_NEW => {
                print!("{}", ("+ ".to_owned() + path).green())
            }
            Status::INDEX_MODIFIED | Status::WT_MODIFIED => {
                print!("{}", ("M ".to_owned() + path).yellow())
            }
            Status::INDEX_DELETED | Status::WT_DELETED => {
                print!("{}", ("- ".to_owned() + path).red())
            }
            _ => continue,
        }
        println!();
    }
    println!(
        "{} files changed, {} lines added, {} lines deleted",
        files.len().to_string().yellow(),
        ("+".to_owned() + &insertions.to_string()).green(),
        ("-".to_owned() + &deletions.to_string()).red(),
    );
}

// -- Spinner ---------------------------------------------------------------

pub struct Spinner {
    running: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Spinner {
    pub fn start(message: &str) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let flag = running.clone();
        let msg = message.to_string();

        let handle = thread::spawn(move || {
            let frames = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
            let mut i = 0;
            while flag.load(Ordering::Relaxed) {
                print!(
                    "\r {} {}",
                    frames[i % frames.len()].to_string().cyan(),
                    msg.dimmed()
                );
                stdout().flush().unwrap();
                thread::sleep(Duration::from_millis(80));
                i += 1;
            }
            // Clear the spinner line
            print!("\r\x1b[2K");
            stdout().flush().unwrap();
        });

        Spinner {
            running,
            handle: Some(handle),
        }
    }

    pub fn stop(mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            h.join().unwrap();
        }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            h.join().unwrap();
        }
    }
}
