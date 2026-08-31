mod ast;
mod parser;
mod render;

pub use ast::{Circuit, Control, Operation, OperationKind, Span, Wire, WireKind};
pub use parser::{Diagnostic, parse};
pub use render::{Target, render};

pub fn compile(source: &str, target: Target) -> Result<String, Diagnostic> {
    parse(source).map(|circuit| render(&circuit, target))
}
