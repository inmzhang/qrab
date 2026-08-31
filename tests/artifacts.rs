use std::fs;
use std::path::PathBuf;
use std::process::Command;

use qrab::{Target, compile, load_source};

const QPIC_GOLDEN_TESTS: &[&str] = &[
    "2Bitcomp",
    "Adder_CDKM",
    "Adder_CDKM_MAJ",
    "Adder_CDKM_UMA",
    "Adder_VBE",
    "Adder_VBE_Carry",
    "Adder_VBE_Sum",
    "AutoTest",
    "BasicTeleportation",
    "CoherentSuperDenseCoding",
    "ModAdder",
    "ModExp",
    "NestedLevels",
    "PleaseTouch",
    "QFT3v1",
    "QFT3v2",
    "QFT4",
    "QFT4vert",
    "QuantumTeleportation",
    "RecursiveQFT",
    "RecursiveQFTv2",
    "ShapeExamples",
    "ShapeExamplesVertical",
    "ShorNutshell",
    "Simon",
    "Steane_NOOP",
    "SuperDenseCoding",
    "Teleport",
    "boxtest",
    "cswap",
    "gatecompare",
    "measure",
    "measure_tag",
    "oop",
    "permute",
    "phantom_test",
    "reverse-8",
    "sink",
    "start_and_end",
    "starwars",
    "test-rotate",
    "test40",
    "through",
    "wiretest",
];

const QPIC_MANUAL_EXAMPLES: &[&str] = &[
    "Adder_CDKM_MAJ",
    "QFT3v1",
    "ShorNutshell",
    "ex.BARRIER",
    "ex.C",
    "ex.CHANGEcwire",
    "ex.CUT",
    "ex.DEFINE",
    "ex.DEFINEargs",
    "ex.G",
    "ex.GG",
    "ex.Gbar",
    "ex.HX",
    "ex.INOUT",
    "ex.M",
    "ex.MIXGATES",
    "ex.Mtag",
    "ex.N",
    "ex.P",
    "ex.PERMUTE",
    "ex.PHANTOM",
    "ex.Pwidth",
    "ex.R1",
    "ex.R2",
    "ex.Rmark",
    "ex.S",
    "ex.STARTEND",
    "ex.SWAP",
    "ex.T",
    "ex.TOUCH",
    "ex.W",
    "ex.Z",
    "ex.at",
    "ex.atfill",
    "ex.breadth",
    "ex.color",
    "ex.comment",
    "ex.delay",
    "ex.ellipsis",
    "ex.equals",
    "ex.equals2",
    "ex.fill",
    "ex.hyperlink",
    "ex.hypertarget",
    "ex.label",
    "ex.level",
    "ex.nW",
    "ex.noTOUCH",
    "ex.none",
    "ex.operator",
    "ex.operator2",
    "ex.operatorquotes",
    "ex.plus",
    "ex.qcowire",
    "ex.semicolon",
    "ex.setsize",
    "ex.shape",
    "ex.size",
    "ex.sizevert",
    "ex.slash",
    "ex.style",
    "ex_latex",
    "reverse-8",
    "teleport",
];

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
    compile_fixture(
        "functions",
        include_str!("../examples/functions.qrab"),
        &output_dir,
    );
    compile_fixture(
        "programming",
        include_str!("../examples/programming.qrab"),
        &output_dir,
    );
    compile_fixture(
        "annotations",
        include_str!("../examples/annotations.qrab"),
        &output_dir,
    );
    compile_fixture(
        "regions",
        include_str!("../examples/regions.qrab"),
        &output_dir,
    );
    compile_fixture(
        "measurements",
        include_str!("../examples/measurements.qrab"),
        &output_dir,
    );
    compile_fixture(
        "ellipsis",
        include_str!("../examples/ellipsis.qrab"),
        &output_dir,
    );
    compile_fixture(
        "escapes",
        include_str!("../examples/escapes.qrab"),
        &output_dir,
    );
    let imports = load_source("examples/imports.qrab").expect("load imported example");
    compile_fixture("imports", imports.as_str(), &output_dir);

    for name in QPIC_GOLDEN_TESTS {
        let source = fs::read_to_string(format!("tests/qpic/{name}.qrab"))
            .unwrap_or_else(|error| panic!("missing qpic parity fixture `{name}`: {error}"));
        compile_fixture(&format!("qpic-{name}"), &source, &output_dir);
    }
    for name in QPIC_MANUAL_EXAMPLES {
        let source = fs::read_to_string(format!("tests/qpic-manual/{name}.qrab"))
            .unwrap_or_else(|error| panic!("missing qpic manual fixture `{name}`: {error}"));
        compile_fixture(&format!("qpic-manual-{name}"), &source, &output_dir);
    }
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
