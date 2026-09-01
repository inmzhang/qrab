use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{CommandFactory, Parser, error::ErrorKind};
use qrab::{Diagnostic, LoadedSource, Target, compile, load_source, parse};

mod cli;

use cli::{Cli, Command, CompileArgs, OutputTarget};

fn main() -> ExitCode {
    let cli = Cli::parse();
    if let Command::Compile(arguments) = &cli.command
        && let Err(error) = validate_compile_args(arguments)
    {
        error.exit();
    }
    match run(cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: Command) -> Result<(), String> {
    match command {
        Command::Check { input } => check_command(&input),
        Command::Compile(arguments) => compile_command(arguments),
    }
}

fn check_command(input: &Path) -> Result<(), String> {
    let source = load_source(input).map_err(|error| error.to_string())?;
    let circuit = parse(source.as_str()).map_err(|error| format_diagnostic(&source, &error))?;
    println!(
        "{}: {} wire(s), {} operation(s)",
        circuit.name,
        circuit.wires.len(),
        circuit.operations.len()
    );
    Ok(())
}

fn compile_command(arguments: CompileArgs) -> Result<(), String> {
    let CompileArgs {
        input,
        target,
        output,
    } = arguments;
    let source = load_source(&input).map_err(|error| error.to_string())?;

    match target {
        OutputTarget::Latex => {
            write_compiled(
                &source,
                Target::Latex,
                output.unwrap_or_else(|| input.with_extension("tex")),
            )?;
        }
        OutputTarget::Typst => {
            write_compiled(
                &source,
                Target::Typst,
                output.unwrap_or_else(|| input.with_extension("typ")),
            )?;
        }
        OutputTarget::All => {
            write_compiled(&source, Target::Latex, input.with_extension("tex"))?;
            write_compiled(&source, Target::Typst, input.with_extension("typ"))?;
        }
    }
    Ok(())
}

fn validate_compile_args(arguments: &CompileArgs) -> Result<(), clap::Error> {
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

fn write_compiled(source: &LoadedSource, target: Target, path: PathBuf) -> Result<(), String> {
    let rendered =
        compile(source.as_str(), target).map_err(|error| format_diagnostic(source, &error))?;
    fs::write(&path, rendered)
        .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    println!("wrote {}", path.display());
    Ok(())
}

fn format_diagnostic(source: &LoadedSource, diagnostic: &Diagnostic) -> String {
    let (path, line) = source
        .origin(diagnostic.span.line)
        .unwrap_or((Path::new("<expanded>"), diagnostic.span.line));
    let source_line = source
        .as_str()
        .lines()
        .nth(diagnostic.span.line.saturating_sub(1))
        .unwrap_or("");
    format!(
        "{}:{line}:{}: {}\n  {source_line}\n  {}^",
        path.display(),
        diagnostic.span.column,
        diagnostic.message,
        " ".repeat(diagnostic.span.column.saturating_sub(1))
    )
}
