mod cli;
mod config;
mod gui;
pub mod cache;
pub mod render;

use cli::{determine_execution_mode, ExecutionMode, parse_args};
use config::Config;
use eframe;
mod cli_runner;

fn main() -> eframe::Result<()> {
    let args = parse_args();
    let _config = Config::load();
    let mode = determine_execution_mode();

    match mode {
        ExecutionMode::CLI => {
            cli_runner::run_cli(args, _config);
            Ok(())
        }
        ExecutionMode::GUI => {
            let options = gui::get_eframe_options();
            eframe::run_native(
                "Phosphene",
                options,
                Box::new(move |cc| Ok(Box::new(gui::PhospheneApp::new(cc, args, _config)))),
            )
        }
    }
}
