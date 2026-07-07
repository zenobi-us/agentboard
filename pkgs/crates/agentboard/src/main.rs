use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "agentboard")]
#[command(about = "Collect task-tracking items into local agent work queues")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print planned CLI vision.
    Vision,
}

fn main() {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Command::Vision) {
        Command::Vision => {
            println!("AgentBoard: collect -> store locally -> run workspace actions");
        }
    }
}
