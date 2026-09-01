use std::error::Error;
use std::fs::{self, File};
use std::path::PathBuf;

use clap::CommandFactory;
use clap_complete::{Shell, generate_to};

#[path = "src/cli.rs"]
mod cli;

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=src/cli.rs");
    let assets =
        PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR")).join("assets");
    fs::create_dir_all(&assets)?;

    let mut command = cli::Cli::command();
    for shell in [
        Shell::Bash,
        Shell::Elvish,
        Shell::Fish,
        Shell::PowerShell,
        Shell::Zsh,
    ] {
        generate_to(shell, &mut command, "qrab", &assets)?;
    }
    clap_mangen::Man::new(cli::Cli::command()).render(&mut File::create(assets.join("qrab.1"))?)?;
    Ok(())
}
