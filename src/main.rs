use std::fs;
use std::path::{Path, PathBuf};

use clap::{CommandFactory, Parser, error::ErrorKind};
use miette::{IntoDiagnostic, Report, Result, WrapErr};
use qrab::{Circuit, Target, load_source, parse, render};

mod cli;

use cli::{Cli, Command, CompileArgs, OutputTarget};

fn main() -> Result<()> {
    let cli = Cli::parse();
    if let Command::Compile(arguments) = &cli.command
        && let Err(error) = validate_compile_args(arguments)
    {
        error.exit();
    }
    run(cli.command)
}

fn run(command: Command) -> Result<()> {
    match command {
        Command::Check { input } => check_command(&input),
        Command::Compile(arguments) => compile_command(arguments),
    }
}

fn check_command(input: &Path) -> Result<()> {
    let source = load_source(input)?;
    let circuit = parse(source.as_str())
        .map_err(|error| Report::new(error).with_source_code(source.clone()))?;
    println!(
        "{}: {} wire(s), {} operation(s)",
        circuit.name,
        circuit.wires.len(),
        circuit.operations.len()
    );
    Ok(())
}

fn compile_command(arguments: CompileArgs) -> Result<()> {
    let CompileArgs {
        input,
        target,
        output,
    } = arguments;
    let source = load_source(&input)?;
    let circuit = parse(source.as_str())
        .map_err(|error| Report::new(error).with_source_code(source.clone()))?;

    match target {
        OutputTarget::Latex => {
            write_compiled(
                &circuit,
                Target::Latex,
                output.unwrap_or_else(|| input.with_extension("tex")),
            )?;
        }
        OutputTarget::Typst => {
            write_compiled(
                &circuit,
                Target::Typst,
                output.unwrap_or_else(|| input.with_extension("typ")),
            )?;
        }
        OutputTarget::All => {
            write_compiled(&circuit, Target::Latex, input.with_extension("tex"))?;
            write_compiled(&circuit, Target::Typst, input.with_extension("typ"))?;
        }
    }
    Ok(())
}

fn validate_compile_args(arguments: &CompileArgs) -> std::result::Result<(), clap::Error> {
    if arguments.target != OutputTarget::All || arguments.output.is_none() {
        return Ok(());
    }
    let mut command = Cli::command();
    let compile = command
        .find_subcommand_mut("compile")
        .expect("compile subcommand is defined");
    Err(compile.error(
        ErrorKind::ArgumentConflict,
        "the argument '--output <OUTPUT>' cannot be used with '--target all'",
    ))
}

fn write_compiled(circuit: &Circuit, target: Target, path: PathBuf) -> Result<()> {
    fs::write(&path, render(circuit, target))
        .into_diagnostic()
        .wrap_err_with(|| format!("cannot write {}", path.display()))?;
    println!("wrote {}", path.display());
    Ok(())
}
