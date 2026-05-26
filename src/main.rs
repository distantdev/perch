mod cli;
mod config;
mod error;
mod model;
mod platform;
mod process;
mod scan;
mod tui;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use error::Result;

#[derive(Parser, Debug)]
#[command(
    name = "perch",
    version,
    about = "Discover and manage local development servers"
)]
struct Cli {
    /// Path to config file (default: ~/.config/perch/config.toml)
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    /// Verbose scan diagnostics
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Show all local listeners (disables dev-focused scan defaults)
    #[arg(long, global = true)]
    all: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// List active development listeners
    #[command(alias = "ls")]
    List {
        #[arg(long, value_enum, default_value = "table")]
        format: OutputFormat,
    },
    /// Interactive dashboard (same as default)
    Ui,
    /// Stop a process by port or PID
    Kill {
        target: String,
        #[arg(short, long)]
        force: bool,
    },
    /// Pause (freeze) a process by port or PID
    Pause { target: String },
    /// Resume a paused process by port or PID
    Resume { target: String },
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum OutputFormat {
    Table,
    Json,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let mut config = config::Config::load(cli.config.as_deref())?;
    if cli.all {
        config = config.with_show_all();
    }
    let scanner = platform::default_scanner();

    match cli.command {
        None | Some(Commands::Ui) => {
            let mut app = tui::app::TuiApp::new(config);
            app.run()?;
        }
        Some(Commands::List { format }) => {
            let fmt = match format {
                OutputFormat::Table => cli::list::ListFormat::Table,
                OutputFormat::Json => cli::list::ListFormat::Json,
            };
            cli::list::run_list(scanner.as_ref(), &config, fmt, cli.verbose)?;
        }
        Some(Commands::Kill { target, force }) => {
            cli::kill::run_kill(&target, force, scanner.as_ref(), &config)?;
        }
        Some(Commands::Pause { target }) => {
            cli::pause::run_pause(&target, scanner.as_ref(), &config)?;
        }
        Some(Commands::Resume { target }) => {
            cli::pause::run_resume(&target, scanner.as_ref(), &config)?;
        }
    }

    Ok(())
}
