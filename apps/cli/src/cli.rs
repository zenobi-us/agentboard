use agentboard_core::model::WorkspaceConfig;
use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::{
    config::load_workspace,
    runtime::{parse_duration, run_once, watch},
    store::{doctor, list_items, show_item},
};

#[derive(Debug, Parser)]
#[command(name = "agentboard")]
#[command(about = "Collect task-tracking items into local agent work queues")]
/// Parsed `agentboard` command-line interface.
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
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

/// Parse CLI arguments and dispatch the requested user command.
pub async fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Run { workspace, dry_run } => {
            run_once(&load_workspace(&workspace)?, dry_run).await
        }
        Command::Watch {
            workspace,
            interval,
        } => watch(load_workspace(&workspace)?, parse_duration(&interval)?).await,
        Command::List { workspace, json } => list_items(&load_workspace(&workspace)?, json),
        Command::Show {
            workspace,
            item_id,
            json,
        } => show_item(&load_workspace(&workspace)?, &item_id, json),
        Command::Doctor { workspace } => doctor(&load_workspace(&workspace)?).await,
        Command::Schema => {
            println!(
                "{}",
                serde_json::to_string_pretty(&schemars::schema_for!(WorkspaceConfig))?
            );
            Ok(())
        }
    }
}
