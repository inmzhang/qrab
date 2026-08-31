mod ast;
mod loader;
mod parser;
mod render;

pub use ast::{
    BraceSide, Circuit, Control, Group, Layout, NoteSide, Operation, OperationKind, Orientation,
    Shape, Span, Style, Wire, WireKind,
};
pub use loader::{LoadError, LoadedSource, load_source};
pub use parser::{Diagnostic, parse};
pub use render::{Target, render};

pub fn compile(source: &str, target: Target) -> Result<String, Diagnostic> {
    parse(source).map(|circuit| render(&circuit, target))
}
