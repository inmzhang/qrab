//! WebAssembly bindings for the qrab playground.
//!
//! The browser has no filesystem, so this wraps [`qrab::parse`] and
//! [`qrab::render`] directly rather than [`qrab::load_source`]: a circuit that
//! uses `import` cannot be resolved here and is reported as an ordinary
//! diagnostic.

use qrab::{Target, parse, render};
use wasm_bindgen::prelude::*;

// The menu is generated at build time from `examples/` and the ported qpic
// corpus; see `build.rs`.
include!(concat!(env!("OUT_DIR"), "/examples.rs"));

/// One compilation attempt.
///
/// Success and failure share a single shape so the caller can render either
/// without a discriminant: `message` is empty exactly when the compile
/// succeeded.
#[wasm_bindgen(getter_with_clone)]
pub struct Compiled {
    /// The rendered document, empty when the compile failed.
    pub output: String,
    /// A one-line summary of the parsed circuit, empty when it failed.
    pub summary: String,
    /// The primary diagnostic message, empty on success.
    pub message: String,
    /// Advice attached to the primary diagnostic, when it has any.
    pub help: String,
    /// Additional diagnostics recovered from the same parse, one per line.
    pub related: String,
    /// One-based line of the primary diagnostic, zero on success.
    pub line: u32,
    /// One-based column of the primary diagnostic, zero on success.
    pub column: u32,
}

/// Compiles `source` for `target`, which is one of `svg`, `latex`, or `typst`.
///
/// An unknown target falls back to SVG so a stale query string cannot leave the
/// page with nothing to show.
#[wasm_bindgen]
pub fn compile(source: &str, target: &str) -> Compiled {
    let target = match target {
        "latex" => Target::Latex,
        "typst" => Target::Typst,
        _ => Target::Svg,
    };
    match parse(source) {
        Ok(circuit) => Compiled {
            output: render(&circuit, target),
            summary: format!(
                "{}: {} wire(s), {} operation(s)",
                circuit.name,
                circuit.wires.len(),
                circuit.operations.len()
            ),
            message: String::new(),
            help: String::new(),
            related: String::new(),
            line: 0,
            column: 0,
        },
        Err(diagnostic) => Compiled {
            output: String::new(),
            summary: String::new(),
            message: diagnostic.message.clone(),
            help: diagnostic.help.clone().unwrap_or_default(),
            related: diagnostic
                .related()
                .iter()
                .map(|related| {
                    format!(
                        "{}:{}: {}",
                        related.span.line, related.span.column, related.message
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
            line: diagnostic.span.line as u32,
            column: diagnostic.span.column as u32,
        },
    }
}

/// Bundled examples in menu order, each as `"group/name"`.
///
/// The two halves are joined rather than returned separately so the caller
/// needs one call to build a grouped menu; no group or circuit name contains a
/// slash.
#[wasm_bindgen]
pub fn example_names() -> Vec<String> {
    EXAMPLES
        .iter()
        .map(|(group, name, _)| format!("{group}/{name}"))
        .collect()
}

/// Source of the example named `"group/name"`, or the first one when the name
/// is unknown, so a stale link still opens a working circuit.
#[wasm_bindgen]
pub fn example_source(name: &str) -> String {
    let (group, name) = name.split_once('/').unwrap_or(("Guide", name));
    EXAMPLES
        .iter()
        .find(|(example_group, example, _)| *example_group == group && *example == name)
        .map_or(EXAMPLES[0].2, |(_, _, source)| source)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bundled_example_compiles() {
        for name in example_names() {
            let compiled = compile(&example_source(&name), "svg");
            assert!(
                compiled.message.is_empty(),
                "{name} failed to compile: {}",
                compiled.message
            );
            assert!(
                compiled.output.starts_with("<svg"),
                "{name} rendered no SVG"
            );
        }
    }

    #[test]
    fn a_parse_error_reports_its_location() {
        let compiled = compile("circuit broken {\n  h nowhere\n}\n", "svg");
        assert_eq!(compiled.line, 2);
        assert!(!compiled.message.is_empty());
        assert!(compiled.output.is_empty());
    }
}
