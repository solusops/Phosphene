use std::io::IsTerminal;
use clap::Parser;

pub enum ExecutionMode {
    CLI,
    GUI,
}

pub fn determine_execution_mode() -> ExecutionMode {
    if std::io::stdout().is_terminal() {
        ExecutionMode::CLI
    } else {
        ExecutionMode::GUI
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct CliArgs {
    /// Path to the binary file to analyze
    pub file_path: String,
}

pub fn parse_args() -> CliArgs {
    CliArgs::parse()
}
