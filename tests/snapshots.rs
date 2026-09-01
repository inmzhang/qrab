use std::path::Path;

use qrab::{Target, load_source, parse, render};

const BELL: &str = r#"circuit bell {
  qubit q[2]: "|0>" -> "bell"
  h q[0]
  x q[1] if q[0]
  measure q[0], q[1]
}
"#;

const PORTABLE: &str = r#"circuit portable {
  qubit q
  space q with width: 40, height: 12
  space q
  label "stage" on q with stroke: blue, fill: yellow, shape: box, opacity: 0.5, link: "https://example.com/stage"
  end q as "done" with stroke: red, opacity: 0.25
}
"#;

const FILE_CASES: &[(&str, &str)] = &[
    ("teleportation", "examples/teleportation.qrab"),
    ("permute", "tests/qpic/permute.qrab"),
    ("styling", "examples/styling.qrab"),
    ("lifecycle", "examples/lifecycle.qrab"),
    ("annotations", "examples/annotations.qrab"),
    ("regions", "examples/regions.qrab"),
    ("measurements", "examples/measurements.qrab"),
    ("qft4vert", "tests/qpic/QFT4vert.qrab"),
    ("start_and_end", "tests/qpic/start_and_end.qrab"),
    ("shor_nutshell", "tests/qpic/ShorNutshell.qrab"),
    ("imports", "examples/imports.qrab"),
];

#[test]
fn representative_renderer_outputs() {
    assert_outputs("bell", BELL);
    assert_outputs("portable", PORTABLE);
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (name, path) in FILE_CASES {
        let source = load_source(root.join(path)).expect("load snapshot fixture");
        assert_outputs(name, source.as_str());
    }
}

fn assert_outputs(name: &str, source: &str) {
    let circuit = parse(source).expect("parse snapshot fixture");
    insta::with_settings!({
        snapshot_path => "snapshots",
        prepend_module_to_snapshot => false,
        omit_expression => true,
    }, {
        insta::assert_snapshot!(format!("{name}_latex"), render(&circuit, Target::Latex));
        insta::assert_snapshot!(format!("{name}_typst"), render(&circuit, Target::Typst));
        insta::assert_snapshot!(format!("{name}_svg"), render(&circuit, Target::Svg));
    });
}
