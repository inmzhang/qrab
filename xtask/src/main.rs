//! Regenerates every file that is derived from the source tree but committed to
//! it: the shell completions and man pages the release archives ship, and the
//! diagrams the README and the manual embed.
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
    generate_manual_diagrams(&root)?;
    Ok(())
}

fn generate_cli_assets(assets: &Path) -> Result<(), Box<dyn Error>> {
    recreate(assets)?;

    // Completions describe the binary as invoked, so they keep clap's synthetic
    // `help` subcommand: it is something a user can actually type.
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

    // The man pages are built from a second command for two reasons. `version`
    // is derived from `CARGO_PKG_VERSION` at the point `cli.rs` is compiled,
    // which here is this crate, so without the override every shipped page
    // would advertise xtask's own 0.0.0. And `help` has nothing to document, so
    // dropping it keeps the parent page from cross-referencing a page that the
    // release archive does not contain.
    let mut manual = cli::Cli::command()
        .version(qrab::VERSION)
        .disable_help_subcommand(true);
    manual.build();
    clap_mangen::Man::new(manual.clone()).generate_to(assets)?;
    for subcommand in manual.get_subcommands() {
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

/// Renders every manual example, so an example can be added by dropping a file
/// into the directory rather than by editing a list here. The manual reads the
/// same file for its listing, which is what keeps a snippet and its picture
/// from ever drifting apart.
fn generate_manual_diagrams(root: &Path) -> Result<(), Box<dyn Error>> {
    let examples = root.join("docs/manual/examples");
    let images = root.join("docs/manual/images");
    recreate(&images)?;

    let mut entries = fs::read_dir(&examples)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort();
    for path in entries {
        if path.extension().is_none_or(|extension| extension != "qrab") {
            continue;
        }
        let name = path
            .file_stem()
            .expect("a `.qrab` path has a file stem")
            .to_string_lossy()
            .into_owned();
        let source = load_source(&path)?;
        let circuit = parse(source.as_str()).map_err(|error| format!("{name}: {error}"))?;
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
