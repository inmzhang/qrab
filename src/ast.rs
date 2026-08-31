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
}

#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub label: String,
    pub wires: Vec<usize>,
    pub start: usize,
    pub end: usize,
    pub style: Style,
    pub span: Span,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasurementShape {
    D,
    Tag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BraceSide {
    Left,
    Right,
    Both,
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
    pub(crate) fn occupied_wires(&self, wire_count: usize) -> Vec<usize> {
        match self {
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
            Self::WireChange { wires, .. }
            | Self::Endpoint { wires, .. }
            | Self::Label { wires, .. }
            | Self::Permute { wires }
            | Self::Phantom { wires }
            | Self::Touch { wires }
            | Self::WireLabels { wires, .. }
            | Self::Brace { wires, .. }
            | Self::Note { wires, .. }
            | Self::Cut { wires, .. }
                if wires.is_empty() =>
            {
                (0..wire_count).collect()
            }
            Self::WireChange { wires, .. }
            | Self::Endpoint { wires, .. }
            | Self::Label { wires, .. }
            | Self::Permute { wires }
            | Self::Phantom { wires }
            | Self::Touch { wires }
            | Self::WireLabels { wires, .. }
            | Self::Brace { wires, .. }
            | Self::Note { wires, .. }
            | Self::Cut { wires, .. } => wires.clone(),
            Self::Bundle { wire, .. } => vec![*wire],
        }
    }

    pub(crate) fn remap_wires(&self, mapping: &[usize]) -> Self {
        let wires = |items: &[usize]| items.iter().map(|wire| mapping[*wire]).collect();
        let selected = |items: &[usize]| {
            if items.is_empty() {
                mapping.to_vec()
            } else {
                wires(items)
            }
        };
        match self {
            Self::Gate {
                label,
                targets,
                controls,
            } => Self::Gate {
                label: label.clone(),
                targets: wires(targets),
                controls: controls
                    .iter()
                    .map(|control| Control {
                        wire: mapping[control.wire],
                        positive: control.positive,
                    })
                    .collect(),
            },
            Self::Measure {
                targets,
                label,
                shape,
            } => Self::Measure {
                targets: wires(targets),
                label: label.clone(),
                shape: *shape,
            },
            Self::Swap { left, right } => Self::Swap {
                left: mapping[*left],
                right: mapping[*right],
            },
            Self::Barrier { wires: targets } => Self::Barrier {
                wires: selected(targets),
            },
            Self::WireChange {
                wires: targets,
                kind,
                label,
            } => Self::WireChange {
                wires: wires(targets),
                kind: *kind,
                label: label.clone(),
            },
            Self::Endpoint {
                wires: targets,
                start,
                label,
            } => Self::Endpoint {
                wires: selected(targets),
                start: *start,
                label: label.clone(),
            },
            Self::Label {
                wires: targets,
                label,
                brace,
            } => Self::Label {
                wires: selected(targets),
                label: label.clone(),
                brace: *brace,
            },
            Self::Bundle { wire, label } => Self::Bundle {
                wire: mapping[*wire],
                label: label.clone(),
            },
            Self::Permute { wires: targets } => Self::Permute {
                wires: wires(targets),
            },
            Self::Phantom { wires: targets } => Self::Phantom {
                wires: selected(targets),
            },
            Self::Touch { wires: targets } => Self::Touch {
                wires: selected(targets),
            },
            Self::WireLabels {
                wires: targets,
                labels,
            } => Self::WireLabels {
                wires: selected(targets),
                labels: labels.clone(),
            },
            Self::Brace {
                wires: targets,
                label,
                side,
            } => Self::Brace {
                wires: selected(targets),
                label: label.clone(),
                side: *side,
            },
            Self::Note {
                wires: targets,
                text,
            } => Self::Note {
                wires: selected(targets),
                text: text.clone(),
            },
            Self::Cut {
                wires: targets,
                label,
            } => Self::Cut {
                wires: selected(targets),
                label: label.clone(),
            },
        }
    }
}
