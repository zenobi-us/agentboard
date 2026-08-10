//! CLI composition root and command dispatch.
//!
//! Built-ins are registered here once so command handlers cannot drift into
//! separate registration sets for loading, diagnostics, and schema generation.

use agentboard_action_run_cmd::RunCmdDefinition;
use agentboard_action_worktree::WorktreeDefinition;
use agentboard_core::{model::Workspace, registry::Registry, CancellationToken};
use agentboard_source_github::GithubSourceDefinition;
use agentboard_source_jira::JiraSourceDefinition;
use agentboard_source_qmd::QmdSourceDefinition;
use std::{env, io::IsTerminal, path::PathBuf, process::Command as ProcessCommand, sync::Arc};

use anyhow::{bail, Context, Result};
use clap::{ArgAction, Parser, Subcommand};

use crate::{
    config::{
        event_log_path, init_workspace, list_workspaces, load_workspace, named_workspace_path,
        validate_workspace_name,
    },
    dashboard::{dashboard, require_dashboard_terminals},
    output::{ColorChoice, Output, Verbosity},
    runtime::{is_cancelled, parse_duration, run_once, run_watch, InvocationCancelled},
    schema::workspace_schema,
    store::{
        doctor, list_items, list_items_watch, require_watch_stdout, show_item, show_item_watch,
        watch_stdout_is_terminal,
    },
};

#[derive(Debug, Parser)]
#[command(name = "agentboard")]
#[command(about = "Collect task-tracking items into local agent work queues")]
/// Parsed `agentboard` command-line interface.
pub struct Cli {
    /// Show per-Item and per-Action progress.
    #[arg(short = 'v', long, global = true, action = ArgAction::Count, conflicts_with = "quiet")]
    verbose: u8,
    /// Suppress non-error progress.
    #[arg(short = 'q', long, global = true, conflicts_with = "verbose")]
    quiet: bool,
    /// Control colour in human-readable stderr output.
    #[arg(long, global = true, value_enum, default_value_t)]
    color: ColorChoice,
    /// Append structured metadata events to a JSONL file.
    #[arg(long, global = true)]
    log_file: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Manage named Workspaces.
    Workspace {
        #[command(subcommand)]
        command: WorkspaceCommand,
    },
    /// List available named Workspaces (compatibility alias).
    #[command(hide = true)]
    Workspaces,
    /// Execute one workspace run.
    Run {
        workspace: Option<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        watch: bool,
        #[arg(long, requires = "watch")]
        interval: Option<String>,
    },
    /// List latest stored items.
    List {
        workspace: Option<String>,
        #[arg(long, conflicts_with = "watch")]
        json: bool,
        #[arg(long)]
        watch: bool,
        #[arg(long, requires = "watch")]
        interval: Option<String>,
    },
    /// Open a read-only Store dashboard.
    Dashboard { workspace: Option<String> },
    /// Show one latest stored item and action attempts.
    Show {
        /// Pass ITEM_ID alone, or WORKSPACE followed by ITEM_ID.
        #[arg(value_name = "ITEM_OR_WORKSPACE", num_args = 1..=2, required = true)]
        workspace_and_item: Vec<String>,
        #[arg(long, conflicts_with = "watch")]
        json: bool,
        #[arg(long)]
        watch: bool,
        #[arg(long, requires = "watch")]
        interval: Option<String>,
    },
    /// Validate workspace and local environment.
    Doctor { workspace: Option<String> },
    /// Print workspace JSON Schema.
    Schema,
}

#[derive(Debug, Subcommand)]
enum WorkspaceCommand {
    /// List available named Workspaces.
    List,
    /// Create an empty named Workspace.
    Init { name: String },
    /// Open an existing named Workspace in $EDITOR.
    Edit { name: String },
}

fn edit_workspace(name: &str) -> Result<()> {
    validate_workspace_name(name)?;
    let path = named_workspace_path(name);
    if !path.is_file() {
        bail!("workspace does not exist: {}", path.display());
    }

    let editor = env::var("EDITOR").context("EDITOR is not set; set it to an editor command")?;
    if editor.trim().is_empty() {
        bail!("EDITOR is empty; set it to an editor command");
    }
    let mut arguments = shlex::split(&editor)
        .context("parse EDITOR as a command")?
        .into_iter();
    let program = arguments
        .next()
        .filter(|program| !program.is_empty())
        .context("EDITOR does not contain an editor command")?;
    let status = ProcessCommand::new(&program)
        .args(arguments)
        .arg(&path)
        .status()
        .with_context(|| format!("start editor {program:?}"))?;
    if !status.success() {
        bail!("editor exited unsuccessfully: {status}");
    }
    Ok(())
}

