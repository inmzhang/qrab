use std::fs;
use std::path::PathBuf;
use std::process::Command;

use qrab::{Target, compile};

#[test]
#[ignore = "requires tectonic, typst, and network access for their first package download"]
fn generated_backends_compile_to_pdfs() {
    let output_dir = PathBuf::from("target/artifact-tests");
    fs::create_dir_all(&output_dir).expect("create artifact test directory");

    compile_fixture(
        "teleportation",
        include_str!("../examples/teleportation.qrab"),
        &output_dir,
    );
    compile_fixture(
        "styling",
        include_str!("../examples/styling.qrab"),
        &output_dir,
    );
    compile_fixture(
        "lifecycle",
        include_str!("../examples/lifecycle.qrab"),
        &output_dir,
    );
}

fn compile_fixture(name: &str, source: &str, output_dir: &std::path::Path) {
    let latex = output_dir.join(format!("{name}.tex"));
    let typst = output_dir.join(format!("{name}.typ"));
    fs::write(
        &latex,
        compile(source, Target::Latex).expect("compile LaTeX source"),
    )
    .expect("write LaTeX artifact");
    fs::write(
        &typst,
        compile(source, Target::Typst).expect("compile Typst source"),
    )
    .expect("write Typst artifact");

    run(
        "tectonic",
        &[
            latex.to_string_lossy().as_ref(),
            "--outdir",
            output_dir.to_string_lossy().as_ref(),
        ],
    );
    let typst_pdf = output_dir.join(format!("{name}-typst.pdf"));
    run(
        "typst",
        &[
            "compile",
            typst.to_string_lossy().as_ref(),
            typst_pdf.to_string_lossy().as_ref(),
        ],
    );

    assert!(output_dir.join(format!("{name}.pdf")).is_file());
    assert!(typst_pdf.is_file());
}

fn run(program: &str, arguments: &[&str]) {
    let output = Command::new(program)
        .args(arguments)
        .output()
        .unwrap_or_else(|error| panic!("could not run {program}: {error}"));
    assert!(
        output.status.success(),
        "{program} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
