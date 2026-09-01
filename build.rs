use std::error::Error;
use std::fs;
use std::path::PathBuf;

use clap::CommandFactory;
use clap_complete::{Shell, generate_to};

#[path = "src/cli.rs"]
mod cli;

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=src/cli.rs");
    let assets =
        PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR")).join("assets");
    if assets.exists() {
        fs::remove_dir_all(&assets)?;
    }
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
    command.build();
    clap_mangen::Man::new(command.clone()).generate_to(&assets)?;
    for subcommand in command
        .get_subcommands()
        .filter(|subcommand| subcommand.get_name() != "help")
    {
        let name = format!("qrab-{}", subcommand.get_name());
        let invocation = format!("qrab {}", subcommand.get_name());
        clap_mangen::Man::new(subcommand.clone().bin_name(invocation).display_name(name))
            .generate_to(&assets)?;
    }
    Ok(())
}
