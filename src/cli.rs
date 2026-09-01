use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "qrab",
    version,
    about = "Readable quantum circuits for TikZ, Typst/Quill, SVG, and Quirk",
    propagate_version = true,
    arg_required_else_help = true
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Check a circuit without rendering it.
    Check {
        /// Input `.qrab` file.
        input: PathBuf,
    },
    /// Compile a circuit to LaTeX, Typst, SVG, Quirk, or all of them.
    Compile(CompileArgs),
}

#[derive(Debug, Args)]
pub(crate) struct CompileArgs {
    /// Input `.qrab` file.
    pub(crate) input: PathBuf,
    /// Output backend.
    #[arg(short, long, value_enum, default_value_t = OutputTarget::All)]
    pub(crate) target: OutputTarget,
    /// Output file; requires a single backend.
    #[arg(short, long)]
    pub(crate) output: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum OutputTarget {
    #[value(alias = "tikz")]
    Latex,
    #[value(alias = "quill")]
    Typst,
    Svg,
    Quirk,
    All,
}
