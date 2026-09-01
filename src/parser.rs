use std::collections::HashMap;
use std::ops::Range;

use logos::Logos;
use miette::Diagnostic as MietteDiagnostic;
use thiserror::Error;

use crate::ast::{
    BackendEscapes, BraceSide, Circuit, Control, EscapeBlock, Group, Layout, MeasurementShape,
    NoteSide, Operation, OperationKind, Orientation, Shape, Span, Style, Wire, WireKind,
};

/// A source-located lexer, parser, or semantic diagnostic.
///
/// Diagnostics are produced by [`parse`](crate::parse) and are not constructed
/// by downstream code.
#[derive(Debug, Clone, PartialEq, Eq, MietteDiagnostic, Error)]
#[error("{message}")]
#[non_exhaustive]
pub struct Diagnostic {
    /// Human-readable explanation of the error.
    pub message: String,
    /// Byte range and derived source location where the error was detected.
    pub span: Span,
    /// Actionable advice, when the error has a direct remedy.
    #[help]
    pub help: Option<String>,
    #[label]
    label: Option<miette::SourceSpan>,
    #[related]
    related: Box<[Diagnostic]>,
}

impl Diagnostic {
    fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            help: None,
            label: Some(span.into()),
            related: Box::default(),
        }
    }

    fn with_help(message: impl Into<String>, help: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            help: Some(help.into()),
            label: Some(span.into()),
            related: Box::default(),
        }
    }

    fn multiple(mut diagnostics: Vec<Self>) -> Self {
        debug_assert!(!diagnostics.is_empty());
        if diagnostics.len() == 1 {
            return diagnostics.pop().expect("one diagnostic exists");
        }
        Self {
            message: format!("{} errors found", diagnostics.len()),
            span: diagnostics[0].span,
            help: None,
            label: None,
            related: diagnostics.into_boxed_slice(),
        }
    }

    /// Returns independently located errors collected after parser recovery.
    pub fn related(&self) -> &[Self] {
        &self.related
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TokenKind {
    Identifier(String),
    String(String),
    Number(String),
    LeftBrace,
    RightBrace,
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    Colon,
    Equal,
    Comma,
    Bang,
    Arrow,
    DotDot,
    Newline,
    End,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Token {
    kind: TokenKind,
    span: Span,
}

#[derive(Logos, Debug, Clone, Copy, PartialEq, Eq)]
#[logos(skip r"[ \t\r\f]+")]
#[logos(skip(r"//[^\n]*", allow_greedy = true))]
enum Lexeme {
    #[regex(r"[A-Za-z_][A-Za-z0-9_]*")]
    Identifier,
    #[regex(r#""([^"\\\n]|\\.)*""#)]
    String,
    #[regex(r"[0-9]+(\.[0-9]+)?")]
    Number,
    #[token("{")]
    LeftBrace,
    #[token("}")]
    RightBrace,
    #[token("(")]
    LeftParen,
    #[token(")")]
    RightParen,
    #[token("[")]
    LeftBracket,
    #[token("]")]
    RightBracket,
    #[token(":")]
    Colon,
    #[token("=")]
    Equal,
    #[token(",")]
    Comma,
    #[token("!")]
    Bang,
    #[token("->")]
    Arrow,
    #[token("..")]
    DotDot,
    #[token("\n")]
    #[token(";")]
    Newline,
}

/// Parses one expanded `.qrab` source string into a checked circuit.
///
/// Use [`crate::load_source`] first when the source contains imports.
pub fn parse(source: &str) -> Result<Circuit, Diagnostic> {
    Parser::new(lex(source)?).parse_circuit()
}

fn lex(source: &str) -> Result<Vec<Token>, Diagnostic> {
    let line_starts = std::iter::once(0)
        .chain(source.match_indices('\n').map(|(offset, _)| offset + 1))
        .collect::<Vec<_>>();
    let mut lexer = Lexeme::lexer(source);
    let mut tokens = Vec::new();
    while let Some(lexeme) = lexer.next() {
        let range = lexer.span();
        let span = source_span(source, &line_starts, range.clone());
        let lexeme = lexeme.map_err(|()| lex_error(source, &line_starts, range.clone()))?;
        let kind = match lexeme {
            Lexeme::Identifier => TokenKind::Identifier(lexer.slice().into()),
            Lexeme::String => {
                TokenKind::String(decode_string(source, &line_starts, range.clone())?)
            }
            Lexeme::Number => TokenKind::Number(lexer.slice().into()),
            Lexeme::LeftBrace => TokenKind::LeftBrace,
            Lexeme::RightBrace => TokenKind::RightBrace,
            Lexeme::LeftParen => TokenKind::LeftParen,
            Lexeme::RightParen => TokenKind::RightParen,
            Lexeme::LeftBracket => TokenKind::LeftBracket,
            Lexeme::RightBracket => TokenKind::RightBracket,
            Lexeme::Colon => TokenKind::Colon,
            Lexeme::Equal => TokenKind::Equal,
            Lexeme::Comma => TokenKind::Comma,
            Lexeme::Bang => TokenKind::Bang,
            Lexeme::Arrow => TokenKind::Arrow,
            Lexeme::DotDot => TokenKind::DotDot,
            Lexeme::Newline => TokenKind::Newline,
        };
        tokens.push(Token { kind, span });
    }
    tokens.push(Token {
        kind: TokenKind::End,
        span: source_span(source, &line_starts, source.len()..source.len()),
    });
    Ok(tokens)
}

fn source_span(source: &str, line_starts: &[usize], range: Range<usize>) -> Span {
    let line_index = line_starts
        .partition_point(|start| *start <= range.start)
        .saturating_sub(1);
    Span {
        offset: range.start,
        length: range.len(),
        line: line_index + 1,
        column: source[line_starts[line_index]..range.start].chars().count() + 1,
    }
}

fn decode_string(
    source: &str,
    line_starts: &[usize],
    range: Range<usize>,
) -> Result<String, Diagnostic> {
    let mut value = String::new();
    let mut characters = source[range.start + 1..range.end - 1].char_indices();
    while let Some((_, character)) = characters.next() {
        if character != '\\' {
            value.push(character);
            continue;
        }
        let (offset, escaped) = characters
            .next()
            .expect("the string token ends after a complete escape");
        value.push(match escaped {
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            '"' => '"',
            '\\' => '\\',
            _ => {
                let start = range.start + 1 + offset;
                return Err(Diagnostic::new(
                    format!("unknown string escape `\\{escaped}`"),
                    source_span(source, line_starts, start..start + escaped.len_utf8()),
                ));
            }
        });
    }
    Ok(value)
}

fn lex_error(source: &str, line_starts: &[usize], range: Range<usize>) -> Diagnostic {
    let character = source[range.clone()]
        .chars()
        .next()
        .expect("logos errors cover at least one character");
    if character != '"' {
        return Diagnostic::new(
            format!("unexpected character `{character}`"),
            source_span(source, line_starts, range),
        );
    }

    let opening = source_span(source, line_starts, range.clone());
    let mut characters = source[range.end..].char_indices();
    while let Some((offset, character)) = characters.next() {
        if character == '\n' {
            let start = range.end + offset;
            return Diagnostic::new(
                "strings cannot cross lines",
                source_span(source, line_starts, start..start + 1),
            );
        }
        if character == '\\' {
            match characters.next() {
                Some((offset, '\n')) => {
                    let start = range.end + offset;
                    return Diagnostic::new(
                        "strings cannot cross lines",
                        source_span(source, line_starts, start..start + 1),
                    );
                }
                Some(_) => {}
                None => return Diagnostic::new("unterminated string escape", opening),
            }
        }
    }
    Diagnostic::new("unterminated string", opening)
}

fn is_reserved_statement(name: &str) -> bool {
    matches!(
        name,
        "circuit"
            | "import"
            | "fn"
            | "let"
            | "style"
            | "layout"
            | "backend"
            | "qubit"
            | "bit"
            | "hidden"
            | "ellipsis"
            | "autowires"
            | "h"
            | "x"
            | "y"
            | "z"
            | "s"
            | "t"
            | "gate"
            | "phase"
            | "measure"
            | "swap"
            | "barrier"
            | "set"
            | "start"
            | "end"
            | "label"
            | "equals"
            | "bundle"
            | "permute"
            | "space"
            | "touch"
            | "repeat"
            | "reverse"
            | "parallel"
            | "overlay"
            | "labels"
            | "brace"
            | "note"
            | "above"
            | "below"
            | "cut"
            | "mark"
            | "group"
            | "if"
            | "on"
            | "as"
            | "to"
            | "from"
            | "here"
            | "with"
            | "using"
            | "braced"
            | "latex"
            | "typst"
            | "preamble"
            | "before"
            | "after"
    )
}

fn is_style_property(name: &str) -> bool {
    matches!(
        name,
        "stroke" | "fill" | "link" | "width" | "height" | "size" | "shape" | "dash" | "opacity"
    )
}

struct Parser {
    tokens: Vec<Token>,
    position: usize,
    wires: Vec<Wire>,
    wire_indices: HashMap<String, usize>,
    operations: Vec<Operation>,
    layout: Layout,
    functions: HashMap<String, Function>,
    values: HashMap<String, String>,
    styles: HashMap<String, Style>,
    in_function: bool,
    operation_block_depth: usize,
    marks: HashMap<String, usize>,
    groups: Vec<Group>,
    escapes: BackendEscapes,
    auto_wires: bool,
    next_overlay: usize,
}

#[derive(Clone, Copy)]
struct Checkpoint {
    position: usize,
    wires: usize,
    operations: usize,
    groups: usize,
    operation_block_depth: usize,
    next_overlay: usize,
}

#[derive(Clone)]
struct Function {
    parameters: Vec<String>,
    body: Vec<Operation>,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            position: 0,
            wires: Vec::new(),
            wire_indices: HashMap::new(),
            operations: Vec::new(),
            layout: Layout::default(),
            functions: HashMap::new(),
            values: HashMap::new(),
            styles: HashMap::new(),
            in_function: false,
            operation_block_depth: 0,
            marks: HashMap::new(),
            groups: Vec::new(),
            escapes: BackendEscapes::default(),
            auto_wires: false,
            next_overlay: 0,
        }
    }

    fn parse_circuit(mut self) -> Result<Circuit, Diagnostic> {
        self.skip_newlines();
        while self.at_keyword("fn") || self.at_keyword("let") || self.at_keyword("style") {
            if self.at_keyword("fn") {
                self.parse_function_definition()?;
            } else if self.at_keyword("let") {
                self.parse_value_definition()?;
            } else {
                self.parse_style_definition()?;
            }
            self.skip_newlines();
        }
        self.expect_keyword("circuit")?;
        let name = self.take_identifier("circuit name")?;
        self.expect(TokenKind::LeftBrace, "`{`")?;
        self.skip_newlines();

        let mut diagnostics = Vec::new();
        while !self.at(&TokenKind::RightBrace) && !self.at(&TokenKind::End) {
            let checkpoint = self.checkpoint();
            if let Err(diagnostic) = self.parse_statement() {
                self.restore(checkpoint);
                self.recover_statement();
                diagnostics.push(diagnostic);
            }
            self.skip_newlines();
        }
        if self.at(&TokenKind::End) {
            diagnostics.push(self.error("expected `}` to close the circuit"));
        } else {
            self.advance();
            self.skip_newlines();
            if let Err(diagnostic) = self.expect(TokenKind::End, "end of file") {
                diagnostics.push(diagnostic);
            }
        }
        if !diagnostics.is_empty() {
            return Err(Diagnostic::multiple(diagnostics));
        }

        if self.wires.is_empty() {
            return Err(Diagnostic::new(
                "a circuit needs at least one wire",
                Span {
                    offset: 0,
                    length: 0,
                    line: 1,
                    column: 1,
                },
            ));
        }
        self.resolve_active_defaults()?;

        Ok(Circuit {
            name,
            layout: self.layout,
            wires: self.wires,
            operations: self.operations,
            groups: self.groups,
            escapes: self.escapes,
        })
    }

    fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            position: self.position,
            wires: self.wires.len(),
            operations: self.operations.len(),
            groups: self.groups.len(),
            operation_block_depth: self.operation_block_depth,
            next_overlay: self.next_overlay,
        }
    }

    fn restore(&mut self, checkpoint: Checkpoint) {
        self.position = checkpoint.position;
        self.wires.truncate(checkpoint.wires);
        self.wire_indices.retain(|_, wire| *wire < checkpoint.wires);
        self.operations.truncate(checkpoint.operations);
        self.groups.truncate(checkpoint.groups);
        self.operation_block_depth = checkpoint.operation_block_depth;
        self.next_overlay = checkpoint.next_overlay;
    }

    fn resolve_active_defaults(&mut self) -> Result<(), Diagnostic> {
        let wire_count = self.wires.len();
        let mut active = (0..wire_count)
            .map(|wire| {
                !self
                    .operations
                    .iter()
                    .find_map(|operation| {
                        if let OperationKind::Endpoint { wires, start, .. } = &operation.kind
                            && (wires.is_empty() || wires.contains(&wire))
                        {
                            Some(*start)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>();

        for operation in &mut self.operations {
            let active_wires = || {
                active
                    .iter()
                    .enumerate()
                    .filter_map(|(wire, is_active)| is_active.then_some(wire))
                    .collect::<Vec<_>>()
            };
            let inactive_wires = || {
                active
                    .iter()
                    .enumerate()
                    .filter_map(|(wire, is_active)| (!is_active).then_some(wire))
                    .collect::<Vec<_>>()
            };

            let defaulted_empty = match &mut operation.kind {
                OperationKind::Endpoint { wires, start, .. } if wires.is_empty() => {
                    *wires = if *start {
                        inactive_wires()
                    } else {
                        active_wires()
                    };
                    wires.is_empty()
                }
                OperationKind::Barrier { wires }
                | OperationKind::Label { wires, .. }
                | OperationKind::Phantom { wires }
                | OperationKind::Touch { wires }
                | OperationKind::WireLabels { wires, .. }
                | OperationKind::Brace { wires, .. }
                | OperationKind::Note { wires, .. }
                | OperationKind::Cut { wires, .. }
                    if wires.is_empty() =>
                {
                    *wires = active_wires();
                    wires.is_empty()
                }
                _ => false,
            };
            if defaulted_empty {
                return Err(Diagnostic::new(
                    "the targetless statement has no applicable active wires",
                    operation.span,
                ));
            }
            if let OperationKind::WireLabels { wires, labels } = &operation.kind
                && labels.len() != 1
                && labels.len() != wires.len()
            {
                return Err(Diagnostic::new(
                    format!(
                        "labels needs one label or one per selected wire ({}), but got {}",
                        wires.len(),
                        labels.len()
                    ),
                    operation.span,
                ));
            }
            if let OperationKind::Endpoint { wires, start, .. } = &operation.kind {
                for wire in wires {
                    active[*wire] = *start;
                }
            }
        }
        Ok(())
    }

    fn parse_value_definition(&mut self) -> Result<(), Diagnostic> {
        let span = self.current().span;
        self.expect_keyword("let")?;
        let name = self.take_identifier("value name")?;
        if is_reserved_statement(&name)
            || self.values.contains_key(&name)
            || self.styles.contains_key(&name)
            || self.functions.contains_key(&name)
        {
            return Err(Diagnostic::new(
                format!("definition name `{name}` is reserved or already used"),
                span,
            ));
        }
        self.expect(TokenKind::Equal, "`=`")?;
        let value = self.take_label("string or earlier value")?;
        self.expect_statement_end()?;
        self.values.insert(name, value);
        Ok(())
    }

    fn parse_style_definition(&mut self) -> Result<(), Diagnostic> {
        let span = self.current().span;
        self.expect_keyword("style")?;
        let name = self.take_identifier("style name")?;
        if is_reserved_statement(&name)
            || is_style_property(&name)
            || self.values.contains_key(&name)
            || self.styles.contains_key(&name)
            || self.functions.contains_key(&name)
        {
            return Err(Diagnostic::new(
                format!("definition name `{name}` is reserved or already used"),
                span,
            ));
        }
        self.expect(TokenKind::LeftBrace, "`{`")?;
        let mut style = Style::default();
        self.skip_newlines();
        while !self.at(&TokenKind::RightBrace) {
            self.parse_style_property(&mut style)?;
            self.expect_statement_end()?;
            self.skip_newlines();
        }
        self.advance();
        self.expect_statement_end()?;
        self.styles.insert(name, style);
        Ok(())
    }

    fn parse_function_definition(&mut self) -> Result<(), Diagnostic> {
        let span = self.current().span;
        self.expect_keyword("fn")?;
        let name = self.take_identifier("function name")?;
        if is_reserved_statement(&name) {
            return Err(Diagnostic::new(
                format!("function name `{name}` is reserved"),
                span,
            ));
        }
        if self.functions.contains_key(&name)
            || self.values.contains_key(&name)
            || self.styles.contains_key(&name)
        {
            return Err(Diagnostic::new(
                format!("function `{name}` is already defined"),
                span,
            ));
        }
        self.expect(TokenKind::LeftParen, "`(`")?;
        let mut parameters = Vec::new();
        if !self.at(&TokenKind::RightParen) {
            loop {
                let parameter_span = self.current().span;
                let parameter = self.take_identifier("parameter name")?;
                if parameters.contains(&parameter) {
                    return Err(Diagnostic::new(
                        format!("parameter `{parameter}` is repeated"),
                        parameter_span,
                    ));
                }
                parameters.push(parameter);
                if !self.consume(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(TokenKind::RightParen, "`)`")?;
        self.expect(TokenKind::LeftBrace, "`{`")?;

        let circuit_wires = std::mem::take(&mut self.wires);
        let circuit_indices = std::mem::take(&mut self.wire_indices);
        let circuit_operations = std::mem::take(&mut self.operations);
        self.wires = parameters
            .iter()
            .map(|parameter| Wire {
                name: parameter.clone(),
                kind: WireKind::Quantum,
                ellipsis: false,
                input: None,
                output: None,
                style: Style::default(),
            })
            .collect();
        self.wire_indices = parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| (parameter.clone(), index))
            .collect();
        self.in_function = true;
        self.skip_newlines();
        while !self.at(&TokenKind::RightBrace) {
            if self.at(&TokenKind::End) {
                return Err(self.error("expected `}` to close the function"));
            }
            self.parse_statement()?;
            self.skip_newlines();
        }
        self.advance();
        let body = std::mem::take(&mut self.operations);
        self.wires = circuit_wires;
        self.wire_indices = circuit_indices;
        self.operations = circuit_operations;
        self.in_function = false;
        self.functions.insert(name, Function { parameters, body });
        self.expect_statement_end()
    }

    fn parse_statement(&mut self) -> Result<(), Diagnostic> {
        let span = self.current().span;
        let keyword = self.take_identifier("statement")?;
        if (self.in_function || self.operation_block_depth > 0)
            && matches!(
                keyword.as_str(),
                "layout" | "backend" | "qubit" | "bit" | "hidden" | "autowires" | "mark" | "group"
            )
        {
            return Err(Diagnostic::new(
                "operation blocks may contain operations and function calls, not declarations",
                span,
            ));
        }
        match keyword.as_str() {
            "layout" => self.parse_layout(),
            "backend" => self.parse_backend(),
            "qubit" => self.parse_wire_declaration(WireKind::Quantum, false),
            "bit" => self.parse_wire_declaration(WireKind::Classical, false),
            "hidden" => self.parse_wire_declaration(WireKind::Hidden, false),
            "ellipsis" => self.parse_wire_declaration(WireKind::Hidden, true),
            "autowires" => self.parse_autowires(),
            "h" | "x" | "y" | "z" | "s" | "t" => {
                self.parse_builtin_gate(keyword.to_ascii_uppercase(), span)
            }
            "gate" => self.parse_named_gate(span),
            "phase" => self.parse_phase_gate(span),
            "measure" => self.parse_measure(span),
            "swap" => self.parse_swap(span),
            "barrier" => self.parse_barrier(span),
            "set" => self.parse_wire_change(span),
            "start" => self.parse_endpoint(span, true),
            "end" => self.parse_endpoint(span, false),
            "label" => self.parse_label(span),
            "equals" => self.parse_equals(span),
            "bundle" => self.parse_bundle(span),
            "permute" => self.parse_permute(span),
            "space" => self.parse_phantom(span),
            "touch" => self.parse_touch(span),
            "repeat" => self.parse_repeat(span),
            "reverse" => self.parse_reverse(span),
            "parallel" => self.parse_parallel(span),
            "overlay" => self.parse_overlay(span),
            "labels" => self.parse_wire_labels(span),
            "brace" => self.parse_brace(span),
            "note" => self.parse_note(span),
            "cut" => self.parse_cut(span),
            "mark" => self.parse_mark(span),
            "group" => self.parse_group(span),
            name if self.functions.contains_key(name) => self.parse_function_call(name, span),
            _ => Err(Diagnostic::new(
                format!("unknown statement `{keyword}`"),
                span,
            )),
        }
    }

    fn parse_function_call(&mut self, name: &str, span: Span) -> Result<(), Diagnostic> {
        self.expect(TokenKind::LeftParen, "`(` after the function name")?;
        let mut arguments = Vec::new();
        if !self.at(&TokenKind::RightParen) {
            loop {
                arguments.extend(self.parse_wire_selection(true)?);
                if !self.consume(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(TokenKind::RightParen, "`)`")?;
        self.expect_statement_end()?;
        let function = self
            .functions
            .get(name)
            .cloned()
            .expect("function dispatch checked the name");
        if arguments.len() != function.parameters.len() {
            return Err(Diagnostic::new(
                format!(
                    "function `{name}` expects {} wire argument(s), but got {}",
                    function.parameters.len(),
                    arguments.len()
                ),
                span,
            ));
        }
        self.ensure_unique(&arguments, span, "function argument")?;
        let start = self.operations.len();
        self.operations
            .extend(function.body.into_iter().map(|operation| Operation {
                kind: operation.kind.remap_wires(&arguments),
                span,
                style: operation.style,
                overlay: operation.overlay,
            }));
        self.freshen_overlays(start);
        Ok(())
    }

    fn parse_repeat(&mut self, span: Span) -> Result<(), Diagnostic> {
        let count = self.take_number("repeat count")?;
        let body = self.parse_operation_block("repeat")?;
        let added = body
            .len()
            .checked_mul(count)
            .ok_or_else(|| Diagnostic::new("repeat block is too large", span))?;
        self.operations
            .try_reserve(added)
            .map_err(|_| Diagnostic::new("repeat block is too large", span))?;
        for _ in 0..count {
            let start = self.operations.len();
            self.operations.extend(body.iter().cloned());
            self.freshen_overlays(start);
        }
        Ok(())
    }

    fn parse_reverse(&mut self, span: Span) -> Result<(), Diagnostic> {
        let mut body = if self.consume_keyword("from") {
            let start_span = self.current().span;
            let start_name = self.take_identifier("start mark")?;
            let start = self.marks.get(&start_name).copied().ok_or_else(|| {
                Diagnostic::new(format!("unknown mark `{start_name}`"), start_span)
            })?;
            self.expect_keyword("to")?;
            let end = if self.consume_keyword("here") {
                self.operations.len()
            } else {
                let end_span = self.current().span;
                let end_name = self.take_identifier("end mark or `here`")?;
                self.marks.get(&end_name).copied().ok_or_else(|| {
                    Diagnostic::new(format!("unknown mark `{end_name}`"), end_span)
                })?
            };
            self.expect_statement_end()?;
            if start >= end {
                return Err(Diagnostic::new(
                    "a reversed range must contain at least one operation",
                    span,
                ));
            }
            self.operations[start..end].to_vec()
        } else {
            self.parse_operation_block("reverse")?
        };
        body.reverse();
        let start = self.operations.len();
        self.operations.extend(body);
        self.freshen_overlays(start);
        Ok(())
    }

    fn parse_parallel(&mut self, span: Span) -> Result<(), Diagnostic> {
        let body = self.parse_operation_block("parallel")?;
        if body.is_empty() {
            return Ok(());
        }
        let touch = || Operation {
            kind: OperationKind::Touch { wires: Vec::new() },
            span,
            style: Style::default(),
            overlay: None,
        };
        self.operations.push(touch());
        self.operations.extend(body);
        self.operations.push(touch());
        Ok(())
    }

    fn parse_overlay(&mut self, span: Span) -> Result<(), Diagnostic> {
        let mut body = self.parse_operation_block("overlay")?;
        if body.is_empty() {
            return Ok(());
        }
        if let Some(operation) = body.iter().find(|operation| {
            matches!(
                operation.kind,
                OperationKind::Endpoint { .. } | OperationKind::Permute { .. }
            )
        }) {
            return Err(Diagnostic::new(
                "overlay blocks cannot contain lifecycle changes or permutations",
                operation.span,
            ));
        }
        let mut occupied = vec![false; self.wires.len()];
        for operation in &body {
            if matches!(operation.kind, OperationKind::Note { .. }) {
                continue;
            }
            for wire in operation
                .kind
                .occupied_wires(self.wires.len())
                .iter()
                .copied()
            {
                if std::mem::replace(&mut occupied[wire], true) {
                    return Err(Diagnostic::with_help(
                        format!(
                            "overlay operations cannot share wire `{}`",
                            self.wires[wire].name
                        ),
                        "use `parallel` to serialize operations on the same wire",
                        span,
                    ));
                }
            }
        }
        let overlay = self.next_overlay;
        self.next_overlay += 1;
        for operation in &mut body {
            operation.overlay = Some(overlay);
        }
        self.operations.extend(body);
        Ok(())
    }

    fn freshen_overlays(&mut self, start: usize) {
        let mut overlays = HashMap::new();
        for operation in &mut self.operations[start..] {
            let Some(old) = operation.overlay else {
                continue;
            };
            let next_overlay = &mut self.next_overlay;
            operation.overlay = Some(*overlays.entry(old).or_insert_with(|| {
                let overlay = *next_overlay;
                *next_overlay += 1;
                overlay
            }));
        }
    }

    fn parse_operation_block(&mut self, name: &str) -> Result<Vec<Operation>, Diagnostic> {
        self.expect(TokenKind::LeftBrace, "`{`")?;
        let start = self.operations.len();
        // The caller either succeeds or restores the whole statement from a checkpoint.
        self.operation_block_depth += 1;
        self.skip_newlines();
        while !self.at(&TokenKind::RightBrace) {
            if self.at(&TokenKind::End) {
                return Err(self.error(format!("expected `}}` to close the {name} block")));
            }
            self.parse_statement()?;
            self.skip_newlines();
        }
        self.advance();
        self.operation_block_depth -= 1;
        let body = self.operations.split_off(start);
        self.expect_statement_end()?;
        Ok(body)
    }

    fn recover_statement(&mut self) {
        let mut braces = 0_usize;
        loop {
            match self.current().kind {
                TokenKind::End => break,
                TokenKind::LeftBrace => {
                    braces += 1;
                    self.advance();
                }
                TokenKind::RightBrace if braces == 0 => break,
                TokenKind::RightBrace => {
                    braces -= 1;
                    self.advance();
                    if braces == 0 {
                        break;
                    }
                }
                TokenKind::Newline if braces == 0 => {
                    self.advance();
                    break;
                }
                _ => self.advance(),
            }
        }
    }

    fn parse_layout(&mut self) -> Result<(), Diagnostic> {
        let mut layout = self.layout.clone();
        self.expect(TokenKind::LeftBrace, "`{`")?;
        self.skip_newlines();
        while !self.at(&TokenKind::RightBrace) {
            let span = self.current().span;
            let property = self.take_identifier("layout property")?;
            self.expect(TokenKind::Colon, "`:`")?;
            match property.as_str() {
                "orientation" => {
                    let value = self.take_identifier("`horizontal` or `vertical`")?;
                    layout.orientation = match value.as_str() {
                        "horizontal" => Orientation::Horizontal,
                        "vertical" => Orientation::Vertical,
                        _ => {
                            return Err(Diagnostic::new(
                                "orientation must be `horizontal` or `vertical`",
                                span,
                            ));
                        }
                    };
                }
                "scale" => {
                    layout.scale = self.take_positive_scalar("layout scale")?;
                }
                "column_gap" => {
                    layout.column_gap = self.take_positive_scalar("column gap")?;
                }
                "wire_gap" => {
                    layout.wire_gap = self.take_positive_scalar("wire gap")?;
                }
                "gate_size" => {
                    layout.gate_size = self.take_positive_scalar("gate size")?;
                }
                "corner_radius" => {
                    layout.corner_radius = self.take_scalar("corner radius")?;
                    if layout.corner_radius < 0.0 {
                        return Err(Diagnostic::new("corner radius cannot be negative", span));
                    }
                }
                "comment_width" => {
                    layout.comment_width = self.take_positive_scalar("comment width")?;
                }
                "background" => layout.background = self.take_color("background color")?,
                _ => {
                    return Err(Diagnostic::new(
                        format!("unknown layout property `{property}`"),
                        span,
                    ));
                }
            }
            self.expect_statement_end()?;
            self.skip_newlines();
        }
        self.advance();
        self.expect_statement_end()?;
        self.layout = layout;
        Ok(())
    }

    fn parse_backend(&mut self) -> Result<(), Diagnostic> {
        let target_span = self.current().span;
        let target = self.take_identifier("`latex` or `typst`")?;
        if !matches!(target.as_str(), "latex" | "typst") {
            return Err(Diagnostic::new(
                "backend target must be `latex` or `typst`",
                target_span,
            ));
        }
        self.expect(TokenKind::LeftBrace, "`{`")?;
        let mut block = EscapeBlock::default();
        self.skip_newlines();
        while !self.at(&TokenKind::RightBrace) {
            let section_span = self.current().span;
            let section = self.take_identifier("`preamble`, `before`, or `after`")?;
            self.expect(TokenKind::Colon, "`:`")?;
            let code = self.take_string("backend code string")?;
            match section.as_str() {
                "preamble" => block.preamble.push(code),
                "before" => block.before.push(code),
                "after" => block.after.push(code),
                _ => {
                    return Err(Diagnostic::new(
                        "backend section must be `preamble`, `before`, or `after`",
                        section_span,
                    ));
                }
            }
            self.expect_statement_end()?;
            self.skip_newlines();
        }
        self.advance();
        self.expect_statement_end()?;

        let destination = if target == "latex" {
            &mut self.escapes.latex
        } else {
            &mut self.escapes.typst
        };
        destination.preamble.extend(block.preamble);
        destination.before.extend(block.before);
        destination.after.extend(block.after);
        Ok(())
    }

    fn parse_wire_declaration(&mut self, kind: WireKind, ellipsis: bool) -> Result<(), Diagnostic> {
        let span = self.current().span;
        let base = self.take_identifier("wire name")?;
        let count = if self.consume(&TokenKind::LeftBracket) {
            let count = self.take_number("wire count")?;
            self.expect(TokenKind::RightBracket, "`]`")?;
            if count == 0 {
                return Err(Diagnostic::new("wire arrays cannot be empty", span));
            }
            Some(count)
        } else {
            None
        };
        let input = if self.consume(&TokenKind::Colon) {
            Some(self.take_string("input label")?)
        } else {
            None
        };
        let output = if self.consume(&TokenKind::Arrow) {
            Some(self.take_string("output label")?)
        } else {
            None
        };
        let style = self.parse_style()?;
        self.expect_statement_end()?;

        if ellipsis && count.is_some() {
            return Err(Diagnostic::new(
                "an ellipsis declaration names one visual gap",
                span,
            ));
        }

        let names = count.map_or_else(
            || vec![base.clone()],
            |length| {
                (0..length)
                    .map(|index| format!("{base}[{index}]"))
                    .collect()
            },
        );
        for name in names {
            if self.wire_indices.contains_key(&name) {
                return Err(Diagnostic::new(
                    format!("wire `{name}` is already declared"),
                    span,
                ));
            }
            let index = self.wires.len();
            self.wire_indices.insert(name.clone(), index);
            self.wires.push(Wire {
                name,
                kind,
                ellipsis,
                input: input.clone().or_else(|| ellipsis.then(|| "...".into())),
                output: output.clone().or_else(|| ellipsis.then(|| "...".into())),
                style: style.clone(),
            });
        }
        Ok(())
    }

    fn parse_autowires(&mut self) -> Result<(), Diagnostic> {
        self.expect_statement_end()?;
        self.auto_wires = true;
        Ok(())
    }

    fn parse_builtin_gate(&mut self, label: String, span: Span) -> Result<(), Diagnostic> {
        let target = self.parse_wire_reference()?;
        let controls = self.parse_controls()?;
        let style = self.parse_style()?;
        self.expect_statement_end()?;
        self.push_gate(label, vec![target], controls, style, span)
    }

    fn parse_named_gate(&mut self, span: Span) -> Result<(), Diagnostic> {
        let label = self.take_label("gate label")?;
        self.expect_keyword("on")?;
        let targets = self.parse_wire_list()?;
        let controls = self.parse_controls()?;
        let style = self.parse_style()?;
        self.expect_statement_end()?;
        self.push_gate(label, targets, controls, style, span)
    }

    fn parse_phase_gate(&mut self, span: Span) -> Result<(), Diagnostic> {
        let phase = self.take_label("phase label")?;
        self.expect_keyword("on")?;
        let target = self.parse_wire_reference()?;
        let controls = self.parse_controls()?;
        let mut style = self.parse_style()?;
        style.shape.get_or_insert(Shape::Circle);
        self.expect_statement_end()?;
        self.push_gate(format!("P({phase})"), vec![target], controls, style, span)
    }

    fn parse_measure(&mut self, span: Span) -> Result<(), Diagnostic> {
        let targets = self.parse_wire_list()?;
        let label = if self.consume_keyword("as") {
            Some(self.take_label("measurement label")?)
        } else {
            None
        };
        let explicit_shape = self.consume_keyword("using");
        let shape = if explicit_shape {
            let shape_span = self.current().span;
            match self.take_identifier("`d` or `tag`")?.as_str() {
                "d" => MeasurementShape::D,
                "tag" => MeasurementShape::Tag,
                _ => {
                    return Err(Diagnostic::new(
                        "measurement shape must be `d` or `tag`",
                        shape_span,
                    ));
                }
            }
        } else {
            MeasurementShape::D
        };
        let style = self.parse_style()?;
        self.expect_statement_end()?;
        if label.is_none() && explicit_shape {
            return Err(Diagnostic::new(
                "a shaped measurement needs a label introduced by `as`",
                span,
            ));
        }
        if style.shape.is_some() {
            return Err(Diagnostic::with_help(
                "the generic `shape` style does not apply to measurements",
                "use `using d` or `using tag` after the measurement target",
                span,
            ));
        }
        self.ensure_unique(&targets, span, "measurement target")?;
        self.operations.push(Operation {
            kind: OperationKind::Measure {
                targets,
                label,
                shape,
            },
            span,
            style,
            overlay: None,
        });
        Ok(())
    }

    fn parse_swap(&mut self, span: Span) -> Result<(), Diagnostic> {
        let left = self.parse_wire_reference()?;
        self.expect(TokenKind::Comma, "`,`")?;
        let right = self.parse_wire_reference()?;
        let style = self.parse_style()?;
        self.expect_statement_end()?;
        if left == right {
            return Err(Diagnostic::new(
                "a wire cannot be swapped with itself",
                span,
            ));
        }
        self.operations.push(Operation {
            kind: OperationKind::Swap { left, right },
            span,
            style,
            overlay: None,
        });
        Ok(())
    }

    fn parse_barrier(&mut self, span: Span) -> Result<(), Diagnostic> {
        let wires = if self.at_statement_end() || self.at_keyword("with") {
            Vec::new()
        } else {
            self.parse_wire_list()?
        };
        let style = self.parse_style()?;
        self.expect_statement_end()?;
        self.ensure_unique(&wires, span, "barrier wire")?;
        self.operations.push(Operation {
            kind: OperationKind::Barrier { wires },
            span,
            style,
            overlay: None,
        });
        Ok(())
    }

    fn parse_wire_change(&mut self, span: Span) -> Result<(), Diagnostic> {
        let wires = self.parse_wire_list()?;
        self.expect_keyword("to")?;
        let kind = self.parse_wire_kind()?;
        let label = if self.consume_keyword("as") {
            Some(self.take_label("known wire value")?)
        } else {
            None
        };
        let style = self.parse_style()?;
        self.expect_statement_end()?;
        if label.is_some() && kind == WireKind::Classical {
            return Err(Diagnostic::new(
                "known-value markers only apply to `quantum` or `hidden` transitions",
                span,
            ));
        }
        self.ensure_unique(&wires, span, "wire")?;
        self.operations.push(Operation {
            kind: OperationKind::WireChange { wires, kind, label },
            span,
            style,
            overlay: None,
        });
        Ok(())
    }

    fn parse_endpoint(&mut self, span: Span, start: bool) -> Result<(), Diagnostic> {
        let wires = if self.at_statement_end() || self.at_keyword("as") || self.at_keyword("with") {
            Vec::new()
        } else {
            self.parse_wire_list()?
        };
        let label = if self.consume_keyword("as") {
            Some(self.take_label("endpoint label")?)
        } else {
            None
        };
        let style = self.parse_style()?;
        self.expect_statement_end()?;
        self.ensure_unique(&wires, span, "endpoint wire")?;
        self.operations.push(Operation {
            kind: OperationKind::Endpoint {
                wires,
                start,
                label,
            },
            span,
            style,
            overlay: None,
        });
        Ok(())
    }

    fn parse_label(&mut self, span: Span) -> Result<(), Diagnostic> {
        let label = self.take_label("label text")?;
        let wires = if self.consume_keyword("on") {
            self.parse_wire_list()?
        } else {
            Vec::new()
        };
        let style = self.parse_style()?;
        self.expect_statement_end()?;
        self.ensure_unique(&wires, span, "label wire")?;
        self.operations.push(Operation {
            kind: OperationKind::Label {
                wires,
                label,
                brace: None,
            },
            span,
            style,
            overlay: None,
        });
        Ok(())
    }

    fn parse_equals(&mut self, span: Span) -> Result<(), Diagnostic> {
        let label = if self.at_statement_end()
            || self.at_keyword("on")
            || self.at_keyword("braced")
            || self.at_keyword("with")
        {
            "=".into()
        } else {
            self.take_label("equals label")?
        };
        let wires = if self.consume_keyword("on") {
            self.parse_wire_list()?
        } else {
            Vec::new()
        };
        let brace = if self.consume_keyword("braced") {
            let brace_span = self.current().span;
            Some(
                match self.take_identifier("`left`, `right`, or `both`")?.as_str() {
                    "left" => BraceSide::Left,
                    "right" => BraceSide::Right,
                    "both" => BraceSide::Both,
                    _ => {
                        return Err(Diagnostic::new(
                            "brace side must be `left`, `right`, or `both`",
                            brace_span,
                        ));
                    }
                },
            )
        } else {
            None
        };
        let style = self.parse_style()?;
        self.expect_statement_end()?;
        self.ensure_unique(&wires, span, "equals wire")?;
        self.operations.push(Operation {
            kind: OperationKind::Label {
                wires,
                label,
                brace,
            },
            span,
            style,
            overlay: None,
        });
        Ok(())
    }

    fn parse_wire_labels(&mut self, span: Span) -> Result<(), Diagnostic> {
        let mut labels = vec![self.take_label("wire label")?];
        while self.consume(&TokenKind::Comma) {
            labels.push(self.take_label("wire label")?);
        }
        let wires = if self.consume_keyword("on") {
            self.parse_wire_list()?
        } else {
            Vec::new()
        };
        let style = self.parse_style()?;
        self.expect_statement_end()?;
        self.ensure_unique(&wires, span, "label wire")?;
        self.operations.push(Operation {
            kind: OperationKind::WireLabels { wires, labels },
            span,
            style,
            overlay: None,
        });
        Ok(())
    }

    fn parse_brace(&mut self, span: Span) -> Result<(), Diagnostic> {
        let side_span = self.current().span;
        let side = match self.take_identifier("`left`, `right`, or `both`")?.as_str() {
            "left" => BraceSide::Left,
            "right" => BraceSide::Right,
            "both" => BraceSide::Both,
            _ => {
                return Err(Diagnostic::new(
                    "brace side must be `left`, `right`, or `both`",
                    side_span,
                ));
            }
        };
        let label = self.take_label("brace label")?;
        let wires = if self.consume_keyword("on") {
            self.parse_wire_list()?
        } else {
            Vec::new()
        };
        let style = self.parse_style()?;
        self.expect_statement_end()?;
        self.ensure_unique(&wires, span, "brace wire")?;
        self.operations.push(Operation {
            kind: OperationKind::Brace { wires, label, side },
            span,
            style,
            overlay: None,
        });
        Ok(())
    }

    fn parse_note(&mut self, span: Span) -> Result<(), Diagnostic> {
        let side = if self.consume_keyword("below") {
            NoteSide::Below
        } else {
            self.consume_keyword("above");
            NoteSide::Above
        };
        let text = self.take_label("note text")?;
        let wires = if self.consume_keyword("on") {
            self.parse_wire_list()?
        } else {
            Vec::new()
        };
        let style = self.parse_style()?;
        self.expect_statement_end()?;
        self.ensure_unique(&wires, span, "note wire")?;
        self.operations.push(Operation {
            kind: OperationKind::Note { wires, text, side },
            span,
            style,
            overlay: None,
        });
        Ok(())
    }

    fn parse_cut(&mut self, span: Span) -> Result<(), Diagnostic> {
        let wires = if self.consume_keyword("on") {
            self.parse_wire_list()?
        } else {
            Vec::new()
        };
        let label = if self.consume_keyword("as") {
            Some(self.take_label("cut label")?)
        } else {
            None
        };
        let style = self.parse_style()?;
        self.expect_statement_end()?;
        self.ensure_unique(&wires, span, "cut wire")?;
        self.operations.push(Operation {
            kind: OperationKind::Cut { wires, label },
            span,
            style,
            overlay: None,
        });
        Ok(())
    }

    fn parse_mark(&mut self, span: Span) -> Result<(), Diagnostic> {
        let name = self.take_identifier("mark name")?;
        self.expect_statement_end()?;
        if self.marks.contains_key(&name) {
            return Err(Diagnostic::new(
                format!("mark `{name}` is already defined"),
                span,
            ));
        }
        self.marks.insert(name, self.operations.len());
        Ok(())
    }

    fn parse_group(&mut self, span: Span) -> Result<(), Diagnostic> {
        let label = self.take_label("group label")?;
        self.expect_keyword("from")?;
        let start_span = self.current().span;
        let start_name = self.take_identifier("start mark")?;
        let start =
            self.marks.get(&start_name).copied().ok_or_else(|| {
                Diagnostic::new(format!("unknown mark `{start_name}`"), start_span)
            })?;
        self.expect_keyword("to")?;
        let end = if self.consume_keyword("here") {
            self.operations.len()
        } else {
            let end_span = self.current().span;
            let end_name = self.take_identifier("end mark or `here`")?;
            self.marks
                .get(&end_name)
                .copied()
                .ok_or_else(|| Diagnostic::new(format!("unknown mark `{end_name}`"), end_span))?
        };
        let wires = if self.consume_keyword("on") {
            self.parse_wire_list()?
        } else {
            Vec::new()
        };
        let style = self.parse_style()?;
        self.expect_statement_end()?;
        self.ensure_unique(&wires, span, "group wire")?;
        if start >= end {
            return Err(Diagnostic::new(
                "a group must contain at least one operation after its start mark",
                span,
            ));
        }
        self.groups.push(Group {
            label,
            wires,
            start,
            end,
            style,
        });
        Ok(())
    }

    fn parse_bundle(&mut self, span: Span) -> Result<(), Diagnostic> {
        let label = self.take_label("bundle label")?;
        self.expect_keyword("on")?;
        let wire = self.parse_wire_reference()?;
        let style = self.parse_style()?;
        self.expect_statement_end()?;
        self.operations.push(Operation {
            kind: OperationKind::Bundle { wire, label },
            span,
            style,
            overlay: None,
        });
        Ok(())
    }

    fn parse_permute(&mut self, span: Span) -> Result<(), Diagnostic> {
        let wires = self.parse_wire_list()?;
        let style = self.parse_style()?;
        self.expect_statement_end()?;
        if wires.len() < 2 {
            return Err(Diagnostic::new("permute needs at least two wires", span));
        }
        self.ensure_unique(&wires, span, "permutation wire")?;
        self.operations.push(Operation {
            kind: OperationKind::Permute { wires },
            span,
            style,
            overlay: None,
        });
        Ok(())
    }

    fn parse_phantom(&mut self, span: Span) -> Result<(), Diagnostic> {
        let wires = self.parse_optional_wires()?;
        let style = self.parse_style()?;
        self.expect_statement_end()?;
        self.ensure_unique(&wires, span, "space wire")?;
        self.operations.push(Operation {
            kind: OperationKind::Phantom { wires },
            span,
            style,
            overlay: None,
        });
        Ok(())
    }

    fn parse_touch(&mut self, span: Span) -> Result<(), Diagnostic> {
        let wires = self.parse_optional_wires()?;
        let style = self.parse_style()?;
        self.expect_statement_end()?;
        self.ensure_unique(&wires, span, "touch wire")?;
        self.operations.push(Operation {
            kind: OperationKind::Touch { wires },
            span,
            style,
            overlay: None,
        });
        Ok(())
    }

    fn parse_optional_wires(&mut self) -> Result<Vec<usize>, Diagnostic> {
        if self.at_statement_end() || self.at_keyword("with") {
            Ok(Vec::new())
        } else {
            self.parse_wire_list()
        }
    }

    fn parse_wire_kind(&mut self) -> Result<WireKind, Diagnostic> {
        let span = self.current().span;
        let kind = self.take_identifier("wire type")?;
        match kind.as_str() {
            "quantum" | "qubit" => Ok(WireKind::Quantum),
            "classical" | "bit" => Ok(WireKind::Classical),
            "hidden" | "off" => Ok(WireKind::Hidden),
            _ => Err(Diagnostic::new(
                "wire type must be `quantum`, `classical`, or `hidden`",
                span,
            )),
        }
    }

    fn push_gate(
        &mut self,
        label: String,
        targets: Vec<usize>,
        controls: Vec<Control>,
        style: Style,
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.ensure_unique(&targets, span, "gate target")?;
        let control_wires = controls
            .iter()
            .map(|control| control.wire)
            .collect::<Vec<_>>();
        self.ensure_unique(&control_wires, span, "gate control")?;
        if let Some(wire) = targets.iter().find(|target| control_wires.contains(target)) {
            return Err(Diagnostic::new(
                format!(
                    "wire `{}` cannot be both a target and a control",
                    self.wires[*wire].name
                ),
                span,
            ));
        }
        self.operations.push(Operation {
            kind: OperationKind::Gate {
                label,
                targets,
                controls,
            },
            span,
            style,
            overlay: None,
        });
        Ok(())
    }

    fn parse_controls(&mut self) -> Result<Vec<Control>, Diagnostic> {
        if !self.consume_keyword("if") {
            return Ok(Vec::new());
        }
        let mut controls = Vec::new();
        loop {
            let positive = !self.consume(&TokenKind::Bang);
            controls.extend(
                self.parse_wire_selection(true)?
                    .into_iter()
                    .map(|wire| Control { wire, positive }),
            );
            if !self.consume(&TokenKind::Comma) {
                break;
            }
        }
        Ok(controls)
    }

    fn parse_style(&mut self) -> Result<Style, Diagnostic> {
        if !self.consume_keyword("with") {
            return Ok(Style::default());
        }

        let mut style = if let TokenKind::Identifier(name) = &self.current().kind
            && let Some(style) = self.styles.get(name).cloned()
        {
            self.advance();
            if !self.consume(&TokenKind::Comma) {
                return Ok(style);
            }
            style
        } else {
            Style::default()
        };
        loop {
            self.parse_style_property(&mut style)?;
            if !self.consume(&TokenKind::Comma) {
                break;
            }
        }
        Ok(style)
    }

    fn parse_style_property(&mut self, style: &mut Style) -> Result<(), Diagnostic> {
        let span = self.current().span;
        let property = self.take_identifier("style property")?;
        self.expect(TokenKind::Colon, "`:`")?;
        match property.as_str() {
            "stroke" => style.stroke = Some(self.take_color("stroke color")?),
            "fill" => style.fill = Some(self.take_color("fill color")?),
            "link" => style.link = Some(self.take_link()?),
            "width" => style.width = Some(self.take_positive_scalar("gate width")?),
            "height" => style.height = Some(self.take_positive_scalar("gate height")?),
            "size" => {
                let size = self.take_positive_scalar("gate size")?;
                style.width = Some(size);
                style.height = Some(size);
            }
            "shape" => {
                let value = self.take_identifier("shape")?;
                style.shape = Some(match value.as_str() {
                    "box" => Shape::Box,
                    "circle" => Shape::Circle,
                    "ellipse" => Shape::Ellipse,
                    "none" => Shape::None,
                    _ => {
                        return Err(Diagnostic::new(
                            "shape must be `box`, `circle`, `ellipse`, or `none`",
                            span,
                        ));
                    }
                });
            }
            "dash" => {
                let value = self.take_identifier("`dashed` or `solid`")?;
                style.dashed = match value.as_str() {
                    "dashed" | "true" => true,
                    "solid" | "false" => false,
                    _ => return Err(Diagnostic::new("dash must be `dashed` or `solid`", span)),
                };
            }
            "opacity" => {
                let opacity = self.take_scalar("opacity")?;
                if !(0.0..=1.0).contains(&opacity) {
                    return Err(Diagnostic::new("opacity must be between 0 and 1", span));
                }
                style.opacity = Some(opacity);
            }
            _ => {
                return Err(Diagnostic::new(
                    format!("unknown style property `{property}`"),
                    span,
                ));
            }
        }
        Ok(())
    }

    fn parse_wire_list(&mut self) -> Result<Vec<usize>, Diagnostic> {
        let mut wires = self.parse_wire_selection(true)?;
        while self.consume(&TokenKind::Comma) {
            wires.extend(self.parse_wire_selection(true)?);
        }
        Ok(wires)
    }

    fn parse_wire_reference(&mut self) -> Result<usize, Diagnostic> {
        // ponytail: reuse the checked range parser; split out a scalar path only if this
        // one-element allocation becomes measurable.
        Ok(self
            .parse_wire_selection(false)?
            .into_iter()
            .next()
            .expect("a wire selection is not empty"))
    }

    fn parse_wire_selection(&mut self, allow_range: bool) -> Result<Vec<usize>, Diagnostic> {
        let span = self.current().span;
        let base = self.take_identifier("wire name")?;
        if self.consume(&TokenKind::LeftBracket) {
            let start = self.take_number("wire index")?;
            if self.consume(&TokenKind::DotDot) {
                if !allow_range {
                    return Err(Diagnostic::new(
                        "a wire range is not valid where one wire is required",
                        span,
                    ));
                }
                let end = self.take_number("wire range end")?;
                self.expect(TokenKind::RightBracket, "`]`")?;
                if start >= end {
                    return Err(Diagnostic::new(
                        "a wire range must have an end greater than its start",
                        span,
                    ));
                }
                return (start..end)
                    .map(|index| self.resolve_wire(&format!("{base}[{index}]"), span))
                    .collect();
            }
            self.expect(TokenKind::RightBracket, "`]`")?;
            return self
                .resolve_wire(&format!("{base}[{start}]"), span)
                .map(|wire| vec![wire]);
        }
        self.resolve_wire(&base, span).map(|wire| vec![wire])
    }

    fn resolve_wire(&mut self, name: &str, span: Span) -> Result<usize, Diagnostic> {
        if let Some(wire) = self.wire_indices.get(name) {
            return Ok(*wire);
        }
        if !self.auto_wires {
            return Err(Diagnostic::with_help(
                format!("unknown wire `{name}`"),
                "declare it before use or enable `autowires`",
                span,
            ));
        }
        let wire = self.wires.len();
        self.wire_indices.insert(name.into(), wire);
        self.wires.push(Wire {
            name: name.into(),
            kind: WireKind::Quantum,
            ellipsis: false,
            input: Some(name.into()),
            output: None,
            style: Style::default(),
        });
        Ok(wire)
    }

    fn ensure_unique(
        &self,
        wires: &[usize],
        span: Span,
        description: &str,
    ) -> Result<(), Diagnostic> {
        let mut sorted = wires.to_vec();
        sorted.sort_unstable();
        if let Some(pair) = sorted.windows(2).find(|pair| pair[0] == pair[1]) {
            return Err(Diagnostic::new(
                format!("{} `{}` is repeated", description, self.wires[pair[0]].name),
                span,
            ));
        }
        Ok(())
    }

    fn take_label(&mut self, expected: &str) -> Result<String, Diagnostic> {
        match self.current().kind.clone() {
            TokenKind::String(value) => {
                self.advance();
                Ok(value)
            }
            TokenKind::Identifier(name) => {
                let value = self.values.get(&name).cloned().unwrap_or(name);
                self.advance();
                Ok(value)
            }
            _ => Err(self.error(format!("expected {expected}"))),
        }
    }

    fn take_color(&mut self, expected: &str) -> Result<String, Diagnostic> {
        let span = self.current().span;
        let color = self.take_label(expected)?;
        if color.len() == 7
            && color.starts_with('#')
            && color[1..]
                .chars()
                .all(|character| character.is_ascii_hexdigit())
        {
            return Ok(color.to_ascii_uppercase());
        }
        if [
            "black", "white", "gray", "red", "green", "blue", "teal", "purple", "orange", "yellow",
            "olive", "lime",
        ]
        .contains(&color.as_str())
        {
            Ok(color)
        } else {
            Err(Diagnostic::new(
                format!(
                    "{expected} `{color}` is not a portable named color or six-digit hex color"
                ),
                span,
            ))
        }
    }

    fn take_link(&mut self) -> Result<String, Diagnostic> {
        let span = self.current().span;
        let link = self.take_string("link URL")?;
        let valid_scheme = ["https://", "http://", "mailto:"]
            .iter()
            .any(|scheme| link.starts_with(scheme));
        let safe_characters = link.chars().all(|character| {
            character.is_ascii_alphanumeric() || ":/?#[]@!$&'()*+,-._~=%".contains(character)
        });
        if valid_scheme && safe_characters {
            Ok(link)
        } else {
            Err(Diagnostic::new(
                "link must be a safe absolute http, https, or mailto URL",
                span,
            ))
        }
    }

    fn take_identifier(&mut self, expected: &str) -> Result<String, Diagnostic> {
        if let TokenKind::Identifier(value) = &self.current().kind {
            let value = value.clone();
            self.advance();
            Ok(value)
        } else {
            Err(self.error(format!("expected {expected}")))
        }
    }

    fn take_string(&mut self, expected: &str) -> Result<String, Diagnostic> {
        if let TokenKind::String(value) = &self.current().kind {
            let value = value.clone();
            self.advance();
            Ok(value)
        } else {
            Err(self.error(format!("expected {expected} string")))
        }
    }

    fn take_number(&mut self, expected: &str) -> Result<usize, Diagnostic> {
        if let TokenKind::Number(value) = &self.current().kind {
            let value = value.parse().map_err(|_| {
                self.error(format!("expected {expected} as a non-negative integer"))
            })?;
            self.advance();
            Ok(value)
        } else {
            Err(self.error(format!("expected {expected}")))
        }
    }

    fn take_scalar(&mut self, expected: &str) -> Result<f32, Diagnostic> {
        if let TokenKind::Number(value) = &self.current().kind {
            let value = value
                .parse()
                .map_err(|_| self.error(format!("invalid {expected}")))?;
            if !f32::is_finite(value) {
                return Err(self.error(format!("invalid {expected}")));
            }
            self.advance();
            Ok(value)
        } else {
            Err(self.error(format!("expected {expected}")))
        }
    }

    fn take_positive_scalar(&mut self, expected: &str) -> Result<f32, Diagnostic> {
        let span = self.current().span;
        let value = self.take_scalar(expected)?;
        if value > 0.0 {
            Ok(value)
        } else {
            Err(Diagnostic::new(
                format!("{expected} must be greater than zero"),
                span,
            ))
        }
    }

    fn expect_keyword(&mut self, keyword: &str) -> Result<(), Diagnostic> {
        if self.consume_keyword(keyword) {
            Ok(())
        } else {
            Err(self.error(format!("expected `{keyword}`")))
        }
    }

    fn consume_keyword(&mut self, keyword: &str) -> bool {
        let matches = self.at_keyword(keyword);
        if matches {
            self.advance();
        }
        matches
    }

    fn at_keyword(&self, keyword: &str) -> bool {
        matches!(&self.current().kind, TokenKind::Identifier(value) if value == keyword)
    }

    fn expect(&mut self, kind: TokenKind, expected: &str) -> Result<(), Diagnostic> {
        if self.consume(&kind) {
            Ok(())
        } else {
            Err(self.error(format!("expected {expected}")))
        }
    }

    fn consume(&mut self, kind: &TokenKind) -> bool {
        if self.at(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect_statement_end(&mut self) -> Result<(), Diagnostic> {
        if self.at_statement_end() {
            Ok(())
        } else {
            Err(self.error("expected a newline or `;` after the statement"))
        }
    }

    fn at_statement_end(&self) -> bool {
        self.at(&TokenKind::Newline) || self.at(&TokenKind::RightBrace) || self.at(&TokenKind::End)
    }

    fn skip_newlines(&mut self) {
        while self.consume(&TokenKind::Newline) {}
    }

    fn at(&self, kind: &TokenKind) -> bool {
        self.current().kind == *kind
    }

    fn advance(&mut self) {
        // The lexer always appends End, and every parse loop checks it before advancing.
        if self.position + 1 < self.tokens.len() {
            self.position += 1;
        }
    }

    fn current(&self) -> &Token {
        &self.tokens[self.position]
    }

    fn error(&self, message: impl Into<String>) -> Diagnostic {
        Diagnostic::new(message, self.current().span)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexer_tracks_byte_ranges_and_derived_locations() {
        let tokens = lex("\tqubit q\n").expect("valid tokens");
        let span = tokens[0].span;
        assert_eq!(
            (span.offset, span.length, span.line, span.column),
            (1, 5, 1, 2)
        );

        let error = lex("circuit 💥").expect_err("non-ASCII punctuation must fail");
        assert_eq!(
            (
                error.span.offset,
                error.span.length,
                error.span.line,
                error.span.column
            ),
            (8, 4, 1, 9)
        );
    }

    #[test]
    fn parses_a_readable_bell_circuit() {
        let circuit = parse(
            r#"
                circuit bell {
                  qubit q[2]: "|0>" -> "bell"

                  h q[0]
                  x q[1] if q[0]
                  measure q[0], q[1]
                }
            "#,
        )
        .expect("valid circuit");

        assert_eq!(circuit.name, "bell");
        assert_eq!(circuit.wires.len(), 2);
        assert_eq!(circuit.operations.len(), 3);
        assert!(matches!(
            circuit.operations[1].kind,
            OperationKind::Gate {
                ref label,
                ref targets,
                ref controls,
            } if label == "X" && targets == &[1] && controls == &[Control { wire: 0, positive: true }]
        ));
    }

    #[test]
    fn reports_unknown_wires_at_the_source_location() {
        let error = parse("circuit bad {\n  qubit q\n  h missing\n}\n")
            .expect_err("unknown wire should fail");

        assert_eq!(
            (
                error.span.offset,
                error.span.length,
                error.span.line,
                error.span.column
            ),
            (28, 7, 3, 5)
        );
        assert!(error.message.contains("unknown wire `missing`"));
        assert_eq!(
            error.help.as_deref(),
            Some("declare it before use or enable `autowires`")
        );
    }

    #[test]
    fn recovers_at_statement_boundaries_to_report_multiple_errors() {
        let error = parse("circuit bad {\n  qubit q\n  h missing\n  x absent\n}\n")
            .expect_err("both unknown wires should fail");

        assert_eq!(error.message, "2 errors found");
        assert_eq!(error.related().len(), 2);
        assert!(error.related()[0].message.contains("`missing`"));
        assert!(error.related()[1].message.contains("`absent`"));
    }

    #[test]
    fn recovery_checkpoints_scale_to_generated_circuits() {
        let mut source = String::from("circuit generated {\nqubit q\n");
        for _ in 0..5_000 {
            source.push_str("h q\n");
        }
        source.push_str("}\n");

        let started = std::time::Instant::now();
        let circuit = parse(&source).expect("large generated circuit should parse");
        assert_eq!(circuit.operations.len(), 5_000);
        assert!(
            started.elapsed() < std::time::Duration::from_secs(1),
            "5,000 statements took {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn autowires_declares_quantum_wires_on_first_use() {
        let circuit = parse(
            r#"
                circuit sketch {
                  autowires
                  h control
                  x target if control
                  gate "U" on work[0..2]
                }
            "#,
        )
        .expect("valid automatic wires");

        assert_eq!(
            circuit
                .wires
                .iter()
                .map(|wire| wire.name.as_str())
                .collect::<Vec<_>>(),
            ["control", "target", "work[0]", "work[1]"]
        );
        assert!(
            circuit
                .wires
                .iter()
                .all(|wire| wire.kind == WireKind::Quantum && wire.input.is_some())
        );
    }

    #[test]
    fn parses_structured_layout_and_portable_styles() {
        let circuit = parse(
            r##"
                circuit styled {
                  layout {
                    orientation: vertical
                    scale: 1.25
                    gate_size: 24
                    corner_radius: 2
                    comment_width: 96
                    background: "#f7f8fc"
                  }
                  qubit q[2] with stroke: "#336699"
                  h q[0] with fill: yellow, shape: circle, size: 20, link: "https://example.com/gate?id=H"
                }
            "##,
        )
        .expect("valid styled circuit");

        assert_eq!(circuit.layout.orientation, Orientation::Vertical);
        assert_eq!(circuit.layout.scale, 1.25);
        assert_eq!(circuit.layout.gate_size, 24.0);
        assert_eq!(circuit.layout.corner_radius, 2.0);
        assert_eq!(circuit.layout.comment_width, 96.0);
        assert_eq!(circuit.layout.background, "#F7F8FC");
        assert_eq!(circuit.wires[0].style.stroke.as_deref(), Some("#336699"));
        assert_eq!(circuit.operations[0].style.shape, Some(Shape::Circle));
        assert_eq!(circuit.operations[0].style.width, Some(20.0));
        assert_eq!(
            circuit.operations[0].style.link.as_deref(),
            Some("https://example.com/gate?id=H")
        );

        let error = parse("circuit bad { qubit q with stroke: \"#12ZZ99\" }")
            .expect_err("invalid hex color should fail");
        assert!(error.message.contains("six-digit hex color"));
        assert!(
            parse("circuit bad { qubit q; h q with link: \"javascript:alert(1)\" }")
                .expect_err("unsafe link should fail")
                .message
                .contains("safe absolute")
        );
    }

    #[test]
    fn isolates_explicit_backend_escape_blocks() {
        let circuit = parse(
            r##"
                circuit hooks {
                  backend latex {
                    preamble: "\\newcommand{\\hook}{ok}"
                    before: "\\node {ok};"
                  }
                  backend typst {
                    after: "#v(1pt)"
                  }
                  qubit q
                }
            "##,
        )
        .expect("valid backend hooks");

        assert_eq!(circuit.escapes.latex.preamble, ["\\newcommand{\\hook}{ok}"]);
        assert_eq!(circuit.escapes.latex.before, ["\\node {ok};"]);
        assert_eq!(circuit.escapes.typst.after, ["#v(1pt)"]);
    }

    #[test]
    fn parses_wire_lifecycle_and_placement_statements() {
        let circuit = parse(
            r#"
                circuit lifecycle {
                  qubit q[3]
                  start q[2] as "aux"
                  bundle "3" on q[0]
                  label "middle" on q[0], q[2]
                  permute q[2], q[0], q[1]
                  set q[0] to classical
                  space q[1] with width: 12
                  touch q[0], q[2]
                  end q[2]
                }
            "#,
        )
        .expect("valid lifecycle circuit");

        assert_eq!(circuit.operations.len(), 8);
        assert!(matches!(
            circuit.operations[0].kind,
            OperationKind::Endpoint {
                start: true,
                ref label,
                ..
            } if label.as_deref() == Some("aux")
        ));
        assert!(matches!(
            circuit.operations[3].kind,
            OperationKind::Permute { ref wires } if wires == &[2, 0, 1]
        ));
        assert!(matches!(
            circuit.operations[4].kind,
            OperationKind::WireChange {
                kind: WireKind::Classical,
                ..
            }
        ));
        assert!(matches!(
            circuit.operations[6].kind,
            OperationKind::Touch { ref wires } if wires == &[0, 2]
        ));
    }

    #[test]
    fn targetless_statements_use_only_active_wires() {
        let circuit = parse(
            r#"
                circuit active_defaults {
                  qubit q[3]
                  touch
                  start q[2]
                  end q[0]
                  label "active"
                  end q[2]
                  barrier
                }
            "#,
        )
        .expect("valid lifecycle defaults");

        assert!(matches!(
            circuit.operations[0].kind,
            OperationKind::Touch { ref wires } if wires == &[0, 1]
        ));
        assert!(matches!(
            circuit.operations[3].kind,
            OperationKind::Label { ref wires, .. } if wires == &[1, 2]
        ));
        assert!(matches!(
            circuit.operations[5].kind,
            OperationKind::Barrier { ref wires } if wires == &[1]
        ));
    }

    #[test]
    fn targetless_statements_require_an_active_wire() {
        let error = parse("circuit inactive { qubit q[2]; end q[0]; end q[1]; barrier }")
            .expect_err("a targetless statement cannot select ended wires");

        assert!(error.message.contains("no applicable active wires"));
    }

    #[test]
    fn wire_label_arity_is_checked_after_autowires_finish() {
        let error = parse("circuit labels { autowires; h a; h b; labels \"p\", \"q\"; h c }")
            .expect_err("late automatic wires must be included in label arity");

        assert!(error.message.contains("selected wire (3)"));
    }

    #[test]
    fn rejects_non_finite_scalars() {
        let error = parse(
            "circuit huge { layout { scale: 99999999999999999999999999999999999999999999999999 }; qubit q }",
        )
        .expect_err("an overflowing scalar must not render as infinity");

        assert!(error.message.contains("invalid layout scale"));
    }

    #[test]
    fn parses_named_measurement_shapes() {
        let circuit = parse(
            r#"
                circuit measurements {
                  qubit q[3]
                  measure q[0]
                  measure q[1] as "Z"
                  measure q[2] as "X" using tag
                }
            "#,
        )
        .expect("valid measurement shapes");

        assert!(matches!(
            circuit.operations[1].kind,
            OperationKind::Measure {
                shape: MeasurementShape::D,
                ..
            }
        ));
        assert!(matches!(
            circuit.operations[2].kind,
            OperationKind::Measure {
                shape: MeasurementShape::Tag,
                ..
            }
        ));
        assert!(
            parse("circuit bad { qubit q; measure q using tag }")
                .expect_err("tag needs a label")
                .message
                .contains("needs a label")
        );
    }

    #[test]
    fn parses_known_value_wire_transitions() {
        let circuit = parse(
            r#"
                circuit values {
                  qubit q
                  set q to hidden as "0"
                  set q to quantum as "1"
                }
            "#,
        )
        .expect("valid known-value transitions");

        assert!(matches!(
            circuit.operations[0].kind,
            OperationKind::WireChange {
                kind: WireKind::Hidden,
                label: Some(ref label),
                ..
            } if label == "0"
        ));
        assert!(
            parse("circuit bad { bit c; set c to classical as \"0\" }")
                .expect_err("classical transition cannot carry a value marker")
                .message
                .contains("quantum` or `hidden")
        );
    }

    #[test]
    fn parses_an_ellipsis_wire_as_a_visual_gap() {
        let circuit = parse(
            "circuit gap { qubit first; ellipsis omitted; qubit last; label \"...\" on omitted }",
        )
        .expect("valid ellipsis wire");

        assert_eq!(circuit.wires.len(), 3);
        assert!(circuit.wires[1].ellipsis);
        assert_eq!(circuit.wires[1].kind, WireKind::Hidden);
        assert_eq!(circuit.wires[1].input.as_deref(), Some("..."));
        assert_eq!(circuit.wires[1].output.as_deref(), Some("..."));
        assert!(
            parse("circuit bad { ellipsis gap[2] }")
                .expect_err("ellipsis arrays are ambiguous")
                .message
                .contains("one visual gap")
        );
    }

    #[test]
    fn parses_and_lowers_typed_function_calls() {
        let circuit = parse(
            r#"
                fn entangle(control, target) {
                  h control
                  x target if control
                  gate "control" on target
                }

                fn ghz(a, b, c) {
                  entangle(a, b)
                  x c if b
                  barrier
                }

                circuit functions {
                  qubit q[3]
                  ghz(q[2], q[0], q[1])
                }
            "#,
        )
        .expect("valid functions");

        assert_eq!(circuit.operations.len(), 5);
        assert!(matches!(
            circuit.operations[1].kind,
            OperationKind::Gate {
                ref label,
                ref targets,
                ref controls,
            } if label == "X"
                && targets == &[0]
                && controls == &[Control { wire: 2, positive: true }]
        ));
        assert!(matches!(
            circuit.operations[2].kind,
            OperationKind::Gate { ref label, .. } if label == "control"
        ));
        assert!(matches!(
            circuit.operations[4].kind,
            OperationKind::Barrier { ref wires } if wires == &[2, 0, 1]
        ));
    }

    #[test]
    fn resolves_typed_values_and_named_styles() {
        let circuit = parse(
            r##"
                let operator = "U"
                let accent = "#ddeeff"

                style highlighted {
                  fill: accent
                  stroke: blue
                  shape: circle
                }

                circuit declarations {
                  qubit q
                  gate operator on q with highlighted, opacity: 0.5
                }
            "##,
        )
        .expect("valid typed declarations");

        assert!(matches!(
            circuit.operations[0].kind,
            OperationKind::Gate { ref label, .. } if label == "U"
        ));
        assert_eq!(circuit.operations[0].style.fill.as_deref(), Some("#DDEEFF"));
        assert_eq!(circuit.operations[0].style.shape, Some(Shape::Circle));
        assert_eq!(circuit.operations[0].style.opacity, Some(0.5));
        assert!(
            parse("let h = \"bad\"; circuit bad { qubit q }")
                .expect_err("reserved definition name should fail")
                .message
                .contains("reserved or already used")
        );
    }

    #[test]
    fn reports_function_arity_at_the_call() {
        let error =
            parse("fn one(a) { h a }\n\ncircuit bad {\n  qubit q[2]\n  one(q[0], q[1])\n}\n")
                .expect_err("wrong arity should fail");

        assert_eq!((error.span.line, error.span.column), (5, 3));
        assert!(
            error
                .message
                .contains("expects 1 wire argument(s), but got 2")
        );
    }

    #[test]
    fn expands_ranges_and_structured_operation_blocks() {
        let circuit = parse(
            r#"
                fn pair(a, b) {
                  parallel {
                    h a
                    h b
                  }
                }

                circuit structured {
                  qubit q[4]
                  pair(q[1..3])
                  repeat 2 {
                    measure q[0..2]
                  }
                }
            "#,
        )
        .expect("valid structured blocks");

        assert_eq!(circuit.operations.len(), 6);
        assert!(matches!(
            circuit.operations[0].kind,
            OperationKind::Touch { ref wires } if wires == &[1, 2]
        ));
        assert!(matches!(
            circuit.operations[4].kind,
            OperationKind::Measure { ref targets, .. } if targets == &[0, 1]
        ));
        assert!(matches!(
            circuit.operations[5].kind,
            OperationKind::Measure { ref targets, .. } if targets == &[0, 1]
        ));
    }

    #[test]
    fn reverses_blocks_and_marked_ranges() {
        let circuit = parse(
            r#"
                circuit reverse_order {
                  qubit q[3]
                  mark forward
                  reverse {
                    h q[0]
                    measure q[1]
                    z q[2]
                  }
                  mark reversed
                  reverse from forward to reversed
                }
            "#,
        )
        .expect("valid reverse block");

        let kinds = circuit
            .operations
            .iter()
            .map(|operation| match &operation.kind {
                OperationKind::Gate { label, .. } => label.as_str(),
                OperationKind::Measure { .. } => "M",
                _ => panic!("unexpected reverse fixture operation"),
            })
            .collect::<Vec<_>>();
        assert_eq!(kinds, ["Z", "M", "H", "H", "M", "Z"]);
    }

    #[test]
    fn rejects_a_range_where_one_wire_is_required() {
        let error = parse("circuit bad {\n  qubit q[2]\n  h q[0..2]\n}\n")
            .expect_err("range target should fail");

        assert!(error.message.contains("where one wire is required"));
    }

    #[test]
    fn parses_labels_braces_notes_and_cuts() {
        let circuit = parse(
            r#"
                circuit annotations {
                  qubit q[3]
                  labels "a", "b", "c" on q[0..3]
                  brace both "register" on q[0..3]
                  note below "decode" on q[1]
                  cut on q[0..2] as "stage" with stroke: blue
                }
            "#,
        )
        .expect("valid annotations");

        assert!(matches!(
            circuit.operations[0].kind,
            OperationKind::WireLabels { ref wires, ref labels }
                if wires == &[0, 1, 2] && labels == &["a", "b", "c"]
        ));
        assert!(matches!(
            circuit.operations[1].kind,
            OperationKind::Brace {
                side: BraceSide::Both,
                ..
            }
        ));
        assert!(matches!(
            circuit.operations[2].kind,
            OperationKind::Note {
                side: NoteSide::Below,
                ..
            }
        ));
        assert!(matches!(
            circuit.operations[3].kind,
            OperationKind::Cut { ref wires, ref label }
                if wires == &[0, 1] && label.as_deref() == Some("stage")
        ));
    }

    #[test]
    fn parses_centered_equals_with_optional_braces() {
        let circuit = parse(
            r#"
                circuit equals {
                  qubit q[2]
                  equals on q[0..2]
                  equals "stage" on q[0..2] braced both with stroke: blue
                }
            "#,
        )
        .expect("valid equals statements");

        assert!(matches!(
            circuit.operations[0].kind,
            OperationKind::Label {
                ref label,
                brace: None,
                ..
            } if label == "="
        ));
        assert!(matches!(
            circuit.operations[1].kind,
            OperationKind::Label {
                ref label,
                brace: Some(BraceSide::Both),
                ..
            } if label == "stage"
        ));
    }

    #[test]
    fn resolves_marked_group_ranges() {
        let circuit = parse(
            r#"
                circuit regions {
                  qubit q[2]
                  mark start
                  h q[0]
                  mark middle
                  x q[1]
                  group "first" from start to middle on q[0]
                  group "all" from start to here with fill: yellow, opacity: 0.2
                }
            "#,
        )
        .expect("valid groups");

        assert_eq!(circuit.groups.len(), 2);
        assert_eq!((circuit.groups[0].start, circuit.groups[0].end), (0, 1));
        assert_eq!((circuit.groups[1].start, circuit.groups[1].end), (0, 2));
        assert_eq!(circuit.groups[0].wires, [0]);
        assert_eq!(circuit.groups[1].style.opacity, Some(0.2));
    }
}
