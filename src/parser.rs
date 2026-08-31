use std::collections::HashMap;
use std::error::Error;
use std::fmt;

use crate::ast::{
    Circuit, Control, Layout, Operation, OperationKind, Orientation, Shape, Span, Style, Wire,
    WireKind,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
    pub span: Span,
}

impl Diagnostic {
    fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{}: {}",
            self.span.line, self.span.column, self.message
        )
    }
}

impl Error for Diagnostic {}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TokenKind {
    Identifier(String),
    String(String),
    Number(String),
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Colon,
    Comma,
    Bang,
    Arrow,
    Newline,
    End,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Token {
    kind: TokenKind,
    span: Span,
}

pub fn parse(source: &str) -> Result<Circuit, Diagnostic> {
    Parser::new(lex(source)?).parse_circuit()
}

fn lex(source: &str) -> Result<Vec<Token>, Diagnostic> {
    let mut tokens = Vec::new();
    let mut chars = source.chars().peekable();
    let (mut line, mut column) = (1, 1);

    while let Some(character) = chars.next() {
        let span = Span { line, column };
        match character {
            ' ' | '\t' | '\r' => column += 1,
            '\n' | ';' => {
                tokens.push(Token {
                    kind: TokenKind::Newline,
                    span,
                });
                if character == '\n' {
                    line += 1;
                    column = 1;
                } else {
                    column += 1;
                }
            }
            '/' if chars.peek() == Some(&'/') => {
                chars.next();
                column += 2;
                while chars.peek().is_some_and(|next| *next != '\n') {
                    chars.next();
                    column += 1;
                }
            }
            '{' => push_token(&mut tokens, TokenKind::LeftBrace, span, &mut column),
            '}' => push_token(&mut tokens, TokenKind::RightBrace, span, &mut column),
            '[' => push_token(&mut tokens, TokenKind::LeftBracket, span, &mut column),
            ']' => push_token(&mut tokens, TokenKind::RightBracket, span, &mut column),
            ':' => push_token(&mut tokens, TokenKind::Colon, span, &mut column),
            ',' => push_token(&mut tokens, TokenKind::Comma, span, &mut column),
            '!' => push_token(&mut tokens, TokenKind::Bang, span, &mut column),
            '-' if chars.peek() == Some(&'>') => {
                chars.next();
                tokens.push(Token {
                    kind: TokenKind::Arrow,
                    span,
                });
                column += 2;
            }
            '"' => {
                column += 1;
                let mut value = String::new();
                let mut terminated = false;
                while let Some(next) = chars.next() {
                    column += 1;
                    match next {
                        '"' => {
                            terminated = true;
                            break;
                        }
                        '\\' => {
                            let escaped = chars.next().ok_or_else(|| {
                                Diagnostic::new("unterminated string escape", span)
                            })?;
                            column += 1;
                            value.push(match escaped {
                                'n' => '\n',
                                'r' => '\r',
                                't' => '\t',
                                '"' => '"',
                                '\\' => '\\',
                                _ => {
                                    return Err(Diagnostic::new(
                                        format!("unknown string escape `\\{escaped}`"),
                                        Span {
                                            line,
                                            column: column - 1,
                                        },
                                    ));
                                }
                            });
                        }
                        '\n' => {
                            return Err(Diagnostic::new(
                                "strings cannot cross lines",
                                Span {
                                    line,
                                    column: column - 1,
                                },
                            ));
                        }
                        _ => value.push(next),
                    }
                }
                if !terminated {
                    return Err(Diagnostic::new("unterminated string", span));
                }
                tokens.push(Token {
                    kind: TokenKind::String(value),
                    span,
                });
            }
            first if first.is_ascii_alphabetic() || first == '_' => {
                let mut value = String::from(first);
                column += 1;
                while chars
                    .peek()
                    .is_some_and(|next| next.is_ascii_alphanumeric() || *next == '_')
                {
                    value.push(chars.next().expect("peeked character exists"));
                    column += 1;
                }
                tokens.push(Token {
                    kind: TokenKind::Identifier(value),
                    span,
                });
            }
            first if first.is_ascii_digit() => {
                let mut value = String::from(first);
                column += 1;
                while chars.peek().is_some_and(char::is_ascii_digit) {
                    value.push(chars.next().expect("peeked character exists"));
                    column += 1;
                }
                if chars.peek() == Some(&'.') {
                    chars.next();
                    value.push('.');
                    column += 1;
                    if !chars.peek().is_some_and(char::is_ascii_digit) {
                        return Err(Diagnostic::new(
                            "a decimal point must be followed by a digit",
                            span,
                        ));
                    }
                    while chars.peek().is_some_and(char::is_ascii_digit) {
                        value.push(chars.next().expect("peeked character exists"));
                        column += 1;
                    }
                }
                tokens.push(Token {
                    kind: TokenKind::Number(value),
                    span,
                });
            }
            unexpected => {
                return Err(Diagnostic::new(
                    format!("unexpected character `{unexpected}`"),
                    span,
                ));
            }
        }
    }
    tokens.push(Token {
        kind: TokenKind::End,
        span: Span { line, column },
    });
    Ok(tokens)
}

fn push_token(tokens: &mut Vec<Token>, kind: TokenKind, span: Span, column: &mut usize) {
    tokens.push(Token { kind, span });
    *column += 1;
}

struct Parser {
    tokens: Vec<Token>,
    position: usize,
    wires: Vec<Wire>,
    wire_indices: HashMap<String, usize>,
    operations: Vec<Operation>,
    layout: Layout,
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
        }
    }

    fn parse_circuit(mut self) -> Result<Circuit, Diagnostic> {
        self.skip_newlines();
        self.expect_keyword("circuit")?;
        let name = self.take_identifier("circuit name")?;
        self.expect(TokenKind::LeftBrace, "`{`")?;
        self.skip_newlines();

        while !self.at(&TokenKind::RightBrace) {
            if self.at(&TokenKind::End) {
                return Err(self.error("expected `}` to close the circuit"));
            }
            self.parse_statement()?;
            self.skip_newlines();
        }
        self.advance();
        self.skip_newlines();
        self.expect(TokenKind::End, "end of file")?;

        if self.wires.is_empty() {
            return Err(Diagnostic::new(
                "a circuit needs at least one wire",
                Span { line: 1, column: 1 },
            ));
        }

        Ok(Circuit {
            name,
            layout: self.layout,
            wires: self.wires,
            operations: self.operations,
        })
    }

    fn parse_statement(&mut self) -> Result<(), Diagnostic> {
        let span = self.current().span;
        let keyword = self.take_identifier("statement")?;
        match keyword.as_str() {
            "layout" => self.parse_layout(),
            "qubit" => self.parse_wire_declaration(WireKind::Quantum),
            "bit" => self.parse_wire_declaration(WireKind::Classical),
            "hidden" => self.parse_wire_declaration(WireKind::Hidden),
            "h" | "x" | "y" | "z" | "s" | "t" => {
                self.parse_builtin_gate(keyword.to_ascii_uppercase(), span)
            }
            "gate" => self.parse_named_gate(span),
            "phase" => self.parse_phase_gate(span),
            "measure" => self.parse_measure(span),
            "swap" => self.parse_swap(span),
            "barrier" => self.parse_barrier(span),
            _ => Err(Diagnostic::new(
                format!("unknown statement `{keyword}`"),
                span,
            )),
        }
    }

    fn parse_layout(&mut self) -> Result<(), Diagnostic> {
        self.expect(TokenKind::LeftBrace, "`{`")?;
        self.skip_newlines();
        while !self.at(&TokenKind::RightBrace) {
            let span = self.current().span;
            let property = self.take_identifier("layout property")?;
            self.expect(TokenKind::Colon, "`:`")?;
            match property.as_str() {
                "orientation" => {
                    let value = self.take_identifier("`horizontal` or `vertical`")?;
                    self.layout.orientation = match value.as_str() {
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
                    self.layout.scale = self.take_positive_scalar("layout scale")?;
                }
                "column_gap" => {
                    self.layout.column_gap = self.take_positive_scalar("column gap")?;
                }
                "wire_gap" => {
                    self.layout.wire_gap = self.take_positive_scalar("wire gap")?;
                }
                "background" => self.layout.background = self.take_color("background color")?,
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
        self.expect_statement_end()
    }

    fn parse_wire_declaration(&mut self, kind: WireKind) -> Result<(), Diagnostic> {
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
                input: input.clone(),
                output: output.clone(),
                style: style.clone(),
            });
        }
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
        let style = self.parse_style()?;
        self.expect_statement_end()?;
        self.ensure_unique(&targets, span, "measurement target")?;
        self.operations.push(Operation {
            kind: OperationKind::Measure { targets, label },
            span,
            style,
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
        });
        Ok(())
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
            controls.push(Control {
                wire: self.parse_wire_reference()?,
                positive,
            });
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

        let mut style = Style::default();
        loop {
            let span = self.current().span;
            let property = self.take_identifier("style property")?;
            self.expect(TokenKind::Colon, "`:`")?;
            match property.as_str() {
                "stroke" => style.stroke = Some(self.take_color("stroke color")?),
                "fill" => style.fill = Some(self.take_color("fill color")?),
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
                        _ => {
                            return Err(Diagnostic::new("dash must be `dashed` or `solid`", span));
                        }
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
            if !self.consume(&TokenKind::Comma) {
                break;
            }
        }
        Ok(style)
    }

    fn parse_wire_list(&mut self) -> Result<Vec<usize>, Diagnostic> {
        let mut wires = vec![self.parse_wire_reference()?];
        while self.consume(&TokenKind::Comma) {
            wires.push(self.parse_wire_reference()?);
        }
        Ok(wires)
    }

    fn parse_wire_reference(&mut self) -> Result<usize, Diagnostic> {
        let span = self.current().span;
        let mut name = self.take_identifier("wire name")?;
        if self.consume(&TokenKind::LeftBracket) {
            let index = self.take_number("wire index")?;
            self.expect(TokenKind::RightBracket, "`]`")?;
            name = format!("{name}[{index}]");
        }
        self.wire_indices.get(&name).copied().ok_or_else(|| {
            Diagnostic::new(
                format!("unknown wire `{name}`; declare it before use"),
                span,
            )
        })
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
        match &self.current().kind {
            TokenKind::String(value) | TokenKind::Identifier(value) => {
                let value = value.clone();
                self.advance();
                Ok(value)
            }
            _ => Err(self.error(format!("expected {expected}"))),
        }
    }

    fn take_color(&mut self, expected: &str) -> Result<String, Diagnostic> {
        let span = self.current().span;
        let color = self.take_label(expected)?;
        if [
            "black", "white", "gray", "red", "green", "blue", "teal", "purple", "orange", "yellow",
            "olive", "lime",
        ]
        .contains(&color.as_str())
        {
            Ok(color)
        } else {
            Err(Diagnostic::new(
                format!("{expected} `{color}` is not a portable named color"),
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

        assert_eq!(error.span, Span { line: 3, column: 5 });
        assert!(error.message.contains("unknown wire `missing`"));
    }

    #[test]
    fn parses_structured_layout_and_portable_styles() {
        let circuit = parse(
            r#"
                circuit styled {
                  layout {
                    orientation: vertical
                    scale: 1.25
                    background: white
                  }
                  qubit q[2] with stroke: blue
                  h q[0] with fill: yellow, shape: circle, size: 20
                }
            "#,
        )
        .expect("valid styled circuit");

        assert_eq!(circuit.layout.orientation, Orientation::Vertical);
        assert_eq!(circuit.layout.scale, 1.25);
        assert_eq!(circuit.wires[0].style.stroke.as_deref(), Some("blue"));
        assert_eq!(circuit.operations[0].style.shape, Some(Shape::Circle));
        assert_eq!(circuit.operations[0].style.width, Some(20.0));
    }
}
