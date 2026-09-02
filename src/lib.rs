#![warn(missing_docs)]

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
    NoteSide, Operation, OperationKind, Orientation, ParityBasis, Shape, Span, Style, Wire,
    WireKind,
};
pub use loader::{LoadError, LoadedSource, load_source};
pub use parser::{Diagnostic, parse};
pub use render::{Target, render};

/// Version of this crate, as reported by `qrab --version`.
///
/// Exposed so that build tooling outside this package, which cannot see this
/// crate's `CARGO_PKG_VERSION`, can label generated artefacts with the version
/// they document rather than with its own.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Parses `.qrab` source and renders a standalone document for `target`.
pub fn compile(source: &str, target: Target) -> Result<String, Diagnostic> {
    parse(source).map(|circuit| render(&circuit, target))
}
