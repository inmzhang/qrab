use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::{CommandFactory, Parser, error::ErrorKind};
use miette::{IntoDiagnostic, Report, Result, WrapErr};
use qrab::{Circuit, Target, from_quirk_url, load_source, parse, render};

mod cli;

use cli::{Cli, Command, CompileArgs, OutputTarget};

const SKILL: &str = include_str!("../.agents/skills/qrab/SKILL.md");
const SKILL_PATH: &str = ".agents/skills/qrab/SKILL.md";

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
        Command::ImportQuirk { url, output } => import_quirk(&url, output),
        Command::InstallSkill => install_skill(),
    }
}

fn import_quirk(url: &str, output: Option<PathBuf>) -> Result<()> {
    let source = from_quirk_url(url).into_diagnostic()?;
    let Some(path) = output else {
        print!("{source}");
        return Ok(());
    };
    fs::write(&path, source)
        .into_diagnostic()
        .wrap_err_with(|| format!("cannot write {}", path.display()))?;
    println!("wrote {}", path.display());
    Ok(())
}

fn install_skill() -> Result<()> {
    let path = Path::new(SKILL_PATH);
    if path.exists() {
        let installed = fs::read_to_string(path)
            .into_diagnostic()
            .wrap_err_with(|| format!("cannot read {}", path.display()))?;
        if installed != SKILL {
            miette::bail!("refusing to overwrite existing {}", path.display());
        }
        println!("already installed {}", path.display());
        return Ok(());
    }

    fs::create_dir_all(path.parent().expect("skill path has a parent"))
        .into_diagnostic()
        .wrap_err_with(|| format!("cannot create parent of {}", path.display()))?;
    fs::File::create_new(path)
        .and_then(|mut file| file.write_all(SKILL.as_bytes()))
        .into_diagnostic()
        .wrap_err_with(|| format!("cannot write {}", path.display()))?;
    println!("installed {}", path.display());
    Ok(())
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
        OutputTarget::Svg => {
            write_compiled(
                &circuit,
                Target::Svg,
                output.unwrap_or_else(|| input.with_extension("svg")),
            )?;
        }
        OutputTarget::Quirk => {
            write_compiled(
                &circuit,
                Target::Quirk,
                output.unwrap_or_else(|| input.with_extension("url")),
            )?;
        }
        OutputTarget::All => {
            write_compiled(&circuit, Target::Latex, input.with_extension("tex"))?;
            write_compiled(&circuit, Target::Typst, input.with_extension("typ"))?;
            write_compiled(&circuit, Target::Svg, input.with_extension("svg"))?;
            write_compiled(&circuit, Target::Quirk, input.with_extension("url"))?;
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
