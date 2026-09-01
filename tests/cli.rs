use std::fs;

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;

#[test]
fn clap_handles_help_errors_delimiters_and_conflicts() {
    cargo_bin_cmd!("qrab")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage: qrab <COMMAND>"));
    cargo_bin_cmd!("qrab")
        .args(["check", "--version"])
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
    cargo_bin_cmd!("qrab")
        .args(["compil", "missing.qrab"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("compile"));
    cargo_bin_cmd!("qrab")
        .args(["compile", "missing.qrab", "-o", "--target"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--output <OUTPUT>"));
    cargo_bin_cmd!("qrab")
        .args(["compile", "missing.qrab", "-t", "latex", "-t", "typst"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("cannot be used multiple times"));
    cargo_bin_cmd!("qrab")
        .args([
            "compile",
            "missing.qrab",
            "-o",
            "output.tex",
            "--target",
            "all",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "cannot be used with '--target all'",
        ));

    let directory =
        std::env::temp_dir().join(format!("qrab-cli-{}-{}", std::process::id(), line!()));
    fs::create_dir(&directory).expect("create CLI test directory");
    let source = directory.join("input.qrab");
    fs::write(&source, "circuit cli { qubit q; h q }\n").expect("write CLI test source");
    cargo_bin_cmd!("qrab")
        .args(["compile", "--"])
        .arg(&source)
        .assert()
        .success();
    assert!(source.with_extension("tex").is_file());
    assert!(source.with_extension("typ").is_file());
    fs::remove_dir_all(directory).expect("remove CLI test directory");

    let missing = std::env::temp_dir().join(format!(
        "qrab-missing-source-{}-{}.qrab",
        std::process::id(),
        line!()
    ));
    cargo_bin_cmd!("qrab")
        .arg("check")
        .arg(missing)
        .assert()
        .code(1)
        .stderr(predicate::str::contains("cannot read"))
        .stderr(predicate::str::contains("Usage:").not());
}

#[test]
fn miette_reports_imported_source_help_and_multiple_errors() {
    let directory = std::env::temp_dir().join(format!(
        "qrab-diagnostics-{}-{}",
        std::process::id(),
        line!()
    ));
    fs::create_dir(&directory).expect("create diagnostic test directory");
    fs::write(
        directory.join("gates.qrab"),
        "fn broken(a) {\n\th missing\n}\n",
    )
    .expect("write imported source");
    let imported = directory.join("imported.qrab");
    fs::write(
        &imported,
        "import \"gates.qrab\"\ncircuit imported { qubit q; broken(q) }\n",
    )
    .expect("write importing source");
    cargo_bin_cmd!("qrab")
        .env("NO_COLOR", "1")
        .arg("check")
        .arg(&imported)
        .assert()
        .code(1)
        .stderr(predicate::str::contains("gates.qrab"))
        .stderr(predicate::str::contains("h missing"))
        .stderr(predicate::str::contains(
            "declare it before use or enable `autowires`",
        ));

    let multiple = directory.join("multiple.qrab");
    fs::write(
        &multiple,
        "circuit bad {\n\tqubit q\n\th missing\n\tx absent\n}\n",
    )
    .expect("write multiple-error source");
    cargo_bin_cmd!("qrab")
        .env("NO_COLOR", "1")
        .arg("check")
        .arg(&multiple)
        .assert()
        .code(1)
        .stderr(predicate::str::contains("2 errors found"))
        .stderr(predicate::str::contains("unknown wire `missing`"))
        .stderr(predicate::str::contains("unknown wire `absent`"));

    fs::remove_dir_all(directory).expect("remove diagnostic test directory");
}
