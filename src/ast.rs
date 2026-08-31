#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Circuit {
    pub name: String,
    pub layout: Layout,
    pub wires: Vec<Wire>,
    pub operations: Vec<Operation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub orientation: Orientation,
    pub scale: f32,
    pub column_gap: f32,
    pub wire_gap: f32,
    pub background: String,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            orientation: Orientation::Horizontal,
            scale: 1.0,
            column_gap: 1.5,
            wire_gap: 1.0,
            background: "white".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireKind {
    Quantum,
    Classical,
    Hidden,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Wire {
    pub name: String,
    pub kind: WireKind,
    pub input: Option<String>,
    pub output: Option<String>,
    pub style: Style,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Operation {
    pub kind: OperationKind,
    pub span: Span,
    pub style: Style,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Style {
    pub stroke: Option<String>,
    pub fill: Option<String>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub shape: Option<Shape>,
    pub dashed: bool,
    pub opacity: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Box,
    Circle,
    Ellipse,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationKind {
    Gate {
        label: String,
        targets: Vec<usize>,
        controls: Vec<Control>,
    },
    Measure {
        targets: Vec<usize>,
        label: Option<String>,
    },
    Swap {
        left: usize,
        right: usize,
    },
    Barrier {
        wires: Vec<usize>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Control {
    pub wire: usize,
    pub positive: bool,
}

impl OperationKind {
    pub(crate) fn occupied_interval(&self, wire_count: usize) -> (usize, usize) {
        let mut occupied = match self {
            Self::Gate {
                targets, controls, ..
            } => targets
                .iter()
                .copied()
                .chain(controls.iter().map(|control| control.wire))
                .collect::<Vec<_>>(),
            Self::Measure { targets, .. } => targets.clone(),
            Self::Swap { left, right } => vec![*left, *right],
            Self::Barrier { wires } if wires.is_empty() => (0..wire_count).collect(),
            Self::Barrier { wires } => wires.clone(),
        };
        occupied.sort_unstable();
        (
            *occupied.first().unwrap_or(&0),
            *occupied.last().unwrap_or(&0),
        )
    }
}