fn print_workspaces() -> Result<()> {
    for workspace in list_workspaces()? {
        println!("{workspace}");
    }
    Ok(())
}

fn split_show_args(mut args: Vec<String>) -> (Option<String>, String) {
    match args.len() {
        1 => (None, args.remove(0)),
        2 => {
            let item_id = args.remove(1);
            (Some(args.remove(0)), item_id)
        }
        _ => unreachable!("clap requires one or two show arguments"),
    }
}

/// Explicitly composes every statically linked built-in used by this CLI process.
///
/// Keeping registration here makes duplicate IDs fail before command dispatch and
/// guarantees Workspace loading and schema generation see the same frozen set.
pub fn register_builtins() -> Result<Registry> {
    let mut registry = Registry::new();
    registry.add_source::<QmdSourceDefinition>()?;
    registry.add_source::<JiraSourceDefinition>()?;
    registry.add_source::<GithubSourceDefinition>()?;
    registry.add_action::<RunCmdDefinition>()?;
    registry.add_action::<WorktreeDefinition>()?;
    Ok(registry)
}

fn start_signal_handler(
    cancellation: CancellationToken,
    force_exit_on_second_signal: bool,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_err() {
            return;
        }
        cancellation.cancel();
        if force_exit_on_second_signal && tokio::signal::ctrl_c().await.is_ok() {
            std::process::exit(130);
        }
        if !force_exit_on_second_signal {
            loop {
                if tokio::signal::ctrl_c().await.is_err() {
                    return;
                }
            }
        }
    })
}

