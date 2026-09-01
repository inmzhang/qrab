//! The SVG backend has no external renderer to catch its mistakes, so this
//! suite compiles every tracked fixture and checks two properties that the
//! snapshot tests cannot: the document is well-formed XML, and every primitive
//! it draws lies inside the declared `viewBox`.

use std::fs;
use std::path::{Path, PathBuf};

use qrab::{Target, load_source, parse, render};
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;

#[test]
fn every_fixture_renders_well_formed_bounded_svg() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixtures = collect_fixtures(root);
    assert!(
        fixtures.len() > 100,
        "expected the full fixture corpus, found {}",
        fixtures.len()
    );
    for fixture in fixtures {
        let source = load_source(&fixture)
            .unwrap_or_else(|error| panic!("load {}: {error}", fixture.display()));
        let circuit = parse(source.as_str())
            .unwrap_or_else(|error| panic!("parse {}: {error}", fixture.display()));
        let document = render(&circuit, Target::Svg);
        check_document(&document, &fixture);
    }
}

/// Circuits with no operations still have to produce a valid document rather
/// than an inverted or zero-area `viewBox`.
#[test]
fn an_empty_circuit_still_renders() {
    let circuit = parse("circuit empty {\n  qubit q\n}\n").expect("parse empty circuit");
    let document = render(&circuit, Target::Svg);
    check_document(&document, Path::new("<empty>"));
}

fn collect_fixtures(root: &Path) -> Vec<PathBuf> {
    let mut fixtures = Vec::new();
    for directory in ["examples", "tests/qpic", "tests/qpic-manual"] {
        let entries = fs::read_dir(root.join(directory)).expect("read fixture directory");
        for entry in entries {
            let path = entry.expect("read fixture entry").path();
            if path
                .extension()
                .is_some_and(|extension| extension == "qrab")
            {
                fixtures.push(path);
            }
        }
    }
    fixtures.sort();
    fixtures
}

fn check_document(document: &str, fixture: &Path) {
    let mut reader = Reader::from_str(document);
    reader.config_mut().check_end_names = true;
    let mut view_box = None;
    let mut rotated = false;
    let mut depth = 0_i32;
    loop {
        // `read_event` with attribute checks enabled is what makes a repeated
        // attribute — the easiest mistake to make when concatenating attribute
        // strings by hand — a test failure instead of a silent parse error in
        // whatever renderer the user reaches for.
        match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(Event::Start(element)) => {
                depth += 1;
                // Vertical circuits are drawn horizontally inside a rotated
                // group, so their primitives are only inside the viewBox after
                // the same rotation the renderer applies.
                if element.name().as_ref() == "g" {
                    rotated = attribute(&element, "transform")
                        .is_some_and(|value| value.contains("rotate(-90)"));
                }
                inspect(&element, fixture, &mut view_box, rotated);
            }
            Ok(Event::Empty(element)) => inspect(&element, fixture, &mut view_box, rotated),
            Ok(Event::End(_)) => depth -= 1,
            Ok(_) => {}
            Err(error) => panic!("{} is not well-formed XML: {error}", fixture.display()),
        }
    }
    assert_eq!(depth, 0, "{} has unbalanced elements", fixture.display());
    assert!(
        view_box.is_some(),
        "{} declares no viewBox",
        fixture.display()
    );
}

type ViewBox = (f32, f32, f32, f32);

fn attribute(element: &BytesStart<'_>, wanted: &str) -> Option<String> {
    element.attributes().flatten().find_map(|attribute| {
        (attribute.key.as_ref() == wanted).then(|| attribute.value.as_ref().to_string())
    })
}

fn inspect(
    element: &BytesStart<'_>,
    fixture: &Path,
    view_box: &mut Option<ViewBox>,
    rotated: bool,
) {
    let name = element.name();
    let name = name.as_ref().to_string();
    let mut attributes = Vec::new();
    for attribute in element.attributes().with_checks(true) {
        let attribute = attribute.unwrap_or_else(|error| {
            panic!(
                "{} has a bad <{name}> attribute: {error}",
                fixture.display()
            )
        });
        let key = attribute.key.as_ref().to_string();
        let value = attribute.value.as_ref().to_string();
        attributes.push((key, value));
    }

    if name == "svg" {
        let raw = attributes
            .iter()
            .find(|(key, _)| key == "viewBox")
            .map(|(_, value)| value.clone())
            .unwrap_or_else(|| panic!("{} has no viewBox", fixture.display()));
        let numbers = raw
            .split_whitespace()
            .map(|part| part.parse::<f32>().expect("numeric viewBox component"))
            .collect::<Vec<_>>();
        assert_eq!(
            numbers.len(),
            4,
            "{} has a short viewBox",
            fixture.display()
        );
        assert!(
            numbers[2] > 0.0 && numbers[3] > 0.0,
            "{} has a degenerate viewBox {raw}",
            fixture.display()
        );
        *view_box = Some((numbers[0], numbers[1], numbers[2], numbers[3]));
        return;
    }

    let Some((left, top, width, height)) = *view_box else {
        return;
    };
    let get = |wanted: &str| -> Option<f32> {
        attributes
            .iter()
            .find(|(key, _)| key == wanted)
            .and_then(|(_, value)| value.parse::<f32>().ok())
    };
    // Text is placed from an estimated advance width, and paths are already
    // covered by the primitives that bound them, so only the shapes with exact
    // extents are checked.
    let extents: Vec<(f32, f32)> = match name.as_str() {
        "line" => vec![
            (get("x1").unwrap_or(left), get("y1").unwrap_or(top)),
            (get("x2").unwrap_or(left), get("y2").unwrap_or(top)),
        ],
        "rect" => {
            let (x, y) = (get("x").unwrap_or(left), get("y").unwrap_or(top));
            vec![
                (x, y),
                (
                    x + get("width").unwrap_or(0.0),
                    y + get("height").unwrap_or(0.0),
                ),
            ]
        }
        "circle" => {
            let (cx, cy, r) = (
                get("cx").unwrap_or(left),
                get("cy").unwrap_or(top),
                get("r").unwrap_or(0.0),
            );
            vec![(cx - r, cy - r), (cx + r, cy + r)]
        }
        "ellipse" => {
            let (cx, cy) = (get("cx").unwrap_or(left), get("cy").unwrap_or(top));
            let (rx, ry) = (get("rx").unwrap_or(0.0), get("ry").unwrap_or(0.0));
            vec![(cx - rx, cy - ry), (cx + rx, cy + ry)]
        }
        _ => Vec::new(),
    };
    for (x, y) in extents {
        let (x, y) = if rotated { (y, -x) } else { (x, y) };
        assert!(
            x >= left - 0.01
                && x <= left + width + 0.01
                && y >= top - 0.01
                && y <= top + height + 0.01,
            "{}: <{name}> point ({x:.3}, {y:.3}) falls outside viewBox ({left:.3} {top:.3} {width:.3} {height:.3})",
            fixture.display()
        );
    }
}
