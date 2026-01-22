//! Binnacle CLI - A project state tracking tool for AI agents and humans.

use binnacle::action_log;
use binnacle::cli::{
    BugCommands, Cli, Commands, CommitCommands, ConfigCommands, DepCommands, GraphCommands,
    McpCommands, StoreCommands, SystemCommands, TaskCommands, TestCommands,
};
use binnacle::commands::{self, Output};
use binnacle::mcp;
use clap::Parser;
use std::env;
use std::path::{Path, PathBuf};
use std::process;
use std::time::Instant;

fn main() {
    let cli = Cli::parse();
    let human = cli.human_readable;

    // Get the current working directory as the repo path
    let repo_path = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    // Serialize command for logging
    let (cmd_name, args_json) = serialize_command(&cli.command);

    // Start timing
    let start = Instant::now();

    // Execute command
    let result = run_command(cli.command, &repo_path, human);

    // Calculate duration
    let duration = start.elapsed().as_millis() as u64;

    // Determine success/error
    let (success, error) = match &result {
        Ok(_) => (true, None),
        Err(e) => (false, Some(e.to_string())),
    };

    // Log the action (silently fails if logging is disabled or encounters errors)
    let _ = action_log::log_action(&repo_path, &cmd_name, args_json, success, error, duration);

    // Handle result
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
        Some(Commands::Orient) => {
            let result = commands::orient(repo_path)?;
            output(&result, human);
        }

        Some(Commands::Task { command }) => match command {
            TaskCommands::Create {
                title,
                short_name,
                priority,
                tag,
                assignee,
                description,
            } => {
                // Convert empty or whitespace-only string to None
                let short_name = short_name.filter(|s| !s.trim().is_empty());
                let result =
                    commands::task_create(repo_path, title, short_name, description, priority, tag, assignee)?;
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
                short_name,
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
                    short_name,
                    description,
                    priority,
                    status.as_deref(),
                    add_tag,
                    remove_tag,
                    assignee,
                )?;
                output(&result, human);
            }

            TaskCommands::Close { id, reason, force } => {
                let result = commands::task_close(repo_path, &id, reason, force)?;
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

        Some(Commands::Bug { command }) => match command {
            BugCommands::Create {
                title,
                priority,
                severity,
                tag,
                assignee,
                description,
                reproduction_steps,
                affected_component,
            } => {
                let result = commands::bug_create(
                    repo_path,
                    title,
                    description,
                    priority,
                    severity,
                    tag,
                    assignee,
                    reproduction_steps,
                    affected_component,
                )?;
                output(&result, human);
            }
            BugCommands::List {
                status,
                priority,
                severity,
                tag,
            } => {
                let result = commands::bug_list(
                    repo_path,
                    status.as_deref(),
                    priority,
                    severity.as_deref(),
                    tag.as_deref(),
                )?;
                output(&result, human);
            }
            BugCommands::Show { id } => {
                let result = commands::bug_show(repo_path, &id)?;
                output(&result, human);
            }
            BugCommands::Update {
                id,
                title,
                description,
                priority,
                status,
                severity,
                add_tag,
                remove_tag,
                assignee,
                reproduction_steps,
                affected_component,
            } => {
                let result = commands::bug_update(
                    repo_path,
                    &id,
                    title,
                    description,
                    priority,
                    status.as_deref(),
                    severity,
                    add_tag,
                    remove_tag,
                    assignee,
                    reproduction_steps,
                    affected_component,
                )?;
                output(&result, human);
            }
            BugCommands::Close { id, reason, force } => {
                let result = commands::bug_close(repo_path, &id, reason, force)?;
                output(&result, human);
            }
            BugCommands::Reopen { id } => {
                let result = commands::bug_reopen(repo_path, &id)?;
                output(&result, human);
            }
            BugCommands::Delete { id } => {
                let result = commands::bug_delete(repo_path, &id)?;
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
            let result = commands::doctor(repo_path)?;
            output(&result, human);
        }
        Some(Commands::Log { task_id }) => {
            let result = commands::log(repo_path, task_id.as_deref())?;
            output(&result, human);
        }
        Some(Commands::Compact) => {
            let result = commands::compact(repo_path)?;
            output(&result, human);
        }
        Some(Commands::Sync) => {
            not_implemented("sync", "", human);
        }
        Some(Commands::Config { command }) => match command {
            ConfigCommands::Get { key } => {
                let result = commands::config_get(repo_path, &key)?;
                output(&result, human);
            }
            ConfigCommands::Set { key, value } => {
                let result = commands::config_set(repo_path, &key, &value)?;
                output(&result, human);
            }
            ConfigCommands::List => {
                let result = commands::config_list(repo_path)?;
                output(&result, human);
            }
        },
        Some(Commands::Mcp { command }) => match command {
            McpCommands::Serve => {
                mcp::serve(repo_path);
            }
            McpCommands::Manifest => {
                mcp::manifest();
            }
        },
        Some(Commands::Graph { command }) => match command {
            GraphCommands::Components => {
                not_implemented("graph components", "", human);
            }
        },
        Some(Commands::System { command }) => match command {
            SystemCommands::Init => {
                let result = commands::init(repo_path)?;
                output(&result, human);
            }
            SystemCommands::Store { command } => match command {
                StoreCommands::Show => {
                    let result = commands::system_store_show(repo_path)?;
                    output(&result, human);
                }
                StoreCommands::Export { output: out_path, format } => {
                    let result = commands::system_store_export(repo_path, &out_path, &format)?;
                    // Don't output anything when writing to stdout (would corrupt the binary data)
                    if out_path != "-" {
                        output(&result, human);
                    }
                }
                StoreCommands::Import {
                    input,
                    r#type,
                    dry_run,
                } => {
                    let result = commands::system_store_import(&repo_path, &input, &r#type, dry_run)?;
                    output(&result, human);
                }
            },
        },
        #[cfg(feature = "gui")]
        Some(Commands::Gui { port }) => {
            run_gui(repo_path, port)?;
        }
        None => {
            // Default: show status summary
            match commands::status(repo_path) {
                Ok(summary) => output(&summary, human),
                Err(binnacle::Error::NotInitialized) => {
                    if human {
                        println!("Binnacle - Not initialized.");
                        println!(
                            "Run `bn system init` to initialize, then `bn task create \"Title\"` to add tasks."
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

/// Run the GUI web server
#[cfg(feature = "gui")]
fn run_gui(repo_path: &Path, port: u16) -> Result<(), binnacle::Error> {
    use binnacle::storage::Storage;

    // Ensure storage is initialized
    if !Storage::exists(repo_path)? {
        return Err(binnacle::Error::NotInitialized);
    }

    // Create tokio runtime and run the server
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| binnacle::Error::Other(format!("Failed to create runtime: {}", e)))?
        .block_on(async {
            binnacle::gui::start_server(repo_path, port)
                .await
                .map_err(|e| binnacle::Error::Other(format!("GUI server error: {}", e)))
        })
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

/// Serialize command to extract name and arguments for logging.
fn serialize_command(command: &Option<Commands>) -> (String, serde_json::Value) {
    match command {
        Some(Commands::Orient) => ("orient".to_string(), serde_json::json!({})),

        Some(Commands::Task { command }) => match command {
            TaskCommands::Create {
                title,
                short_name,
                priority,
                tag,
                assignee,
                description,
            } => (
                "task create".to_string(),
                serde_json::json!({
                    "title": title,
                    "short_name": short_name,
                    "priority": priority,
                    "tag": tag,
                    "assignee": assignee,
                    "description": description,
                }),
            ),
            TaskCommands::List {
                status,
                priority,
                tag,
            } => (
                "task list".to_string(),
                serde_json::json!({
                    "status": status,
                    "priority": priority,
                    "tag": tag,
                }),
            ),
            TaskCommands::Show { id } => (
                "task show".to_string(),
                serde_json::json!({ "id": id }),
            ),
            TaskCommands::Update {
                id,
                title,
                short_name,
                description,
                priority,
                status,
                add_tag,
                remove_tag,
                assignee,
            } => (
                "task update".to_string(),
                serde_json::json!({
                    "id": id,
                    "title": title,
                    "short_name": short_name,
                    "description": description,
                    "priority": priority,
                    "status": status,
                    "add_tag": add_tag,
                    "remove_tag": remove_tag,
                    "assignee": assignee,
                }),
            ),
            TaskCommands::Close { id, reason, force } => (
                "task close".to_string(),
                serde_json::json!({
                    "id": id,
                    "reason": reason,
                    "force": force,
                }),
            ),
            TaskCommands::Reopen { id } => (
                "task reopen".to_string(),
                serde_json::json!({ "id": id }),
            ),
            TaskCommands::Delete { id } => (
                "task delete".to_string(),
                serde_json::json!({ "id": id }),
            ),
        },

        Some(Commands::Bug { command }) => match command {
            BugCommands::Create {
                title,
                priority,
                severity,
                tag,
                assignee,
                description,
                reproduction_steps,
                affected_component,
            } => (
                "bug create".to_string(),
                serde_json::json!({
                    "title": title,
                    "priority": priority,
                    "severity": severity,
                    "tag": tag,
                    "assignee": assignee,
                    "description": description,
                    "reproduction_steps": reproduction_steps,
                    "affected_component": affected_component,
                }),
            ),
            BugCommands::List {
                status,
                priority,
                severity,
                tag,
            } => (
                "bug list".to_string(),
                serde_json::json!({
                    "status": status,
                    "priority": priority,
                    "severity": severity,
                    "tag": tag,
                }),
            ),
            BugCommands::Show { id } => ("bug show".to_string(), serde_json::json!({ "id": id })),
            BugCommands::Update {
                id,
                title,
                description,
                priority,
                status,
                severity,
                add_tag,
                remove_tag,
                assignee,
                reproduction_steps,
                affected_component,
            } => (
                "bug update".to_string(),
                serde_json::json!({
                    "id": id,
                    "title": title,
                    "description": description,
                    "priority": priority,
                    "status": status,
                    "severity": severity,
                    "add_tag": add_tag,
                    "remove_tag": remove_tag,
                    "assignee": assignee,
                    "reproduction_steps": reproduction_steps,
                    "affected_component": affected_component,
                }),
            ),
            BugCommands::Close { id, reason, force } => (
                "bug close".to_string(),
                serde_json::json!({
                    "id": id,
                    "reason": reason,
                    "force": force,
                }),
            ),
            BugCommands::Reopen { id } => (
                "bug reopen".to_string(),
                serde_json::json!({ "id": id }),
            ),
            BugCommands::Delete { id } => (
                "bug delete".to_string(),
                serde_json::json!({ "id": id }),
            ),
        },

        Some(Commands::Dep { command }) => match command {
            DepCommands::Add { child, parent } => (
                "dep add".to_string(),
                serde_json::json!({
                    "child": child,
                    "parent": parent,
                }),
            ),
            DepCommands::Rm { child, parent } => (
                "dep rm".to_string(),
                serde_json::json!({
                    "child": child,
                    "parent": parent,
                }),
            ),
            DepCommands::Show { id } => (
                "dep show".to_string(),
                serde_json::json!({ "id": id }),
            ),
        },

        Some(Commands::Test { command }) => match command {
            TestCommands::Create {
                name,
                cmd,
                dir,
                task,
            } => (
                "test create".to_string(),
                serde_json::json!({
                    "name": name,
                    "cmd": cmd,
                    "dir": dir,
                    "task": task,
                }),
            ),
            TestCommands::List { task } => (
                "test list".to_string(),
                serde_json::json!({ "task": task }),
            ),
            TestCommands::Show { id } => (
                "test show".to_string(),
                serde_json::json!({ "id": id }),
            ),
            TestCommands::Link { test_id, task_id } => (
                "test link".to_string(),
                serde_json::json!({
                    "test_id": test_id,
                    "task_id": task_id,
                }),
            ),
            TestCommands::Unlink { test_id, task_id } => (
                "test unlink".to_string(),
                serde_json::json!({
                    "test_id": test_id,
                    "task_id": task_id,
                }),
            ),
            TestCommands::Run {
                id,
                task,
                all,
                failed,
            } => (
                "test run".to_string(),
                serde_json::json!({
                    "id": id,
                    "task": task,
                    "all": all,
                    "failed": failed,
                }),
            ),
        },

        Some(Commands::Commit { command }) => match command {
            CommitCommands::Link { sha, task_id } => (
                "commit link".to_string(),
                serde_json::json!({
                    "sha": sha,
                    "task_id": task_id,
                }),
            ),
            CommitCommands::Unlink { sha, task_id } => (
                "commit unlink".to_string(),
                serde_json::json!({
                    "sha": sha,
                    "task_id": task_id,
                }),
            ),
            CommitCommands::List { task_id } => (
                "commit list".to_string(),
                serde_json::json!({ "task_id": task_id }),
            ),
        },

        Some(Commands::Ready) => ("ready".to_string(), serde_json::json!({})),

        Some(Commands::Blocked) => ("blocked".to_string(), serde_json::json!({})),

        Some(Commands::Doctor) => ("doctor".to_string(), serde_json::json!({})),

        Some(Commands::Log { task_id }) => (
            "log".to_string(),
            serde_json::json!({ "task_id": task_id }),
        ),

        Some(Commands::Compact) => ("compact".to_string(), serde_json::json!({})),

        Some(Commands::Sync) => ("sync".to_string(), serde_json::json!({})),

        Some(Commands::Config { command }) => match command {
            ConfigCommands::Get { key } => (
                "config get".to_string(),
                serde_json::json!({ "key": key }),
            ),
            ConfigCommands::Set { key, value } => (
                "config set".to_string(),
                serde_json::json!({
                    "key": key,
                    "value": value,
                }),
            ),
            ConfigCommands::List => ("config list".to_string(), serde_json::json!({})),
        },

        Some(Commands::Mcp { command }) => match command {
            McpCommands::Serve => ("mcp serve".to_string(), serde_json::json!({})),
            McpCommands::Manifest => ("mcp manifest".to_string(), serde_json::json!({})),
        },

        Some(Commands::Graph { command }) => match command {
            GraphCommands::Components => ("graph components".to_string(), serde_json::json!({})),
        },

        Some(Commands::System { command }) => match command {
            SystemCommands::Init => ("system init".to_string(), serde_json::json!({})),
            SystemCommands::Store { command } => match command {
                StoreCommands::Show => ("system store show".to_string(), serde_json::json!({})),
                StoreCommands::Export { output, format } => (
                    "system store export".to_string(),
                    serde_json::json!({
                        "output": output,
                        "format": format,
                    }),
                ),
                StoreCommands::Import {
                    input,
                    r#type,
                    dry_run,
                } => (
                    "system store import".to_string(),
                    serde_json::json!({
                        "input": input,
                        "type": r#type,
                        "dry_run": dry_run,
                    }),
                ),
            },
        },

        #[cfg(feature = "gui")]
        Some(Commands::Gui { port }) => (
            "gui".to_string(),
            serde_json::json!({ "port": port }),
        ),

        None => ("status".to_string(), serde_json::json!({})),
    }
}
