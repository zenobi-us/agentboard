use agentboard_core::model::WorkspaceConfig;
use std::path::PathBuf;

use anyhow::Result;
use clap::{ArgAction, Parser, Subcommand};

use crate::{
    config::{init_workspace, list_workspaces, load_workspace, load_workspace_for_doctor},
    output::{ColorChoice, Output, Verbosity},
    runtime::{parse_duration, run_once, watch},
    store::{doctor, list_items, show_item},
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
        workspace: String,
        #[arg(long)]
        dry_run: bool,
    },
    /// Repeatedly run one workspace.
    Watch {
        workspace: String,
        #[arg(long, default_value = "60s")]
        interval: String,
    },
    /// List latest stored items.
    List {
        workspace: String,
        #[arg(long)]
        json: bool,
    },
    /// Show one latest stored item and action attempts.
    Show {
        workspace: String,
        item_id: String,
        #[arg(long)]
        json: bool,
    },
    /// Validate workspace and local environment.
    Doctor { workspace: String },
    /// Print workspace JSON Schema.
    Schema,
}

#[derive(Debug, Subcommand)]
enum WorkspaceCommand {
    /// List available named Workspaces.
    List,
    /// Create an empty named Workspace.
    Init { name: String },
}

fn print_workspaces() -> Result<()> {
    for workspace in list_workspaces()? {
        println!("{workspace}");
    }
    Ok(())
}

/// Parse CLI arguments and dispatch the requested user command.
pub async fn run() -> Result<()> {
    let cli = Cli::parse();
    let verbosity = if cli.quiet {
        Verbosity::Quiet
    } else if cli.verbose > 0 {
        Verbosity::Verbose
    } else {
        Verbosity::Normal
    };
    let output = Output::new(verbosity, cli.color, cli.log_file.as_deref())?;
    match cli.command {
        Command::Workspace { command } => match command {
            WorkspaceCommand::List => print_workspaces(),
            WorkspaceCommand::Init { name } => {
                println!("{}", init_workspace(&name)?.display());
                Ok(())
            }
        },
        Command::Workspaces => print_workspaces(),
        Command::Run { workspace, dry_run } => {
            run_once(&load_workspace(&workspace)?, dry_run, &output).await
        }
        Command::Watch {
            workspace,
            interval,
        } => {
            watch(
                load_workspace(&workspace)?,
                parse_duration(&interval)?,
                &output,
            )
            .await
        }
        Command::List { workspace, json } => list_items(&load_workspace(&workspace)?, json),
        Command::Show {
            workspace,
            item_id,
            json,
        } => show_item(&load_workspace(&workspace)?, &item_id, json),
        Command::Doctor { workspace } => {
            doctor(&load_workspace_for_doctor(&workspace)?, &output).await
        }
        Command::Schema => {
            println!(
                "{}",
                serde_json::to_string_pretty(&schemars::schema_for!(WorkspaceConfig))?
            );
            Ok(())
        }
    }
}
