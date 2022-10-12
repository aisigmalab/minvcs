use clap::{Parser, Subcommand};
use std::path::PathBuf;
mod commands;
mod files;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new repository
    Init,

    /// Take snapshot of the directory
    Snap,

    /// Store a file
    Store {
        #[clap(value_parser)]
        path: PathBuf,
    },

    /// Print an object specified by a hash key
    Cat {
        #[clap(value_parser)]
        hash: String,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init => commands::init::run(),
        Commands::Snap => commands::snap::run(),
        Commands::Store {path} => commands::store::run(&path),
        Commands::Cat {hash} => commands::cat::run(&hash),
    }
}
