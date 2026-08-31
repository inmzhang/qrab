use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use qrab::{Target, compile, parse};

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
            eprintln!("error: {error}\n\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: Vec<String>) -> Result<(), String> {
    let Some(command) = arguments.first().map(String::as_str) else {
        return Err("missing command".into());
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
        unknown => Err(format!("unknown command `{unknown}`")),
    }
}

fn check_command(arguments: &[String]) -> Result<(), String> {
    if arguments.len() != 1 {
        return Err("check expects exactly one input file".into());
    }
    let source = read_source(Path::new(&arguments[0]))?;
    let circuit = parse(&source).map_err(|error| error.to_string())?;
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
                    .ok_or("--target needs latex, typst, or all")?;
                target = OutputTarget::parse(value)?;
            }
            "--output" | "-o" => {
                position += 1;
                let value = arguments.get(position).ok_or("--output needs a path")?;
                output = Some(PathBuf::from(value));
            }
            option if option.starts_with('-') => {
                return Err(format!("unknown option `{option}`"));
            }
            path if input.is_none() => input = Some(PathBuf::from(path)),
            path => return Err(format!("unexpected second input `{path}`")),
        }
        position += 1;
    }

    let input = input.ok_or("compile needs an input file")?;
    if target == OutputTarget::All && output.is_some() {
        return Err("-o cannot be used with --target all".into());
    }
    let source = read_source(&input)?;

    match target {
        OutputTarget::Latex => {
            write_compiled(&source, Target::Latex, output_path(&input, output, "tex"))?;
        }
        OutputTarget::Typst => {
            write_compiled(&source, Target::Typst, output_path(&input, output, "typ"))?;
        }
        OutputTarget::All => {
            write_compiled(&source, Target::Latex, input.with_extension("tex"))?;
            write_compiled(&source, Target::Typst, input.with_extension("typ"))?;
        }
    }
    Ok(())
}

fn output_path(input: &Path, output: Option<PathBuf>, extension: &str) -> PathBuf {
    output.unwrap_or_else(|| input.with_extension(extension))
}

fn read_source(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("cannot read {}: {error}", path.display()))
}

fn write_compiled(source: &str, target: Target, path: PathBuf) -> Result<(), String> {
    let rendered = compile(source, target).map_err(|error| error.to_string())?;
    fs::write(&path, rendered)
        .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    println!("wrote {}", path.display());
    Ok(())
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