/// Parse CLI arguments and dispatch the requested user command.
pub async fn run() -> Result<()> {
    let cancellation = CancellationToken::new();
    let registry = Arc::new(register_builtins()?);
    let cli = Cli::parse();
    let dashboard_command = matches!(&cli.command, Command::Dashboard { .. });
    let _signal_handler = start_signal_handler(cancellation.clone(), !dashboard_command);
    let verbosity = if cli.quiet {
        Verbosity::Quiet
    } else if cli.verbose > 0 {
        Verbosity::Verbose
    } else {
        Verbosity::Normal
    };
    // Delay output construction until after Workspace loading. Opening a diagnostic
    // log is a side effect, so invalid config must fail before this closure is called.
    let create_output = |workspace: &Workspace, use_default_log: bool| {
        let default_log = event_log_path(workspace);
        Output::new(
            verbosity,
            cli.color,
            cli.log_file
                .as_deref()
                .or(use_default_log.then_some(default_log.as_path())),
        )
    };
    let result = match cli.command {
        Command::Workspace { command } => match command {
            WorkspaceCommand::List => print_workspaces(),
            WorkspaceCommand::Init { name } => {
                println!("{}", init_workspace(&name)?.display());
                Ok(())
            }
            WorkspaceCommand::Edit { name } => edit_workspace(&name),
        },
        Command::Workspaces => print_workspaces(),
        Command::Run {
            workspace,
            dry_run,
            watch,
            interval,
        } => {
            let workspace = load_workspace(workspace.as_deref(), &registry)?;
            let output = create_output(&workspace, !dry_run)?;
            if watch {
                let interval = parse_duration(interval.as_deref().unwrap_or("60s"))?;
                run_watch(
                    workspace,
                    Arc::clone(&registry),
                    dry_run,
                    interval,
                    &output,
                    cancellation.clone(),
                )
                .await
            } else {
                run_once(
                    &workspace,
                    Arc::clone(&registry),
                    dry_run,
                    &output,
                    cancellation.clone(),
                )
                .await
            }
        }
        Command::List {
            workspace,
            json,
            watch,
            interval,
        } => {
            let workspace = load_workspace(workspace.as_deref(), &registry)?;
            if watch {
                require_watch_stdout(watch_stdout_is_terminal())?;
                let interval = parse_duration(interval.as_deref().unwrap_or("60s"))?;
                let output = create_output(&workspace, true)?;
                list_items_watch(workspace, interval, &output, cancellation.clone()).await
            } else {
                list_items(&workspace, json)
            }
        }
        Command::Dashboard { workspace } => {
            require_dashboard_terminals(
                std::io::stdin().is_terminal(),
                std::io::stdout().is_terminal(),
            )?;
            let workspace = load_workspace(workspace.as_deref(), &registry)?;
            dashboard(&workspace, cancellation.clone())
        }
        Command::Show {
            workspace_and_item,
            json,
            watch,
            interval,
        } => {
            let (workspace_name, item_id) = split_show_args(workspace_and_item);
            let workspace = load_workspace(workspace_name.as_deref(), &registry)?;
            if watch {
                require_watch_stdout(watch_stdout_is_terminal())?;
                let interval = parse_duration(interval.as_deref().unwrap_or("60s"))?;
                let output = create_output(&workspace, true)?;
                show_item_watch(workspace, item_id, interval, &output, cancellation.clone()).await
            } else {
                show_item(&workspace, &item_id, json)
            }
        }
        Command::Doctor { workspace } => {
            let workspace = load_workspace(workspace.as_deref(), &registry)?;
            let output = create_output(&workspace, true)?;
            doctor(&workspace, &registry, &output, cancellation.clone()).await
        }
        Command::Schema => {
            println!(
                "{}",
                serde_json::to_string_pretty(&workspace_schema(&registry)?)?
            );
            Ok(())
        }
    };
    if cancellation.is_cancelled() && !result.as_ref().is_err_and(is_cancelled) {
        Err(InvocationCancelled.into())
    } else {
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operational_commands_allow_omitting_workspace() {
        assert!(matches!(
            Cli::try_parse_from(["agentboard", "run"]).unwrap().command,
            Command::Run {
                workspace: None,
                dry_run: false,
                watch: false,
                interval: None,
            }
        ));
        assert!(matches!(
            Cli::try_parse_from(["agentboard", "run", "--watch"])
                .unwrap()
                .command,
            Command::Run {
                workspace: None,
                dry_run: false,
                watch: true,
                interval: None,
            }
        ));
        assert!(Cli::try_parse_from(["agentboard", "run", "--interval", "5s"]).is_err());
        assert!(Cli::try_parse_from(["agentboard", "list", "--interval", "5s"]).is_err());
        assert!(Cli::try_parse_from(["agentboard", "show", "AB-1", "--interval", "5s"]).is_err());
        assert!(Cli::try_parse_from(["agentboard", "list", "--watch", "--json"]).is_err());
        assert!(Cli::try_parse_from(["agentboard", "show", "AB-1", "--watch", "--json"]).is_err());
        assert!(matches!(
            Cli::try_parse_from(["agentboard", "doctor"])
                .unwrap()
                .command,
            Command::Doctor { workspace: None }
        ));
        assert!(matches!(
            Cli::try_parse_from(["agentboard", "dashboard"])
                .unwrap()
                .command,
            Command::Dashboard { workspace: None }
        ));
        assert!(matches!(
            Cli::try_parse_from(["agentboard", "list", "--watch"])
                .unwrap()
                .command,
            Command::List {
                workspace: None,
                json: false,
                watch: true,
                interval: None,
            }
        ));
    }

    #[test]
    fn show_distinguishes_optional_workspace_from_required_item_id() {
        assert!(Cli::try_parse_from(["agentboard", "show"]).is_err());

        let Command::Show {
            workspace_and_item,
            json: false,
            watch: false,
            interval: None,
        } = Cli::try_parse_from(["agentboard", "show", "AB-001"])
            .unwrap()
            .command
        else {
            panic!("expected show command");
        };
        assert_eq!(split_show_args(workspace_and_item), (None, "AB-001".into()));

        let Command::Show {
            workspace_and_item,
            json: false,
            watch: false,
            interval: None,
        } = Cli::try_parse_from(["agentboard", "show", "work", "AB-001"])
            .unwrap()
            .command
        else {
            panic!("expected show command");
        };
        assert_eq!(
            split_show_args(workspace_and_item),
            (Some("work".into()), "AB-001".into())
        );
    }
}
