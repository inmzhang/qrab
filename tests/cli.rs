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
