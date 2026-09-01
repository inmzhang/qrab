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
    pub groups: Vec<Group>,
    pub escapes: BackendEscapes,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BackendEscapes {
    pub latex: EscapeBlock,
    pub typst: EscapeBlock,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EscapeBlock {
    pub preamble: Vec<String>,
    pub before: Vec<String>,
    pub after: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub label: String,
    pub wires: Vec<usize>,
    pub start: usize,
    pub end: usize,
    pub style: Style,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub orientation: Orientation,
    pub scale: f32,
    pub column_gap: f32,
    pub wire_gap: f32,
    pub gate_size: f32,
    pub corner_radius: f32,
    pub comment_width: f32,
    pub background: String,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            orientation: Orientation::Horizontal,
            scale: 1.0,
            column_gap: 1.5,
            wire_gap: 1.0,
            gate_size: 20.0,
            corner_radius: 4.0,
            comment_width: 144.0,
            background: "white".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Orientation {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum WireKind {
    Quantum,
    Classical,
    Hidden,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Wire {
    pub name: String,
    pub kind: WireKind,
    pub ellipsis: bool,
    pub input: Option<String>,
    pub output: Option<String>,
    pub style: Style,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Operation {
    pub kind: OperationKind,
    pub span: Span,
    pub style: Style,
    pub overlay: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Style {
    pub stroke: Option<String>,
    pub fill: Option<String>,
    pub link: Option<String>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub shape: Option<Shape>,
    pub dashed: bool,
    pub opacity: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Shape {
    Box,
    Circle,
    Ellipse,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MeasurementShape {
    D,
    Tag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BraceSide {
    Left,
    Right,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum NoteSide {
    Above,
    Below,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum OperationKind {
    Gate {
        label: String,
        targets: Vec<usize>,
        controls: Vec<Control>,
    },
    Measure {
        targets: Vec<usize>,
        label: Option<String>,
        shape: MeasurementShape,
    },
    Swap {
        left: usize,
        right: usize,
    },
    Barrier {
        wires: Vec<usize>,
    },
    WireChange {
        wires: Vec<usize>,
        kind: WireKind,
        label: Option<String>,
    },
    Endpoint {
        wires: Vec<usize>,
        start: bool,
        label: Option<String>,
    },
    Label {
        wires: Vec<usize>,
        label: String,
        brace: Option<BraceSide>,
    },
    Bundle {
        wire: usize,
        label: String,
    },
    Permute {
        wires: Vec<usize>,
    },
    Phantom {
        wires: Vec<usize>,
    },
    Touch {
        wires: Vec<usize>,
    },
    WireLabels {
        wires: Vec<usize>,
        labels: Vec<String>,
    },
    Brace {
        wires: Vec<usize>,
        label: String,
        side: BraceSide,
    },
    Note {
        wires: Vec<usize>,
        text: String,
        side: NoteSide,
    },
    Cut {
        wires: Vec<usize>,
        label: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Control {
    pub wire: usize,
    pub positive: bool,
}

impl OperationKind {
    pub(crate) fn wire_selection(&self) -> Option<&[usize]> {
        match self {
            Self::Barrier { wires }
            | Self::WireChange { wires, .. }
            | Self::Endpoint { wires, .. }
            | Self::Label { wires, .. }
            | Self::Permute { wires }
            | Self::Phantom { wires }
            | Self::Touch { wires }
            | Self::WireLabels { wires, .. }
            | Self::Brace { wires, .. }
            | Self::Note { wires, .. }
            | Self::Cut { wires, .. } => Some(wires),
            Self::Gate { .. } | Self::Measure { .. } | Self::Swap { .. } | Self::Bundle { .. } => {
                None
            }
        }
    }

    pub(crate) fn occupied_wires(&self, wire_count: usize) -> Cow<'_, [usize]> {
        if let Some(wires) = self.wire_selection() {
            return if wires.is_empty() {
                Cow::Owned((0..wire_count).collect())
            } else {
                Cow::Borrowed(wires)
            };
        }
        match self {
            Self::Gate {
                targets, controls, ..
            } if controls.is_empty() => Cow::Borrowed(targets),
            Self::Gate {
                targets, controls, ..
            } => Cow::Owned(
                targets
                    .iter()
                    .copied()
                    .chain(controls.iter().map(|control| control.wire))
                    .collect(),
            ),
            Self::Measure { targets, .. } => Cow::Borrowed(targets),
            Self::Swap { left, right } => Cow::Owned(vec![*left, *right]),
            Self::Barrier { .. }
            | Self::WireChange { .. }
            | Self::Endpoint { .. }
            | Self::Label { .. }
            | Self::Permute { .. }
            | Self::Phantom { .. }
            | Self::Touch { .. }
            | Self::WireLabels { .. }
            | Self::Brace { .. }
            | Self::Note { .. }
            | Self::Cut { .. } => unreachable!("wire selections returned above"),
            Self::Bundle { wire, .. } => Cow::Owned(vec![*wire]),
        }
    }

    pub(crate) fn remap_wires(&self, mapping: &[usize]) -> Self {
        let remap = |wires: &mut Vec<usize>| {
            for wire in wires {
                *wire = mapping[*wire];
            }
        };
        let remap_selection = |wires: &mut Vec<usize>| {
            if wires.is_empty() {
                *wires = mapping.to_vec();
            } else {
                remap(wires);
            }
        };
        let mut operation = self.clone();
        match &mut operation {
            Self::Gate {
                targets, controls, ..
            } => {
                remap(targets);
                for control in controls {
                    control.wire = mapping[control.wire];
                }
            }
            Self::Measure { targets, .. } => remap(targets),
            Self::Swap { left, right } => {
                *left = mapping[*left];
                *right = mapping[*right];
            }
            Self::Barrier { wires }
            | Self::Endpoint { wires, .. }
            | Self::Label { wires, .. }
            | Self::Phantom { wires }
            | Self::Touch { wires }
            | Self::WireLabels { wires, .. }
            | Self::Brace { wires, .. }
            | Self::Note { wires, .. }
            | Self::Cut { wires, .. } => remap_selection(wires),
            Self::WireChange { wires, .. } | Self::Permute { wires } => remap(wires),
            Self::Bundle { wire, .. } => *wire = mapping[*wire],
        }
        operation
    }
}
use std::borrow::Cow;
