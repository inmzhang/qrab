use std::fs;
use std::path::PathBuf;
use std::process::Command;

use qrab::{Target, compile, load_source};

const EXAMPLE_FIXTURES: &[&str] = &[
    "bell",
    "teleportation",
    "styling",
    "lifecycle",
    "functions",
    "programming",
    "annotations",
    "math-labels",
    "regions",
    "measurements",
    "ellipsis",
    "escapes",
    "imports",
];
const QPIC_FIXTURES: &[&str] = &["permute", "QFT4vert", "start_and_end", "ShorNutshell"];

const PAGE_SIZE_BASELINES: &[(&str, f32, f32)] = &[
    ("teleportation", 460.62, 92.86),
    ("teleportation-typst", 276.945, 106.24),
    ("qpic-QFT4vert", 126.41, 440.98),
    ("qpic-QFT4vert-typst", 148.08, 311.142),
    ("imports", 232.33, 60.61),
    ("imports-typst", 166.125, 65.66),
    ("styling", 161.63, 439.36),
    ("qpic-start_and_end-typst", 151.89, 119.44),
];

#[test]
#[ignore = "requires tectonic, typst, pdfinfo, and network access for their first package download"]
fn generated_backends_compile_to_pdfs() {
    let output_dir = PathBuf::from("target/artifact-tests");
    fs::create_dir_all(&output_dir).expect("create artifact test directory");

    for name in EXAMPLE_FIXTURES {
        let source = load_source(format!("examples/{name}.qrab"))
            .unwrap_or_else(|error| panic!("load example {name}: {error}"));
        compile_fixture(name, source.as_str(), &output_dir);
    }
    let math_labels =
        fs::read_to_string("examples/math-labels.qrab").expect("load math-labels example");
    compile_fixture(
        "math-labels-vertical",
        &math_labels.replacen('{', "{\n  layout { orientation: vertical }", 1),
        &output_dir,
    );

    for name in QPIC_FIXTURES {
        let source = fs::read_to_string(format!("tests/qpic/{name}.qrab"))
            .unwrap_or_else(|error| panic!("missing qpic fixture `{name}`: {error}"));
        compile_fixture(&format!("qpic-{name}"), &source, &output_dir);
    }
    // Report all drift in one run.
    let drift = PAGE_SIZE_BASELINES
        .iter()
        .filter_map(|(name, width, height)| {
            page_size_drift(&output_dir.join(format!("{name}.pdf")), *width, *height)
        })
        .collect::<Vec<_>>();
    assert!(
        drift.is_empty(),
        "page geometry moved:\n{}",
        drift.join("\n")
    );
}

/// Returns a description of how `path` differs from its baseline, or `None`
/// when it is still within tolerance.
fn page_size_drift(
    path: &std::path::Path,
    expected_width: f32,
    expected_height: f32,
) -> Option<String> {
    let output = Command::new("pdfinfo")
        .arg(path)
        .output()
        .unwrap_or_else(|error| panic!("could not run pdfinfo: {error}"));
    assert!(
        output.status.success(),
        "pdfinfo failed for {}",
        path.display()
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let fields = stdout
        .lines()
        .find(|line| line.starts_with("Page size:"))
        .unwrap_or_else(|| panic!("pdfinfo omitted page size for {}", path.display()))
        .split_whitespace()
        .collect::<Vec<_>>();
    let width = fields[2].parse::<f32>().expect("numeric PDF width");
    let height = fields[4].parse::<f32>().expect("numeric PDF height");
    ((width - expected_width).abs() > 3.0 || (height - expected_height).abs() > 3.0).then(|| {
        format!(
            "  {} changed from {expected_width:.3} x {expected_height:.3} pt to {width:.3} x {height:.3} pt",
            path.display()
        )
    })
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
