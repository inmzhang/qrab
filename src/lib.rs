//! Compiler library for the human-readable `.qrab` quantum-circuit language.
//!
//! Use [`compile`] for in-memory source, or [`load_source`] before [`compile`]
//! when the source may contain relative imports. [`parse`] and [`render`] expose
//! the two compiler stages separately.

mod ast;
mod loader;
mod parser;
mod render;

pub use ast::{
    BackendEscapes, BraceSide, Circuit, Control, EscapeBlock, Group, Layout, MeasurementShape,
    NoteSide, Operation, OperationKind, Orientation, Shape, Span, Style, Wire, WireKind,
};
pub use loader::{LoadError, LoadedSource, load_source};
pub use parser::{Diagnostic, parse};
pub use render::{Target, render};

/// Parses `.qrab` source and renders a standalone document for `target`.
pub fn compile(source: &str, target: Target) -> Result<String, Diagnostic> {
    parse(source).map(|circuit| render(&circuit, target))
}
