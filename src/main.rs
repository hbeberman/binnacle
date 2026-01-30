//! Binnacle CLI - A project state tracking tool for AI agents and humans.

use binnacle::action_log;
#[cfg(feature = "gui")]
use binnacle::cli::GuiCommands;
#[cfg(feature = "tmux")]
use binnacle::cli::TmuxCommands;
use binnacle::cli::{
    AgentCommands, BugCommands, Cli, Commands, CommitCommands, ConfigCommands, ContainerCommands,
    CopilotCommands, DocCommands, EmitTemplate, GraphCommands, HooksCommands, IdeaCommands,
    LinkCommands, LogCommands, McpCommands, MilestoneCommands, MissionCommands, QueueCommands,
    SearchCommands, StoreCommands, SystemCommands, TaskCommands, TestCommands,
};
use binnacle::commands::{self, Output};
use binnacle::mcp;
use binnacle::models::DocType;
use binnacle::storage::find_git_root;
use clap::Parser;
use std::env;
use std::path::{Path, PathBuf};
use std::process;
use std::time::Instant;

fn main() {
    let cli = Cli::parse();
    let human = cli.human_readable;

    // Determine repo path: --repo flag > BN_REPO env > auto-detect git root > cwd
    let repo_path = resolve_repo_path(cli.repo_path, human);

    // Serialize command for logging
    let (cmd_name, args_json) = serialize_command(&cli.command);

    // Start timing
    let start = Instant::now();

    // Execute command
    let result = run_command(cli.command, &repo_path, human);

    // Track agent activity (if this process is a registered agent)
    commands::track_agent_activity(&repo_path);

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

/// Resolve the repository path based on explicit flag, environment variable, or auto-detection.
///
/// Priority: --repo flag > BN_REPO env var > git root detection > current working directory
///
/// When an explicit path is provided (via -C/--repo or BN_REPO), it is used literally
/// without git root detection. This allows targeting specific subdirectories even within
/// a git repository.
///
/// When no explicit path is given, we auto-detect the git root from the current directory
/// to ensure consistent storage regardless of which subdirectory the user runs from.
fn resolve_repo_path(explicit_path: Option<PathBuf>, human: bool) -> PathBuf {
    match explicit_path {
        Some(path) => {
            // Explicit path specified - verify it exists, use it literally (no git root detection)
            if !path.exists() {
                if human {
                    eprintln!(
                        "Error: Specified repo path does not exist: {}",
                        path.display()
                    );
                } else {
                    eprintln!(
                        r#"{{"error": "Specified repo path does not exist: {}"}}"#,
                        path.display()
                    );
                }
                process::exit(1);
            }
            path
        }
        None => {
            // Auto-detect: try git root first, fall back to cwd
            let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            find_git_root(&cwd).unwrap_or(cwd)
        }
    }
}

fn run_command(
    command: Option<Commands>,
    repo_path: &Path,
    human: bool,
) -> Result<(), binnacle::Error> {
    match command {
        Some(Commands::Orient {
            agent_type,
            init,
            name,
            register,
            dry_run,
        }) => {
            match commands::orient(repo_path, &agent_type, init, name, register, dry_run) {
                Ok(result) => output(&result, human),
                Err(binnacle::Error::NotInitialized) => {
                    // Provide helpful error message for uninitialized database
                    if human {
                        eprintln!("Error: No binnacle database found.\n");
                        eprintln!("To initialize a new database:");
                        eprintln!(
                            "    bn system init        # Interactive, recommended for humans"
                        );
                        eprintln!("    bn orient --init      # Non-interactive, for AI agents\n");
                        eprintln!("Database location: {}", repo_path.display());
                    } else {
                        let err = serde_json::json!({
                            "error": "No binnacle database found",
                            "hint": "Human should run 'bn system init' (interactive). AI agents: use 'bn orient --init' (non-interactive, conservative defaults).",
                            "path": repo_path
                        });
                        eprintln!("{}", err);
                    }
                    process::exit(1);
                }
                Err(e) => return Err(e),
            }
        }

        Some(Commands::Goodbye {
            reason,
            dry_run,
            force,
        }) => {
            let result = commands::goodbye(repo_path, reason, force)?;
            output(&result, human);

            // Actually terminate the grandparent process after output (unless dry-run)
            // We target grandparent because: agent → shell → bn goodbye
            // Note: result.terminated is false in MCP mode to prevent killing the MCP server
            if !dry_run && result.terminated {
                // Use 5 second timeout before SIGKILL
                commands::terminate_process(result.grandparent_pid, 5);
            }
        }

        Some(Commands::Show { id }) => {
            let result = commands::generic_show(repo_path, &id)?;
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
                queue,
                check_complexity,
                force,
            } => {
                // Convert empty or whitespace-only string to None
                let short_name = short_name.filter(|s| !s.trim().is_empty());

                // Check complexity if requested (and not forcing)
                if check_complexity && !force {
                    let result = commands::task_create_with_complexity_check(
                        repo_path,
                        title,
                        short_name,
                        description,
                        priority,
                        tag,
                        assignee,
                        queue,
                    )?;
                    output(&result, human);
                } else {
                    let result = commands::task_create_with_queue(
                        repo_path,
                        title,
                        short_name,
                        description,
                        priority,
                        tag,
                        assignee,
                        queue,
                    )?;
                    output(&result, human);
                }
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
                force,
                keep_closed,
                reopen,
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
                    force,
                    keep_closed,
                    reopen,
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
                short_name,
                priority,
                severity,
                tag,
                assignee,
                description,
                reproduction_steps,
                affected_component,
                queue,
            } => {
                let result = commands::bug_create_with_queue(
                    repo_path,
                    title,
                    short_name,
                    description,
                    priority,
                    severity,
                    tag,
                    assignee,
                    reproduction_steps,
                    affected_component,
                    queue,
                )?;
                output(&result, human);
            }
            BugCommands::List {
                status,
                priority,
                severity,
                tag,
                all,
            } => {
                let result = commands::bug_list(
                    repo_path,
                    status.as_deref(),
                    priority,
                    severity.as_deref(),
                    tag.as_deref(),
                    all,
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
                short_name,
                description,
                priority,
                status,
                severity,
                add_tag,
                remove_tag,
                assignee,
                reproduction_steps,
                affected_component,
                force,
                keep_closed,
                reopen,
            } => {
                let result = commands::bug_update(
                    repo_path,
                    &id,
                    title,
                    short_name,
                    description,
                    priority,
                    status.as_deref(),
                    severity,
                    add_tag,
                    remove_tag,
                    assignee,
                    reproduction_steps,
                    affected_component,
                    force,
                    keep_closed,
                    reopen,
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

        Some(Commands::Idea { command }) => match command {
            IdeaCommands::Create {
                title,
                short_name,
                tag,
                description,
            } => {
                let result = commands::idea_create(repo_path, title, short_name, description, tag)?;
                output(&result, human);
            }
            IdeaCommands::List { status, tag } => {
                let result = commands::idea_list(repo_path, status.as_deref(), tag.as_deref())?;
                output(&result, human);
            }
            IdeaCommands::Show { id } => {
                let result = commands::idea_show(repo_path, &id)?;
                output(&result, human);
            }
            IdeaCommands::Update {
                id,
                title,
                short_name,
                description,
                status,
                add_tag,
                remove_tag,
            } => {
                let result = commands::idea_update(
                    repo_path,
                    &id,
                    title,
                    short_name,
                    description,
                    status.as_deref(),
                    add_tag,
                    remove_tag,
                )?;
                output(&result, human);
            }
            IdeaCommands::Close { id, reason } => {
                let result = commands::idea_close(repo_path, &id, reason)?;
                output(&result, human);
            }
            IdeaCommands::Delete { id } => {
                let result = commands::idea_delete(repo_path, &id)?;
                output(&result, human);
            }
            IdeaCommands::Promote {
                id,
                as_prd,
                priority,
            } => {
                let result = commands::idea_promote(repo_path, &id, as_prd, priority)?;
                output(&result, human);
            }
            IdeaCommands::Germinate { id } => {
                let result = commands::idea_germinate(repo_path, &id)?;
                output(&result, human);
            }
        },

        Some(Commands::Doc { command }) => match command {
            DocCommands::Create {
                entity_ids,
                title,
                doc_type,
                short_name,
                content,
                file,
                stdin,
                short,
                tag,
            } => {
                // Parse doc_type string to DocType enum
                let parsed_doc_type = match doc_type.to_lowercase().as_str() {
                    "prd" => DocType::Prd,
                    "note" => DocType::Note,
                    "handoff" => DocType::Handoff,
                    _ => {
                        return Err(binnacle::Error::InvalidInput(format!(
                            "Invalid doc type '{}'. Must be prd, note, or handoff",
                            doc_type
                        )));
                    }
                };

                // Resolve content from various sources
                let resolved_content = if stdin {
                    use std::io::Read;
                    let mut buffer = String::new();
                    std::io::stdin().read_to_string(&mut buffer)?;
                    Some(buffer)
                } else if let Some(ref path) = file {
                    Some(std::fs::read_to_string(path).map_err(|e| {
                        binnacle::Error::InvalidInput(format!(
                            "Failed to read file {}: {}",
                            path.display(),
                            e
                        ))
                    })?)
                } else {
                    content
                };

                let result = commands::doc_create(
                    repo_path,
                    title,
                    parsed_doc_type,
                    short_name,
                    resolved_content,
                    short,
                    tag,
                    entity_ids,
                )?;
                output(&result, human);
            }
            DocCommands::Show { id, full } => {
                let result = commands::doc_show(repo_path, &id, full)?;
                output(&result, human);
            }
            DocCommands::List {
                tag,
                doc_type,
                edited_by,
                for_entity,
            } => {
                // Parse doc_type string to DocType enum
                let parsed_doc_type = doc_type.as_ref().map(|s| match s.to_lowercase().as_str() {
                    "prd" => DocType::Prd,
                    "note" => DocType::Note,
                    "handoff" => DocType::Handoff,
                    _ => DocType::Prd, // default to Prd for unknown types
                });
                let result = commands::doc_list(
                    repo_path,
                    tag.as_deref(),
                    parsed_doc_type.as_ref(),
                    edited_by.as_deref(),
                    for_entity.as_deref(),
                )?;
                output(&result, human);
            }
            DocCommands::Edit {
                id,
                title,
                short_name,
                description,
                content,
                add_tag,
                remove_tag,
            } => {
                let result = commands::doc_edit(
                    repo_path,
                    &id,
                    title,
                    short_name,
                    description,
                    content,
                    add_tag,
                    remove_tag,
                )?;
                output(&result, human);
            }
            DocCommands::Update {
                id,
                content,
                file,
                stdin,
                title,
                short_name,
                description,
                editor,
                clear_dirty,
            } => {
                // Read content from the appropriate source
                let final_content = if stdin {
                    use std::io::Read;
                    let mut buffer = String::new();
                    std::io::stdin().read_to_string(&mut buffer)?;
                    Some(buffer)
                } else if let Some(path) = file {
                    Some(std::fs::read_to_string(path)?)
                } else {
                    content
                };

                let result = commands::doc_update(
                    repo_path,
                    &id,
                    final_content,
                    title,
                    short_name,
                    description,
                    editor.as_deref(),
                    clear_dirty,
                )?;
                output(&result, human);
            }
            DocCommands::History { id } => {
                let result = commands::doc_history(repo_path, &id)?;
                output(&result, human);
            }
            DocCommands::Attach { doc_id, target_id } => {
                let result = commands::doc_attach(repo_path, &doc_id, &target_id)?;
                output(&result, human);
            }
            DocCommands::Detach { doc_id, target_id } => {
                let result = commands::doc_detach(repo_path, &doc_id, &target_id)?;
                output(&result, human);
            }
            DocCommands::Delete { id } => {
                let result = commands::doc_delete(repo_path, &id)?;
                output(&result, human);
            }
        },

        Some(Commands::Milestone { command }) => match command {
            MilestoneCommands::Create {
                title,
                short_name,
                priority,
                tag,
                assignee,
                description,
                due_date,
            } => {
                let result = commands::milestone_create(
                    repo_path,
                    title,
                    short_name,
                    description,
                    priority,
                    tag,
                    assignee,
                    due_date,
                )?;
                output(&result, human);
            }
            MilestoneCommands::List {
                status,
                priority,
                tag,
            } => {
                let result = commands::milestone_list(
                    repo_path,
                    status.as_deref(),
                    priority,
                    tag.as_deref(),
                )?;
                output(&result, human);
            }
            MilestoneCommands::Show { id } => {
                let result = commands::milestone_show(repo_path, &id)?;
                output(&result, human);
            }
            MilestoneCommands::Update {
                id,
                title,
                short_name,
                description,
                priority,
                status,
                add_tag,
                remove_tag,
                assignee,
                due_date,
            } => {
                let result = commands::milestone_update(
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
                    due_date,
                )?;
                output(&result, human);
            }
            MilestoneCommands::Close { id, reason, force } => {
                let result = commands::milestone_close(repo_path, &id, reason, force)?;
                output(&result, human);
            }
            MilestoneCommands::Reopen { id } => {
                let result = commands::milestone_reopen(repo_path, &id)?;
                output(&result, human);
            }
            MilestoneCommands::Delete { id } => {
                let result = commands::milestone_delete(repo_path, &id)?;
                output(&result, human);
            }
            MilestoneCommands::Progress { id } => {
                let result = commands::milestone_progress(repo_path, &id)?;
                output(&result, human);
            }
        },

        Some(Commands::Mission { command }) => match command {
            MissionCommands::Create {
                title,
                short_name,
                priority,
                tag,
                assignee,
                description,
                due_date,
            } => {
                let result = commands::mission_create(
                    repo_path,
                    title,
                    short_name,
                    description,
                    priority,
                    tag,
                    assignee,
                    due_date,
                )?;
                output(&result, human);
            }
            MissionCommands::List {
                status,
                priority,
                tag,
            } => {
                let result =
                    commands::mission_list(repo_path, status.as_deref(), priority, tag.as_deref())?;
                output(&result, human);
            }
            MissionCommands::Show { id } => {
                let result = commands::mission_show(repo_path, &id)?;
                output(&result, human);
            }
            MissionCommands::Update {
                id,
                title,
                short_name,
                description,
                priority,
                status,
                add_tag,
                remove_tag,
                assignee,
                due_date,
            } => {
                let result = commands::mission_update(
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
                    due_date,
                )?;
                output(&result, human);
            }
            MissionCommands::Close { id, reason, force } => {
                let result = commands::mission_close(repo_path, &id, reason, force)?;
                output(&result, human);
            }
            MissionCommands::Reopen { id } => {
                let result = commands::mission_reopen(repo_path, &id)?;
                output(&result, human);
            }
            MissionCommands::Delete { id } => {
                let result = commands::mission_delete(repo_path, &id)?;
                output(&result, human);
            }
            MissionCommands::Progress { id } => {
                let result = commands::mission_progress(repo_path, &id)?;
                output(&result, human);
            }
        },

        Some(Commands::Queue { command }) => match command {
            QueueCommands::Create { title, description } => {
                let result = commands::queue_create(repo_path, title, description)?;
                output(&result, human);
            }
            QueueCommands::Show => {
                let result = commands::queue_show(repo_path)?;
                output(&result, human);
            }
            QueueCommands::Delete => {
                let result = commands::queue_delete(repo_path)?;
                output(&result, human);
            }
            QueueCommands::Add { item_id } => {
                let result = commands::queue_add(repo_path, &item_id)?;
                output(&result, human);
            }
            QueueCommands::Rm { item_id } => {
                let result = commands::queue_rm(repo_path, &item_id)?;
                output(&result, human);
            }
        },

        Some(Commands::Link { command }) => match command {
            LinkCommands::Add {
                source,
                target,
                edge_type,
                reason,
                pinned,
            } => {
                let result =
                    commands::link_add(repo_path, &source, &target, &edge_type, reason, pinned)?;
                output(&result, human);
            }
            LinkCommands::Rm {
                source,
                target,
                edge_type,
            } => {
                let result = commands::link_rm(repo_path, &source, &target, edge_type.as_deref())?;
                output(&result, human);
            }
            LinkCommands::List { id, all, edge_type } => {
                let result =
                    commands::link_list(repo_path, id.as_deref(), all, edge_type.as_deref())?;
                output(&result, human);
            }
        },
        Some(Commands::Test { command }) => match command {
            TestCommands::Create {
                name,
                cmd,
                dir,
                task,
                bug,
            } => {
                let result = commands::test_create(repo_path, name, cmd, dir, task, bug)?;
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
            TestCommands::LinkBug { test_id, bug_id } => {
                let result = commands::test_link_bug(repo_path, &test_id, &bug_id)?;
                output(&result, human);
            }
            TestCommands::UnlinkBug { test_id, bug_id } => {
                let result = commands::test_unlink_bug(repo_path, &test_id, &bug_id)?;
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
            CommitCommands::Link { sha, entity_id } => {
                let result = commands::commit_link(repo_path, &sha, &entity_id)?;
                output(&result, human);
            }
            CommitCommands::Unlink { sha, entity_id } => {
                let result = commands::commit_unlink(repo_path, &sha, &entity_id)?;
                output(&result, human);
            }
            CommitCommands::List { entity_id } => {
                let result = commands::commit_list(repo_path, &entity_id)?;
                output(&result, human);
            }
        },
        Some(Commands::Ready {
            bugs_only,
            tasks_only,
        }) => {
            let result = commands::ready(repo_path, bugs_only, tasks_only)?;
            output(&result, human);
        }
        Some(Commands::Blocked {
            bugs_only,
            tasks_only,
        }) => {
            let result = commands::blocked(repo_path, bugs_only, tasks_only)?;
            output(&result, human);
        }
        Some(Commands::Doctor {
            migrate_edges,
            clean_unused,
            dry_run,
            fix,
            fix_archives,
        }) => {
            if migrate_edges {
                let result = commands::doctor_migrate_edges(repo_path, clean_unused, dry_run)?;
                output(&result, human);
            } else if fix_archives {
                let result = commands::doctor_fix_archives(repo_path, dry_run)?;
                output(&result, human);
            } else if fix {
                let result = commands::doctor_fix(repo_path)?;
                output(&result, human);
            } else {
                let result = commands::doctor(repo_path)?;
                output(&result, human);
            }
        }
        Some(Commands::Log { command, task_id }) => match command {
            Some(LogCommands::Show {
                task_id: show_task_id,
            }) => {
                let result = commands::log(repo_path, show_task_id.as_deref())?;
                output(&result, human);
            }
            Some(LogCommands::Export {
                format,
                command: cmd_filter,
                user,
                success,
                after,
                before,
                limit,
                output: output_path,
            }) => {
                let result = commands::log_export(
                    repo_path,
                    &format,
                    cmd_filter.as_deref(),
                    user.as_deref(),
                    success,
                    after.as_deref(),
                    before.as_deref(),
                    limit,
                    output_path.as_deref(),
                )?;
                output(&result, human);
            }
            Some(LogCommands::Compact {
                max_entries,
                max_age_days,
                dry_run,
            }) => {
                let result = commands::log_compact(repo_path, max_entries, max_age_days, dry_run)?;
                output(&result, human);
            }
            None => {
                // Support old syntax: `bn log <task_id>` as shorthand for `bn log show <task_id>`
                let result = commands::log(repo_path, task_id.as_deref())?;
                output(&result, human);
            }
        },
        Some(Commands::Sync { remote, push, pull }) => {
            let result = commands::sync(repo_path, remote, push, pull)?;
            output(&result, human);
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
            McpCommands::Serve { cwd } => {
                mcp::serve(repo_path, cwd);
            }
            McpCommands::Manifest => {
                mcp::manifest();
            }
        },
        Some(Commands::Graph { command }) => match command {
            GraphCommands::Components => {
                let result = commands::graph_components(repo_path)?;
                output(&result, human);
            }
            GraphCommands::Lineage { id, depth, verbose } => {
                let result = commands::graph_lineage(repo_path, &id, depth, verbose)?;
                output(&result, human);
            }
            GraphCommands::Peers {
                id,
                depth,
                include_closed,
                verbose,
            } => {
                let result = commands::graph_peers(repo_path, &id, depth, include_closed, verbose)?;
                output(&result, human);
            }
            GraphCommands::Descendants {
                id,
                depth,
                all,
                include_closed,
                verbose,
            } => {
                let result = commands::graph_descendants(
                    repo_path,
                    &id,
                    depth,
                    all,
                    include_closed,
                    verbose,
                )?;
                output(&result, human);
            }
        },
        Some(Commands::Search { command }) => match command {
            SearchCommands::Link {
                edge_type,
                source,
                target,
            } => {
                let result = commands::search_link(
                    repo_path,
                    edge_type.as_deref(),
                    source.as_deref(),
                    target.as_deref(),
                )?;
                output(&result, human);
            }
        },
        Some(Commands::System { command }) => match command {
            SystemCommands::Init {
                write_agents_md,
                write_claude_skills,
                write_codex_skills,
                write_copilot_prompts,
                install_hook,
                write_mcp_vscode,
                write_mcp_copilot,
                write_mcp_all,
                install_copilot,
                install_bn_agent,
                build_container,
                yes,
            } => {
                // Expand --write-mcp-all into individual MCP flags
                let write_mcp_vscode = write_mcp_vscode || write_mcp_all;
                let write_mcp_copilot = write_mcp_copilot || write_mcp_all;

                let result = if yes {
                    // Non-interactive: use flags directly
                    commands::init_non_interactive(
                        repo_path,
                        write_agents_md,
                        write_claude_skills,
                        write_codex_skills,
                        write_copilot_prompts,
                        install_hook,
                        write_mcp_vscode,
                        write_mcp_copilot,
                        install_copilot,
                        install_bn_agent,
                        build_container,
                    )?
                } else if write_agents_md
                    || write_claude_skills
                    || write_codex_skills
                    || write_copilot_prompts
                    || install_hook
                    || write_mcp_vscode
                    || write_mcp_copilot
                    || write_mcp_all
                    || install_copilot
                    || install_bn_agent
                    || build_container
                {
                    // Flags provided without -y: use flags as the options
                    commands::init_non_interactive(
                        repo_path,
                        write_agents_md,
                        write_claude_skills,
                        write_codex_skills,
                        write_copilot_prompts,
                        install_hook,
                        write_mcp_vscode,
                        write_mcp_copilot,
                        install_copilot,
                        install_bn_agent,
                        build_container,
                    )?
                } else {
                    // Interactive mode (default)
                    commands::init(repo_path)?
                };
                output(&result, human);
            }
            SystemCommands::Reinit => {
                let result = commands::init_reinit(repo_path)?;
                output(&result, human);
            }
            SystemCommands::Store { command } => match command {
                StoreCommands::Show => {
                    let result = commands::system_store_show(repo_path)?;
                    output(&result, human);
                }
                StoreCommands::Export {
                    output: out_path,
                    format,
                } => {
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
                    let result =
                        commands::system_store_import(repo_path, &input, &r#type, dry_run)?;
                    output(&result, human);
                }
                StoreCommands::Dump => {
                    let result = commands::system_store_dump(repo_path)?;
                    output(&result, human);
                }
                StoreCommands::Clear { force, no_backup } => {
                    let result = commands::system_store_clear(repo_path, force, no_backup, human)?;
                    output(&result, human);
                }
                StoreCommands::Archive { commit_hash } => {
                    let result = commands::generate_commit_archive(repo_path, &commit_hash)?;
                    output(&result, human);
                }
            },
            SystemCommands::Emit { template } => {
                use binnacle::models::agents;
                // Some templates are static strings, agent files are dynamically generated
                let content: String = match template {
                    EmitTemplate::Agents => commands::AGENTS_MD_BLURB.to_string(),
                    EmitTemplate::Skill => commands::SKILLS_FILE_CONTENT.to_string(),
                    EmitTemplate::PlanAgent => commands::PLAN_AGENT_CONTENT.to_string(),
                    EmitTemplate::PrdAgent => commands::PRD_AGENT_CONTENT.to_string(),
                    EmitTemplate::TasksAgent => commands::TASKS_AGENT_CONTENT.to_string(),
                    EmitTemplate::CopilotInstructions => {
                        commands::COPILOT_INSTRUCTIONS_CONTENT.to_string()
                    }
                    EmitTemplate::AutoWorker => commands::AUTO_WORKER_PROMPT.to_string(),
                    EmitTemplate::DoAgent => commands::DO_AGENT_PROMPT.to_string(),
                    EmitTemplate::PrdWriter => commands::PRD_WRITER_PROMPT.to_string(),
                    EmitTemplate::Buddy => commands::BUDDY_PROMPT.to_string(),
                    EmitTemplate::Free => commands::FREE_PROMPT.to_string(),
                    EmitTemplate::AskAgent => commands::ASK_AGENT_PROMPT.to_string(),
                    EmitTemplate::McpClaude => commands::MCP_CLAUDE_CONFIG.to_string(),
                    EmitTemplate::McpVscode => commands::MCP_VSCODE_CONFIG.to_string(),
                    EmitTemplate::McpCopilot => commands::MCP_COPILOT_CONFIG.to_string(),
                    EmitTemplate::McpLifecycle => commands::MCP_LIFECYCLE_BLURB.to_string(),
                    EmitTemplate::McpLifecyclePlanner => {
                        commands::MCP_LIFECYCLE_BLURB_PLANNER.to_string()
                    }
                    EmitTemplate::BnAgent => commands::BN_AGENT_SCRIPT.to_string(),
                    // Agent files are dynamically generated from prompts + metadata
                    EmitTemplate::AgentAuto => agents::agent_auto(),
                    EmitTemplate::AgentDo => agents::agent_do(),
                    EmitTemplate::AgentPrd => agents::agent_prd(),
                    EmitTemplate::AgentBuddy => agents::agent_buddy(),
                    EmitTemplate::AgentAsk => agents::agent_ask(),
                    EmitTemplate::AgentFree => agents::agent_free(),
                };
                if human {
                    println!("{}", content.trim());
                } else {
                    println!("{}", serde_json::json!({"content": content.trim()}));
                }
            }
            SystemCommands::Migrate { to, dry_run } => {
                let result = commands::migrate_storage(repo_path, &to, dry_run)?;
                output(&result, human);
            }
            SystemCommands::MigrateBugs {
                dry_run,
                remove_tag,
            } => {
                let result = commands::migrate_bugs(repo_path, dry_run, remove_tag)?;
                output(&result, human);
            }
            SystemCommands::BuildInfo => {
                let result = serde_json::json!({
                    "version": binnacle::cli::package_version(),
                    "commit": binnacle::cli::git_commit(),
                    "built": binnacle::cli::build_timestamp(),
                    "copilot_version": binnacle::cli::copilot_version(),
                });
                if human {
                    println!("Version: {}", binnacle::cli::package_version());
                    println!("Commit:  {}", binnacle::cli::git_commit());
                    println!("Built:   {}", binnacle::cli::build_timestamp());
                    println!("Copilot: {}", binnacle::cli::copilot_version());
                } else {
                    println!("{}", result);
                }
            }
            SystemCommands::Hooks { command } => match command {
                HooksCommands::Uninstall => {
                    let result = commands::hooks_uninstall(repo_path)?;
                    output(&result, human);
                }
            },
            SystemCommands::Copilot { command } => match command {
                CopilotCommands::Install { version, upstream } => {
                    let result = commands::copilot_install(version.clone(), upstream)?;
                    output(&result, human);
                }
                CopilotCommands::Path => {
                    let result = commands::copilot_path(repo_path)?;
                    output(&result, human);
                }
                CopilotCommands::Version => {
                    let result = commands::copilot_version_list(repo_path)?;
                    output(&result, human);
                }
            },
            #[cfg(feature = "tmux")]
            SystemCommands::Tmux { command } => {
                // Check tmux binary first
                binnacle::tmux::check_tmux_binary()?;
                match command {
                    TmuxCommands::Save {
                        name,
                        project,
                        user,
                    } => {
                        // Capture current tmux session
                        let layout = binnacle::tmux::save::capture_session()?;

                        // Use provided name or session name
                        let layout_name = name.clone().unwrap_or_else(|| layout.name.clone());

                        // Determine save location
                        let save_path = tmux_save_path(repo_path, &layout_name, project, user)?;

                        // Check for existing file and warn
                        if save_path.exists() {
                            let metadata = std::fs::metadata(&save_path).map_err(|e| {
                                binnacle::Error::Other(format!(
                                    "Failed to read file metadata: {}",
                                    e
                                ))
                            })?;
                            let modified = metadata
                                .modified()
                                .map(|t| {
                                    use std::time::SystemTime;
                                    let duration = t.duration_since(SystemTime::UNIX_EPOCH).ok()?;
                                    chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0)
                                })
                                .ok()
                                .flatten();

                            eprintln!("Warning: Layout file already exists:");
                            eprintln!("  Path: {}", save_path.display());
                            if let Some(mod_time) = modified {
                                eprintln!("  Modified: {}", mod_time.format("%Y-%m-%d %H:%M:%S"));
                            }
                            eprintln!();
                            eprint!("Overwrite? [y/N] ");
                            use std::io::{self, BufRead};
                            let stdin = io::stdin();
                            let mut handle = stdin.lock();
                            let mut response = String::new();
                            handle.read_line(&mut response).map_err(|e| {
                                binnacle::Error::Other(format!("Failed to read input: {}", e))
                            })?;

                            if !response.trim().eq_ignore_ascii_case("y") {
                                eprintln!("Cancelled.");
                                return Ok(());
                            }
                        }

                        // Save layout
                        binnacle::tmux::save::save_layout_to_file(&layout, &save_path)?;

                        let result = commands::TmuxSaveResult {
                            saved: true,
                            path: save_path.display().to_string(),
                            layout_name,
                        };
                        output(&result, human);
                    }
                    TmuxCommands::Load { name } => {
                        use binnacle::tmux::command::TmuxCommand;
                        use binnacle::tmux::layout::{ResolvedLayout, find_layout, load_layout};
                        use std::process::Command;

                        // Find the layout using multi-source resolution
                        let discovered = find_layout(&name, repo_path)?.ok_or_else(|| {
                            binnacle::Error::Other(format!(
                                "Layout '{}' not found. Searched in:\n  - {}\n  - Session storage\n  - User config",
                                name,
                                repo_path.join(".binnacle/tmux").display()
                            ))
                        })?;

                        // Load and parse the layout
                        let layout = load_layout(&discovered)?;
                        let session_name = layout.name.clone();

                        // Resolve the layout with current directory as base
                        let cwd = std::env::current_dir().map_err(|e| {
                            binnacle::Error::Other(format!(
                                "Failed to get current directory: {}",
                                e
                            ))
                        })?;
                        let resolved = ResolvedLayout::from_layout(layout, &cwd)?;

                        // Check if session already exists
                        let has_session_cmd = TmuxCommand::has_session(&session_name);
                        let session_exists = Command::new("tmux")
                            .args(&has_session_cmd.args()[1..]) // skip "tmux"
                            .stdout(std::process::Stdio::null())
                            .stderr(std::process::Stdio::null())
                            .status()
                            .map(|s| s.success())
                            .unwrap_or(false);

                        // Check if we're already inside tmux
                        let in_tmux = std::env::var("TMUX").is_ok();

                        // Compute repo hash for env var
                        let repo_hash = binnacle::storage::compute_repo_hash(repo_path)?;

                        // Check for repo mismatch if session exists
                        let mut warning = None;
                        if session_exists {
                            // Check if session was created for a different repo
                            let show_env_cmd = TmuxCommand::show_environment(
                                Some(&session_name),
                                "BINNACLE_REPO_HASH",
                            );
                            if let Ok(output) = Command::new("tmux")
                                .args(&show_env_cmd.args()[1..])
                                .output()
                            {
                                let stdout = String::from_utf8_lossy(&output.stdout);
                                // Format is "BINNACLE_REPO_HASH=value" or "-BINNACLE_REPO_HASH" if not set
                                if let Some(existing_hash) =
                                    stdout.trim().strip_prefix("BINNACLE_REPO_HASH=")
                                    && existing_hash != repo_hash
                                {
                                    warning = Some(format!(
                                        "Session '{}' was created for a different repository",
                                        session_name
                                    ));
                                }
                            }

                            // Session exists - just attach
                            if !in_tmux {
                                // Not in tmux - attach directly
                                let attach_cmd = TmuxCommand::attach_session(&session_name);
                                let status = Command::new("tmux")
                                    .args(&attach_cmd.args()[1..])
                                    .status()
                                    .map_err(|e| {
                                        binnacle::Error::Other(format!(
                                            "Failed to attach to session: {}",
                                            e
                                        ))
                                    })?;

                                if !status.success() {
                                    return Err(binnacle::Error::Other(
                                        "Failed to attach to tmux session".to_string(),
                                    ));
                                }
                            }

                            let result = commands::TmuxLoadResult {
                                created: false,
                                session_name: session_name.clone(),
                                layout_name: name.clone(),
                                source: discovered.source.to_string(),
                                path: discovered.path.display().to_string(),
                                warning,
                                in_tmux,
                                commands: if in_tmux {
                                    Some(vec![format!("tmux switch-client -t {}", session_name)])
                                } else {
                                    None
                                },
                            };
                            output(&result, human);
                        } else {
                            // Create new session
                            let commands = resolved.to_commands();

                            // Execute all commands
                            for cmd in &commands {
                                let args = cmd.args();
                                let status = Command::new(&args[0])
                                    .args(&args[1..])
                                    .status()
                                    .map_err(|e| {
                                        binnacle::Error::Other(format!(
                                            "Failed to execute '{}': {}",
                                            cmd.clone().build(),
                                            e
                                        ))
                                    })?;

                                if !status.success() {
                                    return Err(binnacle::Error::Other(format!(
                                        "Tmux command failed: {}",
                                        cmd.clone().build()
                                    )));
                                }
                            }

                            // Set BINNACLE_REPO_HASH environment variable
                            let set_env_cmd = TmuxCommand::set_environment(
                                Some(&session_name),
                                "BINNACLE_REPO_HASH",
                                &repo_hash,
                                false,
                            );
                            let _ = Command::new("tmux").args(&set_env_cmd.args()[1..]).status();

                            // Auto-attach if not already in tmux
                            if !in_tmux {
                                let attach_cmd = TmuxCommand::attach_session(&session_name);
                                let status = Command::new("tmux")
                                    .args(&attach_cmd.args()[1..])
                                    .status()
                                    .map_err(|e| {
                                        binnacle::Error::Other(format!(
                                            "Failed to attach to session: {}",
                                            e
                                        ))
                                    })?;

                                if !status.success() {
                                    return Err(binnacle::Error::Other(
                                        "Failed to attach to tmux session".to_string(),
                                    ));
                                }
                            }

                            let result = commands::TmuxLoadResult {
                                created: true,
                                session_name: session_name.clone(),
                                layout_name: name.clone(),
                                source: discovered.source.to_string(),
                                path: discovered.path.display().to_string(),
                                warning: None,
                                in_tmux,
                                commands: if in_tmux {
                                    Some(vec![format!("tmux switch-client -t {}", session_name)])
                                } else {
                                    None
                                },
                            };
                            output(&result, human);
                        }
                    }
                }
            }
        },
        Some(Commands::Agent { command }) => match command {
            AgentCommands::List { status } => {
                let result = commands::agent_list(repo_path, status.as_deref())?;
                output(&result, human);
            }
            AgentCommands::Kill { target, timeout } => {
                let result = commands::agent_kill(repo_path, &target, timeout)?;
                output(&result, human);
            }
            AgentCommands::Scale {
                agent_type,
                min,
                max,
            } => {
                // If no type specified and no settings provided, show all
                // If type specified with no settings, show that type
                // If type specified with settings, update that type
                match (&agent_type, min.is_some() || max.is_some()) {
                    (None, false) => {
                        // Show all types
                        let result = commands::config_get_agent_scaling(repo_path)?;
                        output(&result, human);
                    }
                    (Some(t), false) => {
                        // Show specific type
                        let config = commands::config_get_agent_scaling_for_type(repo_path, t)?;
                        let result = commands::AgentScalingConfigWithType {
                            agent_type: t.clone(),
                            min: config.min,
                            max: config.max,
                        };
                        output(&result, human);
                    }
                    (Some(t), true) => {
                        // Update specific type
                        let config = commands::config_set_agent_scaling(repo_path, t, min, max)?;
                        let result = commands::AgentScalingConfigWithType {
                            agent_type: t.clone(),
                            min: config.min,
                            max: config.max,
                        };
                        output(&result, human);
                    }
                    (None, true) => {
                        // Cannot set values without specifying type
                        return Err(binnacle::Error::Other(
                            "Agent type required when setting min/max values".to_string(),
                        ));
                    }
                }
            }
            AgentCommands::Rm {
                target,
                force,
                all,
                agent_type,
            } => {
                let result = commands::agent_rm(repo_path, target, force, all, agent_type)?;
                output(&result, human);
            }
            AgentCommands::Spawn {
                agent_type,
                name,
                cpus,
                memory,
                worktree,
                merge_target,
                no_merge,
                prompt,
            } => {
                let result = commands::agent_spawn(
                    repo_path,
                    &agent_type,
                    name,
                    cpus,
                    memory.as_deref(),
                    worktree.as_deref(),
                    &merge_target,
                    no_merge,
                    prompt.as_deref(),
                )?;
                output(&result, human);
            }
            AgentCommands::Reconcile { dry_run } => {
                let result = commands::agent_reconcile(repo_path, dry_run)?;
                output(&result, human);
            }
        },
        Some(Commands::Container { command }) => match command {
            ContainerCommands::Build {
                definition,
                all,
                tag,
                no_cache,
                skip_mount_validation,
                project,
                host,
            } => {
                // Convert flags to source preference
                let source_preference = if project {
                    binnacle::container::SourcePreference::Project
                } else if host {
                    binnacle::container::SourcePreference::Host
                } else {
                    binnacle::container::SourcePreference::None
                };

                let result = commands::container_build(
                    repo_path,
                    definition.as_deref(),
                    all,
                    &tag,
                    no_cache,
                    skip_mount_validation,
                    source_preference,
                )?;
                output(&result, human);
            }
            ContainerCommands::Run {
                worktree_path,
                agent_type,
                name,
                merge_target,
                no_merge,
                readonly_workspace,
                cpus,
                memory,
                shell,
                prompt,
                definition,
                project,
                host,
            } => {
                // Convert flags to source preference
                let source_preference = if project {
                    binnacle::container::SourcePreference::Project
                } else if host {
                    binnacle::container::SourcePreference::Host
                } else {
                    binnacle::container::SourcePreference::None
                };

                let result = commands::container_run(
                    repo_path,
                    &worktree_path,
                    &agent_type,
                    name,
                    &merge_target,
                    no_merge,
                    readonly_workspace,
                    cpus,
                    memory.as_deref(),
                    shell,
                    prompt.as_deref(),
                    definition.as_deref(),
                    source_preference,
                )?;
                output(&result, human);
            }
            ContainerCommands::Stop { name, all } => {
                let result = commands::container_stop(name, all)?;
                output(&result, human);
            }
            ContainerCommands::List { all, quiet } => {
                let result = commands::container_list(all, quiet)?;
                output(&result, human);
            }
            ContainerCommands::ListDefinitions => {
                let result = commands::container_list_definitions(repo_path)?;
                output(&result, human);
            }
        },
        Some(Commands::Serve { interval, dry_run }) => {
            commands::serve(repo_path, interval, dry_run, human)?;
        }
        #[cfg(feature = "gui")]
        Some(Commands::Gui {
            command,
            port,
            host,
            readonly,
            dev,
            tunnel,
            archive,
        }) => match command {
            Some(GuiCommands::Serve {
                port: sub_port,
                host: sub_host,
                replace,
                readonly: sub_readonly,
                dev: sub_dev,
                tunnel: sub_tunnel,
            }) => {
                let actual_port = sub_port.or(port);
                let actual_host = &sub_host;
                let actual_readonly = sub_readonly || readonly;
                let actual_dev = sub_dev || dev;
                let actual_tunnel = sub_tunnel || tunnel;
                if replace {
                    replace_gui(
                        repo_path,
                        actual_port,
                        actual_host,
                        actual_readonly,
                        archive.as_deref(),
                        human,
                        actual_dev,
                        actual_tunnel,
                    )?;
                } else {
                    run_gui(
                        repo_path,
                        actual_port,
                        actual_host,
                        actual_readonly,
                        archive.as_deref(),
                        actual_dev,
                        actual_tunnel,
                    )?;
                }
            }
            Some(GuiCommands::Status) => {
                show_gui_status(repo_path, human)?;
            }
            Some(GuiCommands::Stop { force }) => {
                stop_gui(repo_path, force, human)?;
            }
            Some(GuiCommands::Kill { force }) => {
                kill_gui(repo_path, force, human)?;
            }
            Some(GuiCommands::Export { output, archive }) => {
                export_static_gui(repo_path, &output, archive.as_deref(), human)?;
            }
            None => {
                // Default: start server (same as `bn gui serve`)
                run_gui(
                    repo_path,
                    port,
                    &host,
                    readonly,
                    archive.as_deref(),
                    dev,
                    tunnel,
                )?;
            }
        },
        None => {
            // Show welcome message when no command is given
            if human {
                println!("Welcome to Binnacle, run bn --help for more info!");
            } else {
                println!(r#"{{"message":"Welcome to Binnacle, run bn --help for more info!"}}"#);
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
fn run_gui(
    repo_path: &Path,
    port: Option<u16>,
    host: &str,
    readonly: bool,
    archive_path: Option<&str>,
    dev: bool,
    tunnel: bool,
) -> Result<(), binnacle::Error> {
    use binnacle::commands::system_store_import;
    use binnacle::gui::{
        DEFAULT_PORT, GuiPidFile, GuiPidInfo, ProcessStatus, TunnelManager, find_available_port,
    };
    use binnacle::storage::{Storage, get_storage_dir};

    // Handle archive mode: import to temp directory and serve from there
    let (_temp_repo_dir, _temp_data_dir, actual_repo_path) = if let Some(archive) = archive_path {
        // Create temporary directories for repo and data storage
        let temp_repo_dir = tempfile::tempdir().map_err(|e| {
            binnacle::Error::Other(format!("Failed to create temp repo directory: {}", e))
        })?;
        let temp_data_dir = tempfile::tempdir().map_err(|e| {
            binnacle::Error::Other(format!("Failed to create temp data directory: {}", e))
        })?;

        let temp_repo_path = temp_repo_dir.path().to_path_buf();
        let temp_data_path = temp_data_dir.path();

        // Set BN_DATA_DIR to use our temp data directory
        // This will be used by all subsequent storage operations in this process
        // SAFETY: We control the process and this is the only place we set BN_DATA_DIR
        unsafe {
            std::env::set_var("BN_DATA_DIR", temp_data_path);
        }

        // Import archive to temp directory
        system_store_import(&temp_repo_path, archive, "replace", false)
            .map_err(|e| binnacle::Error::Other(format!("Failed to import archive: {}", e)))?;

        // Return temp directories (to keep them alive) and the repo path
        (Some(temp_repo_dir), Some(temp_data_dir), temp_repo_path)
    } else {
        // Ensure storage is initialized for normal mode
        if !Storage::exists(repo_path)? {
            return Err(binnacle::Error::NotInitialized);
        }
        (None, None, repo_path.to_path_buf())
    };

    let storage_dir = get_storage_dir(&actual_repo_path)?;
    let pid_file = GuiPidFile::new(&storage_dir);

    // Check if another GUI is already running
    if let Some((status, info)) = pid_file
        .check_running()
        .map_err(|e| binnacle::Error::Other(format!("Failed to check PID file: {}", e)))?
    {
        match status {
            ProcessStatus::Running => {
                return Err(binnacle::Error::Other(format!(
                    "GUI already running (pid: {}, port: {}). Use --stop to stop it first, or --replace to restart.",
                    info.pid, info.port
                )));
            }
            ProcessStatus::Stale | ProcessStatus::NotRunning => {
                // Clean up stale PID file
                pid_file.delete().ok();
            }
        }
    }

    // Tunnel mode: check devtunnel availability and authentication
    if tunnel {
        if !TunnelManager::check_devtunnel() {
            return Err(binnacle::Error::Other(
                "devtunnel not found in PATH. Install it with: just install-devtunnel".to_string(),
            ));
        }
        if !TunnelManager::check_authenticated() {
            return Err(binnacle::Error::Other(
                "devtunnel not authenticated. Run: devtunnel user login".to_string(),
            ));
        }
    }

    // Determine port: use specified port, or for tunnel mode use default (no auto-port)
    let actual_port = match port {
        Some(p) => p,
        None if tunnel => {
            // Tunnel mode: use default port directly (no auto-port search)
            DEFAULT_PORT
        }
        None => find_available_port(host, DEFAULT_PORT).ok_or_else(|| {
            binnacle::Error::Other(format!(
                "Could not find an available port starting from {}",
                DEFAULT_PORT
            ))
        })?,
    };

    // Write PID file before starting server
    let current_pid = std::process::id();
    let pid_info = GuiPidInfo {
        pid: current_pid,
        port: actual_port,
        host: host.to_string(),
    };
    pid_file
        .write(&pid_info)
        .map_err(|e| binnacle::Error::Other(format!("Failed to write PID file: {}", e)))?;

    // Force readonly mode when serving from archive or via tunnel (security)
    let actual_readonly = readonly || archive_path.is_some() || tunnel;

    // Start tunnel if enabled
    let _tunnel_manager = if tunnel {
        println!("Starting devtunnel...");
        let manager = TunnelManager::start(actual_port, &actual_repo_path)
            .map_err(|e| binnacle::Error::Other(format!("Failed to start tunnel: {}", e)))?;

        if let Some(public_url) = manager.public_url() {
            println!();
            println!("┌─────────────────────────────────────────────────────────────────────┐");
            println!("│  Binnacle GUI is available at:                                      │");
            println!("│                                                                     │");
            println!("│  Local:  http://{}:{:<43} │", host, actual_port);
            println!("│  Public: {:<55} │", public_url);
            println!("│                                                                     │");
            println!("│  Note: Public URL is read-only for security.                        │");
            println!("│  Press Ctrl+C to stop                                               │");
            println!("└─────────────────────────────────────────────────────────────────────┘");
            println!();
        }

        Some(manager)
    } else {
        None
    };

    // Create tokio runtime and run the server
    let result = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| binnacle::Error::Other(format!("Failed to create runtime: {}", e)))?
        .block_on(async {
            binnacle::gui::start_server(&actual_repo_path, actual_port, host, actual_readonly, dev)
                .await
                .map_err(|e| binnacle::Error::Other(format!("GUI server error: {}", e)))
        });

    // Clean up PID file on shutdown (whether success or error)
    pid_file.delete().ok();

    // Note: _tunnel_manager is automatically dropped here, which calls shutdown()
    // Note: temp_dir is automatically cleaned up when it goes out of scope

    result
}

/// Stop any running GUI server and start a new one
#[cfg(feature = "gui")]
#[allow(clippy::too_many_arguments)]
fn replace_gui(
    repo_path: &Path,
    port: Option<u16>,
    host: &str,
    readonly: bool,
    archive_path: Option<&str>,
    human: bool,
    dev: bool,
    tunnel: bool,
) -> Result<(), binnacle::Error> {
    use binnacle::gui::{GuiPidFile, ProcessStatus};
    use binnacle::storage::{Storage, get_storage_dir};
    use std::thread;
    use std::time::Duration;

    // For archive mode, we don't check the original repo_path storage
    if archive_path.is_none() {
        // Ensure storage is initialized for normal mode
        if !Storage::exists(repo_path)? {
            return Err(binnacle::Error::NotInitialized);
        }
    }

    let storage_dir = get_storage_dir(repo_path)?;
    let pid_file = GuiPidFile::new(&storage_dir);

    // Check if a GUI is already running and stop it
    if let Some((status, info)) = pid_file
        .check_running()
        .map_err(|e| binnacle::Error::Other(format!("Failed to check PID file: {}", e)))?
    {
        match status {
            ProcessStatus::Running => {
                let pid = info.pid;
                if human {
                    println!("Stopping existing GUI server (PID: {})...", pid);
                }

                // Send SIGTERM for graceful shutdown
                if send_signal(pid, Signal::Term) {
                    // Wait for graceful shutdown with timeout
                    const GRACEFUL_TIMEOUT: Duration = Duration::from_secs(5);
                    const POLL_INTERVAL: Duration = Duration::from_millis(100);
                    let deadline = std::time::Instant::now() + GRACEFUL_TIMEOUT;

                    loop {
                        thread::sleep(POLL_INTERVAL);

                        match pid_file.check_running() {
                            Ok(Some((ProcessStatus::Running, _))) => {
                                if std::time::Instant::now() >= deadline {
                                    // Timeout - send SIGKILL
                                    if human {
                                        println!(
                                            "Graceful shutdown timed out, forcing termination..."
                                        );
                                    }
                                    send_signal(pid, Signal::Kill);
                                    thread::sleep(Duration::from_millis(500));
                                    break;
                                }
                                // Keep waiting
                            }
                            _ => {
                                // Process stopped
                                if human {
                                    println!("Previous server stopped");
                                }
                                break;
                            }
                        }
                    }
                }

                // Clean up stale PID file
                pid_file.delete().ok();
            }
            ProcessStatus::Stale | ProcessStatus::NotRunning => {
                // Clean up stale PID file
                pid_file.delete().ok();
            }
        }
    }

    // Now delegate to run_gui which handles archive logic
    run_gui(repo_path, port, host, readonly, archive_path, dev, tunnel)
}

/// Show the status of the GUI server
#[cfg(feature = "gui")]
fn show_gui_status(repo_path: &Path, human: bool) -> Result<(), binnacle::Error> {
    use binnacle::gui::{GuiPidFile, ProcessStatus};
    use binnacle::storage::get_storage_dir;

    let storage_dir = get_storage_dir(repo_path)?;
    let pid_file = GuiPidFile::new(&storage_dir);

    match pid_file
        .check_running()
        .map_err(|e| binnacle::Error::Other(format!("Failed to check PID file: {}", e)))?
    {
        Some((status, info)) => {
            let status_str = match status {
                ProcessStatus::Running => "running",
                ProcessStatus::NotRunning => "not_running",
                ProcessStatus::Stale => "stale",
            };

            if human {
                match status {
                    ProcessStatus::Running => {
                        println!("GUI server is running");
                        println!("  PID:  {}", info.pid);
                        println!("  Port: {}", info.port);
                        println!("  Host: {}", info.host);
                        println!("  URL:  http://{}:{}", info.host, info.port);
                    }
                    ProcessStatus::NotRunning => {
                        println!("GUI server is not running (stale PID file found)");
                        println!("  Last PID:  {}", info.pid);
                        println!("  Last Port: {}", info.port);
                    }
                    ProcessStatus::Stale => {
                        println!(
                            "GUI server is not running (PID {} was reused by another process)",
                            info.pid
                        );
                    }
                }
            } else {
                println!(
                    r#"{{"status":"{}","pid":{},"port":{},"host":"{}"}}"#,
                    status_str, info.pid, info.port, info.host
                );
            }
        }
        None => {
            if human {
                println!("GUI server is not running (no PID file)");
            } else {
                println!(r#"{{"status":"not_running"}}"#);
            }
        }
    }

    Ok(())
}

/// Stop a running GUI server with graceful shutdown
#[cfg(feature = "gui")]
fn stop_gui(repo_path: &Path, force: bool, human: bool) -> Result<(), binnacle::Error> {
    use binnacle::gui::{GuiPidFile, ProcessStatus};
    use binnacle::storage::get_storage_dir;
    use std::thread;
    use std::time::Duration;

    let storage_dir = get_storage_dir(repo_path)?;
    let pid_file = GuiPidFile::new(&storage_dir);

    match pid_file
        .check_running()
        .map_err(|e| binnacle::Error::Other(format!("Failed to check PID file: {}", e)))?
    {
        Some((status, info)) => match status {
            ProcessStatus::Running => {
                let pid = info.pid;

                if force {
                    // Force mode: skip SIGTERM, send SIGKILL immediately
                    if human {
                        println!("Force stopping GUI server (PID: {}) with SIGKILL...", pid);
                    }
                    send_signal(pid, Signal::Kill);
                    thread::sleep(Duration::from_millis(500));
                    pid_file.delete().ok();
                    if human {
                        println!("GUI server forcefully terminated");
                    } else {
                        println!(
                            r#"{{"status":"stopped","pid":{},"method":"sigkill","forced":true}}"#,
                            pid
                        );
                    }
                    return Ok(());
                }

                // Graceful mode: SIGTERM first
                if human {
                    println!("Stopping GUI server (PID: {})...", pid);
                }

                // Send SIGTERM for graceful shutdown
                if !send_signal(pid, Signal::Term) {
                    // Process already gone
                    pid_file.delete().ok();
                    if human {
                        println!("Process already stopped");
                    } else {
                        println!(
                            r#"{{"status":"stopped","pid":{},"method":"already_gone"}}"#,
                            pid
                        );
                    }
                    return Ok(());
                }

                // Wait for graceful shutdown with timeout
                const GRACEFUL_TIMEOUT: Duration = Duration::from_secs(5);
                const POLL_INTERVAL: Duration = Duration::from_millis(100);
                let deadline = std::time::Instant::now() + GRACEFUL_TIMEOUT;

                loop {
                    thread::sleep(POLL_INTERVAL);

                    // Check if process is still running
                    match pid_file.check_running() {
                        Ok(Some((ProcessStatus::Running, _))) => {
                            if std::time::Instant::now() >= deadline {
                                // Timeout - send SIGKILL
                                if human {
                                    println!("Graceful shutdown timed out, forcing termination...");
                                }
                                send_signal(pid, Signal::Kill);
                                thread::sleep(Duration::from_millis(500));
                                pid_file.delete().ok();
                                if human {
                                    println!("GUI server forcefully terminated");
                                } else {
                                    println!(
                                        r#"{{"status":"stopped","pid":{},"method":"sigkill"}}"#,
                                        pid
                                    );
                                }
                                return Ok(());
                            }
                            // Keep waiting
                        }
                        _ => {
                            // Process stopped
                            pid_file.delete().ok();
                            if human {
                                println!("GUI server stopped gracefully");
                            } else {
                                println!(
                                    r#"{{"status":"stopped","pid":{},"method":"sigterm"}}"#,
                                    pid
                                );
                            }
                            return Ok(());
                        }
                    }
                }
            }
            ProcessStatus::NotRunning | ProcessStatus::Stale => {
                // Clean up stale PID file
                pid_file.delete().ok();
                if human {
                    println!("GUI server is not running (cleaned up stale PID file)");
                } else {
                    println!(r#"{{"status":"not_running","cleaned_stale":true}}"#);
                }
            }
        },
        None => {
            if human {
                println!("GUI server is not running (no PID file)");
            } else {
                println!(r#"{{"status":"not_running"}}"#);
            }
        }
    }

    Ok(())
}

/// Kill a running GUI server (immediate termination)
#[cfg(feature = "gui")]
fn kill_gui(repo_path: &Path, force: bool, human: bool) -> Result<(), binnacle::Error> {
    use binnacle::gui::{GuiPidFile, ProcessStatus};
    use binnacle::storage::get_storage_dir;
    use std::thread;
    use std::time::Duration;

    let storage_dir = get_storage_dir(repo_path)?;
    let pid_file = GuiPidFile::new(&storage_dir);

    match pid_file
        .check_running()
        .map_err(|e| binnacle::Error::Other(format!("Failed to check PID file: {}", e)))?
    {
        Some((status, info)) => match status {
            ProcessStatus::Running => {
                let pid = info.pid;
                let signal = if force { Signal::Kill } else { Signal::Term };
                let signal_name = if force { "SIGKILL" } else { "SIGTERM" };

                if human {
                    println!("Killing GUI server (PID: {}) with {}...", pid, signal_name);
                }

                if send_signal(pid, signal) {
                    // Wait briefly for process to terminate
                    thread::sleep(Duration::from_millis(500));
                    pid_file.delete().ok();
                    if human {
                        println!("GUI server killed");
                    } else {
                        println!(
                            r#"{{"status":"killed","pid":{},"signal":"{}"}}"#,
                            pid,
                            signal_name.to_lowercase()
                        );
                    }
                } else {
                    pid_file.delete().ok();
                    if human {
                        println!("Process already stopped");
                    } else {
                        println!(
                            r#"{{"status":"killed","pid":{},"method":"already_gone"}}"#,
                            pid
                        );
                    }
                }
            }
            ProcessStatus::NotRunning | ProcessStatus::Stale => {
                pid_file.delete().ok();
                if human {
                    println!("GUI server is not running (cleaned up stale PID file)");
                } else {
                    println!(r#"{{"status":"not_running","cleaned_stale":true}}"#);
                }
            }
        },
        None => {
            if human {
                println!("GUI server is not running (no PID file)");
            } else {
                println!(r#"{{"status":"not_running"}}"#);
            }
        }
    }

    Ok(())
}

/// Signal type for process termination
#[cfg(feature = "gui")]
enum Signal {
    Term,
    Kill,
}

/// Send a signal to a process. Returns false if process doesn't exist.
#[cfg(all(feature = "gui", unix))]
fn send_signal(pid: u32, signal: Signal) -> bool {
    use std::process::Command;

    let signal_str = match signal {
        Signal::Term => "-TERM",
        Signal::Kill => "-KILL",
    };

    Command::new("kill")
        .args([signal_str, &pid.to_string()])
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// Send a signal to a process on Windows. Returns false if process doesn't exist.
#[cfg(all(feature = "gui", windows))]
fn send_signal(pid: u32, signal: Signal) -> bool {
    use std::process::Command;

    // On Windows, we use taskkill. /T kills child processes too.
    // SIGTERM equivalent: taskkill (graceful)
    // SIGKILL equivalent: taskkill /F (force)
    let args = match signal {
        Signal::Term => vec!["/PID", &pid.to_string()],
        Signal::Kill => vec!["/F", "/PID", &pid.to_string()],
    };

    Command::new("taskkill")
        .args(&args)
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// Export static viewer bundle
#[cfg(feature = "gui")]
fn export_static_gui(
    repo_path: &Path,
    output_dir: &str,
    archive_path: Option<&str>,
    human: bool,
) -> Result<(), binnacle::Error> {
    use std::fs;
    use std::path::PathBuf;

    let output = PathBuf::from(output_dir);

    // Create output directory
    fs::create_dir_all(&output)
        .map_err(|e| binnacle::Error::Other(format!("Failed to create output directory: {}", e)))?;

    // Create archive if not provided
    let archive_file = if let Some(path) = archive_path {
        PathBuf::from(path)
    } else {
        let temp_archive = output.join("data.bng");
        binnacle::commands::system_store_export(
            repo_path,
            temp_archive.to_str().unwrap(),
            "archive",
        )?;
        temp_archive
    };

    // Copy archive to output directory
    let dest_archive = output.join("data.bng");
    if archive_file != dest_archive {
        fs::copy(&archive_file, &dest_archive)
            .map_err(|e| binnacle::Error::Other(format!("Failed to copy archive: {}", e)))?;
    }

    // Extract embedded web assets
    use binnacle::gui::embedded;
    let assets = embedded::extract_assets_map()
        .map_err(|e| binnacle::Error::Other(format!("Failed to extract assets: {}", e)))?;

    // Write assets to output directory
    for (path, content) in assets {
        // Skip if path ends with / (directory entry)
        if path.ends_with('/') || path.is_empty() {
            continue;
        }

        let dest_path = output.join(&path);
        if let Some(parent) = dest_path.parent()
            && parent != output
        {
            fs::create_dir_all(parent).map_err(|e| {
                binnacle::Error::Other(format!("Failed to create directory for {}: {}", path, e))
            })?;
        }
        fs::write(&dest_path, content)
            .map_err(|e| binnacle::Error::Other(format!("Failed to write file {}: {}", path, e)))?;
    }

    // Create/update index.html to auto-load the archive
    let index_path = output.join("index.html");
    let mut index_html = fs::read_to_string(&index_path)
        .map_err(|e| binnacle::Error::Other(format!("Failed to read index.html: {}", e)))?;

    // Add script to auto-load archive
    let auto_load_script = r#"
<script>
// Auto-load embedded archive on page load
window.addEventListener('DOMContentLoaded', () => {
    // Trigger archive load via URL parameter
    if (!window.location.search.includes('url=')) {
        const archiveUrl = './data.bng';
        window.location.href = window.location.pathname + '?url=' + encodeURIComponent(archiveUrl);
    }
});
</script>
</head>"#;

    index_html = index_html.replace("</head>", auto_load_script);
    fs::write(&index_path, index_html)
        .map_err(|e| binnacle::Error::Other(format!("Failed to write index.html: {}", e)))?;

    if human {
        println!("Static viewer exported to: {}", output.display());
        println!("  - Archive: data.bng");
        println!(
            "  - Open {} in a browser",
            output.join("index.html").display()
        );
        println!("\nTo host on GitHub Pages:");
        println!("  1. Commit {} to your repo", output_dir);
        println!("  2. Enable Pages in repo settings (source: branch)");
        println!(
            "  3. Visit https://<username>.github.io/<repo>/{}",
            output_dir
        );
    } else {
        println!(
            r#"{{"status":"success","output_dir":"{}","archive":"data.bng"}}"#,
            output.display()
        );
    }

    Ok(())
}

/// Determine save path for tmux layout based on flags.
#[cfg(feature = "tmux")]
fn tmux_save_path(
    repo_path: &Path,
    layout_name: &str,
    project: bool,
    user: bool,
) -> Result<PathBuf, binnacle::Error> {
    let filename = format!("{}.kdl", layout_name);

    if project {
        // Project-level: .binnacle/tmux/ in repo
        Ok(repo_path.join(".binnacle").join("tmux").join(filename))
    } else if user {
        // User-level: ~/.config/binnacle/tmux/
        let config_dir = dirs::config_dir()
            .ok_or_else(|| binnacle::Error::Other("Failed to find config directory".to_string()))?;
        Ok(config_dir.join("binnacle").join("tmux").join(filename))
    } else {
        // Session-level (default): ~/.local/share/binnacle/<hash>/tmux/
        let data_dir = dirs::data_local_dir()
            .ok_or_else(|| binnacle::Error::Other("Failed to find data directory".to_string()))?;
        let repo_hash = binnacle::storage::compute_repo_hash(repo_path)?;
        Ok(data_dir
            .join("binnacle")
            .join(&repo_hash)
            .join("tmux")
            .join(filename))
    }
}

/// Serialize command to extract name and arguments for logging.
fn serialize_command(command: &Option<Commands>) -> (String, serde_json::Value) {
    match command {
        Some(Commands::Orient {
            agent_type,
            init,
            name,
            register,
            dry_run,
        }) => (
            "orient".to_string(),
            serde_json::json!({ "agent_type": agent_type, "init": init, "name": name, "register": register, "dry_run": dry_run }),
        ),

        Some(Commands::Goodbye {
            reason,
            dry_run,
            force,
        }) => (
            "goodbye".to_string(),
            serde_json::json!({ "reason": reason, "dry_run": dry_run, "force": force }),
        ),

        Some(Commands::Show { id }) => ("show".to_string(), serde_json::json!({ "id": id })),

        Some(Commands::Task { command }) => match command {
            TaskCommands::Create {
                title,
                short_name,
                priority,
                tag,
                assignee,
                description,
                queue,
                check_complexity,
                force,
            } => (
                "task create".to_string(),
                serde_json::json!({
                    "title": title,
                    "short_name": short_name,
                    "priority": priority,
                    "tag": tag,
                    "assignee": assignee,
                    "description": description,
                    "queue": queue,
                    "check_complexity": check_complexity,
                    "force": force,
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
            TaskCommands::Show { id } => ("task show".to_string(), serde_json::json!({ "id": id })),
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
                force,
                keep_closed,
                reopen,
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
                    "force": force,
                    "keep_closed": keep_closed,
                    "reopen": reopen,
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
            TaskCommands::Reopen { id } => {
                ("task reopen".to_string(), serde_json::json!({ "id": id }))
            }
            TaskCommands::Delete { id } => {
                ("task delete".to_string(), serde_json::json!({ "id": id }))
            }
        },

        Some(Commands::Bug { command }) => match command {
            BugCommands::Create {
                title,
                short_name,
                priority,
                severity,
                tag,
                assignee,
                description,
                reproduction_steps,
                affected_component,
                queue,
            } => (
                "bug create".to_string(),
                serde_json::json!({
                    "title": title,
                    "short_name": short_name,
                    "priority": priority,
                    "severity": severity,
                    "tag": tag,
                    "assignee": assignee,
                    "description": description,
                    "reproduction_steps": reproduction_steps,
                    "affected_component": affected_component,
                    "queue": queue,
                }),
            ),
            BugCommands::List {
                status,
                priority,
                severity,
                tag,
                all,
            } => (
                "bug list".to_string(),
                serde_json::json!({
                    "status": status,
                    "priority": priority,
                    "severity": severity,
                    "tag": tag,
                    "all": all,
                }),
            ),
            BugCommands::Show { id } => ("bug show".to_string(), serde_json::json!({ "id": id })),
            BugCommands::Update {
                id,
                title,
                short_name,
                description,
                priority,
                status,
                severity,
                add_tag,
                remove_tag,
                assignee,
                reproduction_steps,
                affected_component,
                force,
                keep_closed,
                reopen,
            } => (
                "bug update".to_string(),
                serde_json::json!({
                    "id": id,
                    "title": title,
                    "short_name": short_name,
                    "description": description,
                    "priority": priority,
                    "status": status,
                    "severity": severity,
                    "add_tag": add_tag,
                    "remove_tag": remove_tag,
                    "assignee": assignee,
                    "reproduction_steps": reproduction_steps,
                    "affected_component": affected_component,
                    "force": force,
                    "keep_closed": keep_closed,
                    "reopen": reopen,
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
            BugCommands::Reopen { id } => {
                ("bug reopen".to_string(), serde_json::json!({ "id": id }))
            }
            BugCommands::Delete { id } => {
                ("bug delete".to_string(), serde_json::json!({ "id": id }))
            }
        },

        Some(Commands::Idea { command }) => match command {
            IdeaCommands::Create {
                title,
                short_name,
                tag,
                description,
            } => (
                "idea create".to_string(),
                serde_json::json!({
                    "title": title,
                    "short_name": short_name,
                    "tag": tag,
                    "description": description,
                }),
            ),
            IdeaCommands::List { status, tag } => (
                "idea list".to_string(),
                serde_json::json!({
                    "status": status,
                    "tag": tag,
                }),
            ),
            IdeaCommands::Show { id } => ("idea show".to_string(), serde_json::json!({ "id": id })),
            IdeaCommands::Update {
                id,
                title,
                short_name,
                description,
                status,
                add_tag,
                remove_tag,
            } => (
                "idea update".to_string(),
                serde_json::json!({
                    "id": id,
                    "title": title,
                    "short_name": short_name,
                    "description": description,
                    "status": status,
                    "add_tag": add_tag,
                    "remove_tag": remove_tag,
                }),
            ),
            IdeaCommands::Close { id, reason } => (
                "idea close".to_string(),
                serde_json::json!({
                    "id": id,
                    "reason": reason,
                }),
            ),
            IdeaCommands::Delete { id } => {
                ("idea delete".to_string(), serde_json::json!({ "id": id }))
            }
            IdeaCommands::Promote {
                id,
                as_prd,
                priority,
            } => (
                "idea promote".to_string(),
                serde_json::json!({
                    "id": id,
                    "as_prd": as_prd,
                    "priority": priority,
                }),
            ),
            IdeaCommands::Germinate { id } => (
                "idea germinate".to_string(),
                serde_json::json!({ "id": id }),
            ),
        },

        Some(Commands::Doc { command }) => match command {
            DocCommands::Create {
                entity_ids,
                title,
                doc_type,
                short_name,
                content,
                file,
                stdin,
                short,
                tag,
            } => (
                "doc create".to_string(),
                serde_json::json!({
                    "entity_ids": entity_ids,
                    "title": title,
                    "doc_type": doc_type,
                    "short_name": short_name,
                    "content": content,
                    "file": file,
                    "stdin": stdin,
                    "short": short,
                    "tag": tag,
                }),
            ),
            DocCommands::Show { id, full } => (
                "doc show".to_string(),
                serde_json::json!({ "id": id, "full": full }),
            ),
            DocCommands::List {
                tag,
                doc_type,
                edited_by,
                for_entity,
            } => (
                "doc list".to_string(),
                serde_json::json!({
                    "tag": tag,
                    "doc_type": doc_type,
                    "edited_by": edited_by,
                    "for_entity": for_entity
                }),
            ),
            DocCommands::Edit {
                id,
                title,
                short_name,
                description,
                content,
                add_tag,
                remove_tag,
            } => (
                "doc edit".to_string(),
                serde_json::json!({
                    "id": id,
                    "title": title,
                    "short_name": short_name,
                    "description": description,
                    "content": content,
                    "add_tag": add_tag,
                    "remove_tag": remove_tag,
                }),
            ),
            DocCommands::Update {
                id,
                content,
                file,
                stdin,
                title,
                short_name,
                description,
                editor,
                clear_dirty,
            } => (
                "doc update".to_string(),
                serde_json::json!({
                    "id": id,
                    "content": content,
                    "file": file,
                    "stdin": stdin,
                    "title": title,
                    "short_name": short_name,
                    "description": description,
                    "editor": editor,
                    "clear_dirty": clear_dirty,
                }),
            ),
            DocCommands::History { id } => {
                ("doc history".to_string(), serde_json::json!({ "id": id }))
            }
            DocCommands::Attach { doc_id, target_id } => (
                "doc attach".to_string(),
                serde_json::json!({
                    "doc_id": doc_id,
                    "target_id": target_id,
                }),
            ),
            DocCommands::Detach { doc_id, target_id } => (
                "doc detach".to_string(),
                serde_json::json!({
                    "doc_id": doc_id,
                    "target_id": target_id,
                }),
            ),
            DocCommands::Delete { id } => {
                ("doc delete".to_string(), serde_json::json!({ "id": id }))
            }
        },

        Some(Commands::Milestone { command }) => match command {
            MilestoneCommands::Create {
                title,
                short_name,
                priority,
                tag,
                assignee,
                description,
                due_date,
            } => (
                "milestone create".to_string(),
                serde_json::json!({
                    "title": title,
                    "short_name": short_name,
                    "priority": priority,
                    "tag": tag,
                    "assignee": assignee,
                    "description": description,
                    "due_date": due_date,
                }),
            ),
            MilestoneCommands::List {
                status,
                priority,
                tag,
            } => (
                "milestone list".to_string(),
                serde_json::json!({
                    "status": status,
                    "priority": priority,
                    "tag": tag,
                }),
            ),
            MilestoneCommands::Show { id } => (
                "milestone show".to_string(),
                serde_json::json!({ "id": id }),
            ),
            MilestoneCommands::Update {
                id,
                title,
                short_name,
                description,
                priority,
                status,
                add_tag,
                remove_tag,
                assignee,
                due_date,
            } => (
                "milestone update".to_string(),
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
                    "due_date": due_date,
                }),
            ),
            MilestoneCommands::Close { id, reason, force } => (
                "milestone close".to_string(),
                serde_json::json!({
                    "id": id,
                    "reason": reason,
                    "force": force,
                }),
            ),
            MilestoneCommands::Reopen { id } => (
                "milestone reopen".to_string(),
                serde_json::json!({ "id": id }),
            ),
            MilestoneCommands::Delete { id } => (
                "milestone delete".to_string(),
                serde_json::json!({ "id": id }),
            ),
            MilestoneCommands::Progress { id } => (
                "milestone progress".to_string(),
                serde_json::json!({ "id": id }),
            ),
        },

        Some(Commands::Mission { command }) => match command {
            MissionCommands::Create {
                title,
                short_name,
                priority,
                tag,
                assignee,
                description,
                due_date,
            } => (
                "mission create".to_string(),
                serde_json::json!({
                    "title": title,
                    "short_name": short_name,
                    "priority": priority,
                    "tag": tag,
                    "assignee": assignee,
                    "description": description,
                    "due_date": due_date,
                }),
            ),
            MissionCommands::List {
                status,
                priority,
                tag,
            } => (
                "mission list".to_string(),
                serde_json::json!({
                    "status": status,
                    "priority": priority,
                    "tag": tag,
                }),
            ),
            MissionCommands::Show { id } => {
                ("mission show".to_string(), serde_json::json!({ "id": id }))
            }
            MissionCommands::Update {
                id,
                title,
                short_name,
                description,
                priority,
                status,
                add_tag,
                remove_tag,
                assignee,
                due_date,
            } => (
                "mission update".to_string(),
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
                    "due_date": due_date,
                }),
            ),
            MissionCommands::Close { id, reason, force } => (
                "mission close".to_string(),
                serde_json::json!({
                    "id": id,
                    "reason": reason,
                    "force": force,
                }),
            ),
            MissionCommands::Reopen { id } => (
                "mission reopen".to_string(),
                serde_json::json!({ "id": id }),
            ),
            MissionCommands::Delete { id } => (
                "mission delete".to_string(),
                serde_json::json!({ "id": id }),
            ),
            MissionCommands::Progress { id } => (
                "mission progress".to_string(),
                serde_json::json!({ "id": id }),
            ),
        },

        Some(Commands::Queue { command }) => match command {
            QueueCommands::Create { title, description } => (
                "queue create".to_string(),
                serde_json::json!({
                    "title": title,
                    "description": description,
                }),
            ),
            QueueCommands::Show => ("queue show".to_string(), serde_json::json!({})),
            QueueCommands::Delete => ("queue delete".to_string(), serde_json::json!({})),
            QueueCommands::Add { item_id } => (
                "queue add".to_string(),
                serde_json::json!({ "item_id": item_id }),
            ),
            QueueCommands::Rm { item_id } => (
                "queue rm".to_string(),
                serde_json::json!({ "item_id": item_id }),
            ),
        },

        Some(Commands::Link { command }) => match command {
            LinkCommands::Add {
                source,
                target,
                edge_type,
                reason,
                pinned,
            } => (
                "link add".to_string(),
                serde_json::json!({
                    "source": source,
                    "target": target,
                    "edge_type": edge_type,
                    "reason": reason,
                    "pinned": pinned,
                }),
            ),
            LinkCommands::Rm {
                source,
                target,
                edge_type,
            } => (
                "link rm".to_string(),
                serde_json::json!({
                    "source": source,
                    "target": target,
                    "edge_type": edge_type,
                }),
            ),
            LinkCommands::List { id, all, edge_type } => (
                "link list".to_string(),
                serde_json::json!({
                    "id": id,
                    "all": all,
                    "edge_type": edge_type,
                }),
            ),
        },

        Some(Commands::Test { command }) => match command {
            TestCommands::Create {
                name,
                cmd,
                dir,
                task,
                bug,
            } => (
                "test create".to_string(),
                serde_json::json!({
                    "name": name,
                    "cmd": cmd,
                    "dir": dir,
                    "task": task,
                    "bug": bug,
                }),
            ),
            TestCommands::List { task } => {
                ("test list".to_string(), serde_json::json!({ "task": task }))
            }
            TestCommands::Show { id } => ("test show".to_string(), serde_json::json!({ "id": id })),
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
            TestCommands::LinkBug { test_id, bug_id } => (
                "test link-bug".to_string(),
                serde_json::json!({
                    "test_id": test_id,
                    "bug_id": bug_id,
                }),
            ),
            TestCommands::UnlinkBug { test_id, bug_id } => (
                "test unlink-bug".to_string(),
                serde_json::json!({
                    "test_id": test_id,
                    "bug_id": bug_id,
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
            CommitCommands::Link { sha, entity_id } => (
                "commit link".to_string(),
                serde_json::json!({
                    "sha": sha,
                    "entity_id": entity_id,
                }),
            ),
            CommitCommands::Unlink { sha, entity_id } => (
                "commit unlink".to_string(),
                serde_json::json!({
                    "sha": sha,
                    "entity_id": entity_id,
                }),
            ),
            CommitCommands::List { entity_id } => (
                "commit list".to_string(),
                serde_json::json!({ "entity_id": entity_id }),
            ),
        },

        Some(Commands::Ready {
            bugs_only,
            tasks_only,
        }) => (
            "ready".to_string(),
            serde_json::json!({
                "bugs_only": bugs_only,
                "tasks_only": tasks_only,
            }),
        ),

        Some(Commands::Blocked {
            bugs_only,
            tasks_only,
        }) => (
            "blocked".to_string(),
            serde_json::json!({
                "bugs_only": bugs_only,
                "tasks_only": tasks_only,
            }),
        ),

        Some(Commands::Doctor {
            migrate_edges,
            clean_unused,
            dry_run,
            fix,
            fix_archives,
        }) => (
            "doctor".to_string(),
            serde_json::json!({
                "migrate_edges": migrate_edges,
                "clean_unused": clean_unused,
                "dry_run": dry_run,
                "fix": fix,
                "fix_archives": fix_archives
            }),
        ),

        Some(Commands::Log { command, task_id }) => match command {
            Some(LogCommands::Show {
                task_id: show_task_id,
            }) => (
                "log show".to_string(),
                serde_json::json!({ "task_id": show_task_id }),
            ),
            Some(LogCommands::Export {
                format,
                command: cmd_filter,
                user,
                success,
                after,
                before,
                limit,
                output,
            }) => (
                "log export".to_string(),
                serde_json::json!({
                    "format": format,
                    "command": cmd_filter,
                    "user": user,
                    "success": success,
                    "after": after,
                    "before": before,
                    "limit": limit,
                    "output": output,
                }),
            ),
            Some(LogCommands::Compact {
                max_entries,
                max_age_days,
                dry_run,
            }) => (
                "log compact".to_string(),
                serde_json::json!({
                    "max_entries": max_entries,
                    "max_age_days": max_age_days,
                    "dry_run": dry_run,
                }),
            ),
            None => ("log".to_string(), serde_json::json!({ "task_id": task_id })),
        },

        Some(Commands::Sync { remote, push, pull }) => (
            "sync".to_string(),
            serde_json::json!({
                "remote": remote,
                "push_only": push,
                "pull_only": pull
            }),
        ),

        Some(Commands::Config { command }) => match command {
            ConfigCommands::Get { key } => {
                ("config get".to_string(), serde_json::json!({ "key": key }))
            }
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
            McpCommands::Serve { cwd } => {
                ("mcp serve".to_string(), serde_json::json!({ "cwd": cwd }))
            }
            McpCommands::Manifest => ("mcp manifest".to_string(), serde_json::json!({})),
        },

        Some(Commands::Graph { command }) => match command {
            GraphCommands::Components => ("graph components".to_string(), serde_json::json!({})),
            GraphCommands::Lineage { id, depth, verbose } => (
                "graph lineage".to_string(),
                serde_json::json!({
                    "id": id,
                    "depth": depth,
                    "verbose": verbose,
                }),
            ),
            GraphCommands::Peers {
                id,
                depth,
                include_closed,
                verbose,
            } => (
                "graph peers".to_string(),
                serde_json::json!({
                    "id": id,
                    "depth": depth,
                    "include_closed": include_closed,
                    "verbose": verbose,
                }),
            ),
            GraphCommands::Descendants {
                id,
                depth,
                all,
                include_closed,
                verbose,
            } => (
                "graph descendants".to_string(),
                serde_json::json!({
                    "id": id,
                    "depth": depth,
                    "all": all,
                    "include_closed": include_closed,
                    "verbose": verbose,
                }),
            ),
        },

        Some(Commands::Search { command }) => match command {
            SearchCommands::Link {
                edge_type,
                source,
                target,
            } => (
                "search link".to_string(),
                serde_json::json!({
                    "edge_type": edge_type,
                    "source": source,
                    "target": target,
                }),
            ),
        },

        Some(Commands::System { command }) => match command {
            SystemCommands::Init {
                write_agents_md,
                write_claude_skills,
                write_codex_skills,
                write_copilot_prompts,
                install_hook,
                write_mcp_vscode,
                write_mcp_copilot,
                write_mcp_all,
                install_copilot,
                install_bn_agent,
                build_container,
                yes,
            } => (
                "system init".to_string(),
                serde_json::json!({
                    "write_agents_md": write_agents_md,
                    "write_claude_skills": write_claude_skills,
                    "write_codex_skills": write_codex_skills,
                    "write_copilot_prompts": write_copilot_prompts,
                    "install_hook": install_hook,
                    "write_mcp_vscode": write_mcp_vscode,
                    "write_mcp_copilot": write_mcp_copilot,
                    "write_mcp_all": write_mcp_all,
                    "install_copilot": install_copilot,
                    "install_bn_agent": install_bn_agent,
                    "build_container": build_container,
                    "yes": yes,
                }),
            ),
            SystemCommands::Reinit => ("system reinit".to_string(), serde_json::json!({})),
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
                StoreCommands::Dump => ("system store dump".to_string(), serde_json::json!({})),
                StoreCommands::Clear { force, no_backup } => (
                    "system store clear".to_string(),
                    serde_json::json!({
                        "force": force,
                        "no_backup": no_backup,
                    }),
                ),
                StoreCommands::Archive { commit_hash } => (
                    "system store archive".to_string(),
                    serde_json::json!({
                        "commit_hash": commit_hash,
                    }),
                ),
            },
            SystemCommands::Emit { template } => {
                let template_name = match template {
                    EmitTemplate::Agents => "agents",
                    EmitTemplate::Skill => "skill",
                    EmitTemplate::PlanAgent => "plan-agent",
                    EmitTemplate::PrdAgent => "prd-agent",
                    EmitTemplate::TasksAgent => "tasks-agent",
                    EmitTemplate::CopilotInstructions => "copilot-instructions",
                    EmitTemplate::AutoWorker => "auto-worker",
                    EmitTemplate::DoAgent => "do-agent",
                    EmitTemplate::PrdWriter => "prd-writer",
                    EmitTemplate::Buddy => "buddy",
                    EmitTemplate::Free => "free",
                    EmitTemplate::AskAgent => "ask-agent",
                    EmitTemplate::McpClaude => "mcp-claude",
                    EmitTemplate::McpVscode => "mcp-vscode",
                    EmitTemplate::McpCopilot => "mcp-copilot",
                    EmitTemplate::McpLifecycle => "mcp-lifecycle",
                    EmitTemplate::McpLifecyclePlanner => "mcp-lifecycle-planner",
                    EmitTemplate::BnAgent => "bn-agent",
                    EmitTemplate::AgentAuto => "agent-auto",
                    EmitTemplate::AgentDo => "agent-do",
                    EmitTemplate::AgentPrd => "agent-prd",
                    EmitTemplate::AgentBuddy => "agent-buddy",
                    EmitTemplate::AgentAsk => "agent-ask",
                    EmitTemplate::AgentFree => "agent-free",
                };
                (
                    "system emit".to_string(),
                    serde_json::json!({ "template": template_name }),
                )
            }
            SystemCommands::Migrate { to, dry_run } => (
                "system migrate".to_string(),
                serde_json::json!({ "to": to, "dry_run": dry_run }),
            ),
            SystemCommands::MigrateBugs {
                dry_run,
                remove_tag,
            } => (
                "system migrate-bugs".to_string(),
                serde_json::json!({ "dry_run": dry_run, "remove_tag": remove_tag }),
            ),
            SystemCommands::BuildInfo => ("system build-info".to_string(), serde_json::json!({})),
            SystemCommands::Hooks { command } => match command {
                HooksCommands::Uninstall => {
                    ("system hooks uninstall".to_string(), serde_json::json!({}))
                }
            },
            SystemCommands::Copilot { command } => match command {
                CopilotCommands::Install { version, upstream } => (
                    "system copilot install".to_string(),
                    serde_json::json!({ "version": version, "upstream": upstream }),
                ),
                CopilotCommands::Path => ("system copilot path".to_string(), serde_json::json!({})),
                CopilotCommands::Version => {
                    ("system copilot version".to_string(), serde_json::json!({}))
                }
            },
            #[cfg(feature = "tmux")]
            SystemCommands::Tmux { command } => match command {
                TmuxCommands::Save {
                    name,
                    project,
                    user,
                } => (
                    "system tmux save".to_string(),
                    serde_json::json!({ "name": name, "project": project, "user": user }),
                ),
                TmuxCommands::Load { name } => (
                    "system tmux load".to_string(),
                    serde_json::json!({ "name": name }),
                ),
            },
        },

        Some(Commands::Agent { command }) => match command {
            AgentCommands::List { status } => (
                "agent list".to_string(),
                serde_json::json!({ "status": status }),
            ),
            AgentCommands::Kill { target, timeout } => (
                "agent kill".to_string(),
                serde_json::json!({ "target": target, "timeout": timeout }),
            ),
            AgentCommands::Scale {
                agent_type,
                min,
                max,
            } => (
                "agent scale".to_string(),
                serde_json::json!({ "agent_type": agent_type, "min": min, "max": max }),
            ),
            AgentCommands::Rm {
                target,
                force,
                all,
                agent_type,
            } => (
                "agent rm".to_string(),
                serde_json::json!({ "target": target, "force": force, "all": all, "agent_type": agent_type }),
            ),
            AgentCommands::Spawn {
                agent_type,
                name,
                cpus,
                memory,
                worktree,
                merge_target,
                no_merge,
                prompt,
            } => (
                "agent spawn".to_string(),
                serde_json::json!({
                    "agent_type": agent_type,
                    "name": name,
                    "cpus": cpus,
                    "memory": memory,
                    "worktree": worktree,
                    "merge_target": merge_target,
                    "no_merge": no_merge,
                    "prompt": prompt.is_some()
                }),
            ),
            AgentCommands::Reconcile { dry_run } => (
                "agent reconcile".to_string(),
                serde_json::json!({ "dry_run": dry_run }),
            ),
        },

        Some(Commands::Container { command }) => match command {
            ContainerCommands::Build {
                definition,
                all,
                tag,
                no_cache,
                skip_mount_validation,
                project,
                host,
            } => (
                "container build".to_string(),
                serde_json::json!({
                    "definition": definition,
                    "all": all,
                    "tag": tag,
                    "no_cache": no_cache,
                    "skip_mount_validation": skip_mount_validation,
                    "project": project,
                    "host": host
                }),
            ),
            ContainerCommands::Run {
                worktree_path,
                agent_type,
                name,
                merge_target,
                no_merge,
                readonly_workspace,
                cpus,
                memory,
                shell,
                prompt,
                definition,
                project,
                host,
            } => (
                "container run".to_string(),
                serde_json::json!({
                    "worktree_path": worktree_path,
                    "agent_type": agent_type,
                    "name": name,
                    "merge_target": merge_target,
                    "no_merge": no_merge,
                    "readonly_workspace": readonly_workspace,
                    "cpus": cpus,
                    "memory": memory,
                    "shell": shell,
                    "prompt": prompt.is_some(),
                    "definition": definition,
                    "project": project,
                    "host": host
                }),
            ),
            ContainerCommands::Stop { name, all } => (
                "container stop".to_string(),
                serde_json::json!({ "name": name, "all": all }),
            ),
            ContainerCommands::List { all, quiet } => (
                "container list".to_string(),
                serde_json::json!({ "all": all, "quiet": quiet }),
            ),
            ContainerCommands::ListDefinitions => (
                "container list-definitions".to_string(),
                serde_json::json!({}),
            ),
        },

        Some(Commands::Serve { interval, dry_run }) => (
            "serve".to_string(),
            serde_json::json!({ "interval": interval, "dry_run": dry_run }),
        ),

        #[cfg(feature = "gui")]
        Some(Commands::Gui {
            command,
            port,
            host,
            readonly,
            dev,
            tunnel,
            archive,
        }) => {
            let subcommand = match command {
                Some(GuiCommands::Serve {
                    port: sub_port,
                    host: sub_host,
                    replace,
                    readonly: sub_readonly,
                    dev: sub_dev,
                    tunnel: sub_tunnel,
                }) => {
                    serde_json::json!({ "subcommand": "serve", "port": sub_port.or(*port), "host": sub_host, "replace": replace, "readonly": *sub_readonly || *readonly, "dev": *sub_dev || *dev, "tunnel": *sub_tunnel || *tunnel, "archive": archive })
                }
                Some(GuiCommands::Status) => serde_json::json!({ "subcommand": "status" }),
                Some(GuiCommands::Stop { force }) => {
                    serde_json::json!({ "subcommand": "stop", "force": force })
                }
                Some(GuiCommands::Kill { force }) => {
                    serde_json::json!({ "subcommand": "kill", "force": force })
                }
                Some(GuiCommands::Export { output, archive }) => {
                    serde_json::json!({ "subcommand": "export", "output": output, "archive": archive })
                }
                None => {
                    serde_json::json!({ "subcommand": null, "port": port, "host": host, "readonly": readonly, "dev": dev, "tunnel": tunnel, "archive": archive })
                }
            };
            ("gui".to_string(), subcommand)
        }

        None => ("status".to_string(), serde_json::json!({})),
    }
}
