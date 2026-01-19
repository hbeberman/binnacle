//! Binnacle CLI - A project state tracking tool for AI agents and humans.

use binnacle::cli::{Cli, Commands};
use clap::Parser;

fn main() {
    let cli = Cli::parse();

    let human = cli.human_readable;

    match cli.command {
        Some(Commands::Init) => {
            if human {
                println!("Initializing binnacle...");
            } else {
                println!(r#"{{"status": "not_implemented", "command": "init"}}"#);
            }
        }
        Some(Commands::Task { command }) => {
            not_implemented("task", &format!("{:?}", command), human);
        }
        Some(Commands::Dep { command }) => {
            not_implemented("dep", &format!("{:?}", command), human);
        }
        Some(Commands::Test { command }) => {
            not_implemented("test", &format!("{:?}", command), human);
        }
        Some(Commands::Commit { command }) => {
            not_implemented("commit", &format!("{:?}", command), human);
        }
        Some(Commands::Ready) => {
            not_implemented("ready", "", human);
        }
        Some(Commands::Blocked) => {
            not_implemented("blocked", "", human);
        }
        Some(Commands::Doctor) => {
            not_implemented("doctor", "", human);
        }
        Some(Commands::Log { task_id }) => {
            not_implemented("log", &task_id.unwrap_or_default(), human);
        }
        Some(Commands::Compact) => {
            not_implemented("compact", "", human);
        }
        Some(Commands::Sync) => {
            not_implemented("sync", "", human);
        }
        Some(Commands::Config { command }) => {
            not_implemented("config", &format!("{:?}", command), human);
        }
        Some(Commands::Mcp { command }) => {
            not_implemented("mcp", &format!("{:?}", command), human);
        }
        None => {
            // Default: show status summary
            if human {
                println!("Binnacle - No tasks tracked yet.");
                println!(
                    "Run `bn init` to initialize, then `bn task create \"Title\"` to add tasks."
                );
            } else {
                println!(r#"{{"tasks": [], "tests": [], "ready": []}}"#);
            }
        }
    }
}

/// Print a not-implemented message for a command.
fn not_implemented(command: &str, subcommand: &str, human: bool) {
    if human {
        if subcommand.is_empty() {
            println!("Command '{}' is not yet implemented.", command);
        } else {
            println!(
                "Command '{} {}' is not yet implemented.",
                command, subcommand
            );
        }
    } else if subcommand.is_empty() {
        println!(
            r#"{{"status": "not_implemented", "command": "{}"}}"#,
            command
        );
    } else {
        println!(
            r#"{{"status": "not_implemented", "command": "{}", "subcommand": "{}"}}"#,
            command, subcommand
        );
    }
}
