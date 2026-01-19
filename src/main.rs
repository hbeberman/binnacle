//! Binnacle CLI - A project state tracking tool for AI agents and humans.

use binnacle::cli::{Cli, Commands, CommitCommands, DepCommands, TaskCommands, TestCommands};
use binnacle::commands::{self, Output};
use clap::Parser;
use std::env;
use std::path::{Path, PathBuf};
use std::process;

fn main() {
    let cli = Cli::parse();
    let human = cli.human_readable;

    // Get the current working directory as the repo path
    let repo_path = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let result = run_command(cli.command, &repo_path, human);

    if let Err(e) = result {
        if human {
            eprintln!("Error: {}", e);
        } else {
            eprintln!(r#"{{"error": "{}"}}"#, e);
        }
        process::exit(1);
    }
}

fn run_command(
    command: Option<Commands>,
    repo_path: &Path,
    human: bool,
) -> Result<(), binnacle::Error> {
    match command {
        Some(Commands::Init) => {
            let result = commands::init(repo_path)?;
            output(&result, human);
        }

        Some(Commands::Task { command }) => match command {
            TaskCommands::Create {
                title,
                priority,
                tag,
                assignee,
                description,
            } => {
                let result =
                    commands::task_create(repo_path, title, description, priority, tag, assignee)?;
                output(&result, human);
            }

            TaskCommands::List {
                status,
                priority,
                tag,
            } => {
                let result =
                    commands::task_list(repo_path, status.as_deref(), priority, tag.as_deref())?;
                output(&result, human);
            }

            TaskCommands::Show { id } => {
                let result = commands::task_show(repo_path, &id)?;
                output(&result, human);
            }

            TaskCommands::Update {
                id,
                title,
                description,
                priority,
                status,
                add_tag,
                remove_tag,
                assignee,
            } => {
                let result = commands::task_update(
                    repo_path,
                    &id,
                    title,
                    description,
                    priority,
                    status.as_deref(),
                    add_tag,
                    remove_tag,
                    assignee,
                )?;
                output(&result, human);
            }

            TaskCommands::Close { id, reason } => {
                let result = commands::task_close(repo_path, &id, reason)?;
                output(&result, human);
            }

            TaskCommands::Reopen { id } => {
                let result = commands::task_reopen(repo_path, &id)?;
                output(&result, human);
            }

            TaskCommands::Delete { id } => {
                let result = commands::task_delete(repo_path, &id)?;
                output(&result, human);
            }
        },

        Some(Commands::Dep { command }) => match command {
            DepCommands::Add { child, parent } => {
                let result = commands::dep_add(repo_path, &child, &parent)?;
                output(&result, human);
            }
            DepCommands::Rm { child, parent } => {
                let result = commands::dep_rm(repo_path, &child, &parent)?;
                output(&result, human);
            }
            DepCommands::Show { id } => {
                let result = commands::dep_show(repo_path, &id)?;
                output(&result, human);
            }
        },
        Some(Commands::Test { command }) => match command {
            TestCommands::Create {
                name,
                cmd,
                dir,
                task,
            } => {
                let result = commands::test_create(repo_path, name, cmd, dir, task)?;
                output(&result, human);
            }
            TestCommands::List { task } => {
                let result = commands::test_list(repo_path, task.as_deref())?;
                output(&result, human);
            }
            TestCommands::Show { id } => {
                let result = commands::test_show(repo_path, &id)?;
                output(&result, human);
            }
            TestCommands::Link { test_id, task_id } => {
                let result = commands::test_link(repo_path, &test_id, &task_id)?;
                output(&result, human);
            }
            TestCommands::Unlink { test_id, task_id } => {
                let result = commands::test_unlink(repo_path, &test_id, &task_id)?;
                output(&result, human);
            }
            TestCommands::Run {
                id,
                task,
                all,
                failed,
            } => {
                let result =
                    commands::test_run(repo_path, id.as_deref(), task.as_deref(), all, failed)?;
                output(&result, human);
            }
        },
        Some(Commands::Commit { command }) => match command {
            CommitCommands::Link { sha, task_id } => {
                let result = commands::commit_link(repo_path, &sha, &task_id)?;
                output(&result, human);
            }
            CommitCommands::Unlink { sha, task_id } => {
                let result = commands::commit_unlink(repo_path, &sha, &task_id)?;
                output(&result, human);
            }
            CommitCommands::List { task_id } => {
                let result = commands::commit_list(repo_path, &task_id)?;
                output(&result, human);
            }
        },
        Some(Commands::Ready) => {
            let result = commands::ready(repo_path)?;
            output(&result, human);
        }
        Some(Commands::Blocked) => {
            let result = commands::blocked(repo_path)?;
            output(&result, human);
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
            match commands::status(repo_path) {
                Ok(summary) => output(&summary, human),
                Err(binnacle::Error::NotInitialized) => {
                    if human {
                        println!("Binnacle - Not initialized.");
                        println!(
                            "Run `bn init` to initialize, then `bn task create \"Title\"` to add tasks."
                        );
                    } else {
                        println!(r#"{{"initialized": false, "tasks": [], "ready": []}}"#);
                    }
                }
                Err(e) => return Err(e),
            }
        }
    }

    Ok(())
}

/// Print output in JSON or human-readable format.
fn output<T: Output>(result: &T, human: bool) {
    if human {
        println!("{}", result.to_human());
    } else {
        println!("{}", result.to_json());
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
