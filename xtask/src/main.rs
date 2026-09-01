//! Regenerates every file that is derived from the source tree but committed to
//! it: the shell completions and man pages the release archives ship, and the
//! diagrams the README embeds.
//!
//! CI runs this and fails on any diff, so the generated files can never drift
//! from the CLI definition or the renderer.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use clap::CommandFactory;
use clap_complete::{Shell, generate_to};
use qrab::{Target, load_source, parse, render};

// The CLI definition is included as a module rather than reached through the
// `qrab` library, because `Cli` is an implementation detail of the binary and
// should not become public API just to be documented.
#[path = "../../src/cli.rs"]
mod cli;

/// Examples the README walks through, in the order it presents them.
const DIAGRAMS: &[&str] = &[
    "bell",
    "teleportation",
    "functions",
    "annotations",
    "lifecycle",
];

fn main() -> Result<(), Box<dyn Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    generate_cli_assets(&root.join("assets"))?;
    generate_diagrams(&root)?;
    Ok(())
}

fn generate_cli_assets(assets: &Path) -> Result<(), Box<dyn Error>> {
    recreate(assets)?;

    let mut command = cli::Cli::command();
    for shell in [
        Shell::Bash,
        Shell::Elvish,
        Shell::Fish,
        Shell::PowerShell,
        Shell::Zsh,
    ] {
        generate_to(shell, &mut command, "qrab", assets)?;
    }
    command.build();
    clap_mangen::Man::new(command.clone()).generate_to(assets)?;
    // `help` is clap's synthetic subcommand; a man page for it would document
    // nothing and ship in every release archive.
    for subcommand in command
        .get_subcommands()
        .filter(|subcommand| subcommand.get_name() != "help")
    {
        let name = format!("qrab-{}", subcommand.get_name());
        let invocation = format!("qrab {}", subcommand.get_name());
        clap_mangen::Man::new(subcommand.clone().bin_name(invocation).display_name(name))
            .generate_to(assets)?;
    }
    Ok(())
}

fn generate_diagrams(root: &Path) -> Result<(), Box<dyn Error>> {
    let images = root.join("docs/images");
    recreate(&images)?;
    for name in DIAGRAMS {
        let source = load_source(root.join(format!("examples/{name}.qrab")))?;
        let circuit = parse(source.as_str()).map_err(|error| error.to_string())?;
        fs::write(
            images.join(format!("{name}.svg")),
            render(&circuit, Target::Svg),
        )?;
    }
    Ok(())
}

/// Empties a generated directory so a removed input cannot leave a stale file
/// behind that the drift check would never notice.
fn recreate(directory: &Path) -> Result<(), Box<dyn Error>> {
    if directory.exists() {
        fs::remove_dir_all(directory)?;
    }
    fs::create_dir_all(directory)?;
    Ok(())
}
