use std::env;
use std::path::{Path, PathBuf};

use clap::Parser;

mod commands;
mod util;

#[derive(Parser)]
#[command(name = "xtask", about = "RolX app workspace tasks")]
struct Args {
    #[command(subcommand)]
    command: commands::Commands,
}

fn main() -> Result<(), String> {
    let args = Args::parse();

    let root = env::var_os("CARGO_MANIFEST_DIR")
        .and_then(|d| PathBuf::from(d).parent().map(Path::to_path_buf))
        .ok_or("not run via cargo (cannot derive workspace root from CARGO_MANIFEST_DIR)")?;

    args.command.run(&root).map(|msg| {
        println!("{msg}");
    })
}
