use agentboard_core::model::WorkspaceConfig;
use std::{env, path::PathBuf, process::Command as ProcessCommand};

use anyhow::{bail, Context, Result};
use clap::{ArgAction, Parser, Subcommand};

use crate::{
    config::{
        init_workspace, list_workspaces, load_workspace, load_workspace_for_doctor,
        named_workspace_path, validate_workspace_name,
    },
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
            WorkspaceCommand::Edit { name } => edit_workspace(&name),
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
