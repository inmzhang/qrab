use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use qrab::{Diagnostic, LoadedSource, Target, compile, load_source, parse};

const USAGE: &str = "\
qrab — readable quantum circuits for TikZ and Typst/Quill

Usage:
  qrab compile <input.qrab> [--target latex|typst|all] [-o <output>]
  qrab check <input.qrab>
  qrab --help

`--target all` writes <input>.tex and <input>.typ and cannot be combined with -o.
The default target is all.";

fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: Vec<String>) -> Result<(), String> {
    let Some(command) = arguments.first().map(String::as_str) else {
        return Err(usage_error("missing command"));
    };
    match command {
        "--help" | "-h" | "help" => {
            println!("{USAGE}");
            Ok(())
        }
        "--version" | "-V" => {
            println!("qrab {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "check" => check_command(&arguments[1..]),
        "compile" => compile_command(&arguments[1..]),
        unknown => Err(usage_error(format!("unknown command `{unknown}`"))),
    }
}

fn check_command(arguments: &[String]) -> Result<(), String> {
    if arguments.len() != 1 {
        return Err(usage_error("check expects exactly one input file"));
    }
    let source = load_source(Path::new(&arguments[0])).map_err(|error| error.to_string())?;
    let circuit = parse(source.as_str()).map_err(|error| format_diagnostic(&source, &error))?;
    println!(
        "{}: {} wire(s), {} operation(s)",
        circuit.name,
        circuit.wires.len(),
        circuit.operations.len()
    );
    Ok(())
}

fn compile_command(arguments: &[String]) -> Result<(), String> {
    let mut input = None;
    let mut output = None;
    let mut target = OutputTarget::All;
    let mut position = 0;

    while position < arguments.len() {
        match arguments[position].as_str() {
            "--target" | "-t" => {
                position += 1;
                let value = arguments
                    .get(position)
                    .ok_or_else(|| usage_error("--target needs latex, typst, or all"))?;
                target = OutputTarget::parse(value).map_err(usage_error)?;
            }
            "--output" | "-o" => {
                position += 1;
                let value = arguments
                    .get(position)
                    .ok_or_else(|| usage_error("--output needs a path"))?;
                output = Some(PathBuf::from(value));
            }
            option if option.starts_with('-') => {
                return Err(usage_error(format!("unknown option `{option}`")));
            }
            path if input.is_none() => input = Some(PathBuf::from(path)),
            path => return Err(usage_error(format!("unexpected second input `{path}`"))),
        }
        position += 1;
    }

    let input = input.ok_or_else(|| usage_error("compile needs an input file"))?;
    if target == OutputTarget::All && output.is_some() {
        return Err(usage_error("-o cannot be used with --target all"));
    }
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

fn usage_error(message: impl std::fmt::Display) -> String {
    format!("{message}\n\n{USAGE}")
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputTarget {
    Latex,
    Typst,
    All,
}

impl OutputTarget {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "latex" | "tikz" => Ok(Self::Latex),
            "typst" | "quill" => Ok(Self::Typst),
            "all" => Ok(Self::All),
            _ => Err(format!("unknown target `{value}`")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_is_only_attached_to_argument_errors() {
        assert!(
            run(Vec::new())
                .expect_err("missing command")
                .contains(USAGE)
        );
        assert!(
            !run(vec!["check".into(), "Cargo.toml".into()])
                .expect_err("Cargo.toml is not qrab source")
                .contains(USAGE)
        );
    }
}
