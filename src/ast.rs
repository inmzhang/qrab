use std::borrow::Cow;

/// A byte range and derived one-based location in expanded `.qrab` source.
///
/// Spans are produced by the parser and are not constructed by downstream code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Span {
    /// Zero-based byte offset in the expanded source.
    pub offset: usize,
    /// Length of the source range in bytes.
    pub length: usize,
    /// One-based line number.
    pub line: usize,
    /// One-based column number.
    pub column: usize,
}

/// A parsed and semantically checked quantum circuit.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Circuit {
    /// Circuit identifier.
    pub name: String,
    /// Global layout settings.
    pub layout: Layout,
    /// Wires in declaration order.
    pub wires: Vec<Wire>,
    /// Lowered operations in source order.
    pub operations: Vec<Operation>,
    /// Mark-delimited visual groups.
    pub groups: Vec<Group>,
    /// Explicit backend-only snippets.
    pub escapes: BackendEscapes,
}

/// Raw snippets isolated by output backend.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct BackendEscapes {
    /// LaTeX-only snippets.
    pub latex: EscapeBlock,
    /// Typst-only snippets.
    pub typst: EscapeBlock,
}

/// Raw snippets inserted at defined points in one backend document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct EscapeBlock {
    /// Snippets inserted with imports or package declarations.
    pub preamble: Vec<String>,
    /// Snippets inserted immediately before the rendered circuit.
    pub before: Vec<String>,
    /// Snippets inserted immediately after the rendered circuit.
    pub after: Vec<String>,
}

/// A labeled region spanning a half-open operation range.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Group {
    /// Displayed group label.
    pub label: String,
    /// Selected wire indices, or all wires when empty.
    pub wires: Vec<usize>,
    /// First operation index included in the group.
    pub start: usize,
    /// Operation index immediately after the group.
    pub end: usize,
    /// Group border and fill style.
    pub style: Style,
}

/// Global circuit geometry and page appearance.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Layout {
    /// Direction in which operations advance.
    pub orientation: Orientation,
    /// Overall render scale.
    pub scale: f32,
    /// Abstract gap between operation columns.
    pub column_gap: f32,
    /// Abstract gap between wire rows.
    pub wire_gap: f32,
    /// Default gate size in points.
    pub gate_size: f32,
    /// Permutation bend radius in points.
    pub corner_radius: f32,
    /// Note text width in points.
    pub comment_width: f32,
    /// Portable named or hexadecimal background color.
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

/// Direction in which operation columns advance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    /// Wires run from left to right.
    Horizontal,
    /// Wires run from top to bottom.
    Vertical,
}

/// Visual and semantic state of a wire segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireKind {
    /// A single quantum wire.
    Quantum,
    /// A doubled classical wire.
    Classical,
    /// No visible wire.
    Hidden,
}

/// One declared circuit wire.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Wire {
    /// Unique source-level wire name.
    pub name: String,
    /// Initial wire kind.
    pub kind: WireKind,
    /// Whether the row represents an ellipsis gap.
    pub ellipsis: bool,
    /// Optional left endpoint label.
    pub input: Option<String>,
    /// Optional right endpoint label.
    pub output: Option<String>,
    /// Persistent line style.
    pub style: Style,
}

/// One scheduled visual operation before backend rendering.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Operation {
    /// Operation data.
    pub kind: OperationKind,
    /// Source location used for diagnostics.
    pub span: Span,
    /// Portable visual style.
    pub style: Style,
    /// Identifier shared by operations forced into one overlay column.
    pub overlay: Option<usize>,
}

/// Portable visual properties shared by circuit elements.
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct Style {
    /// Stroke or text color.
    pub stroke: Option<String>,
    /// Interior or label-background color.
    pub fill: Option<String>,
    /// Absolute HTTP(S) or mailto hyperlink.
    pub link: Option<String>,
    /// Requested width in points.
    pub width: Option<f32>,
    /// Requested height in points.
    pub height: Option<f32>,
    /// Requested geometric shape.
    pub shape: Option<Shape>,
    /// Whether supported strokes are dashed.
    pub dashed: bool,
    /// Opacity from zero through one.
    pub opacity: Option<f32>,
}

/// Geometric shape requested for a supported visual element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// A rectangular box.
    Box,
    /// A circle.
    Circle,
    /// An ellipse.
    Ellipse,
    /// No visible border or box.
    None,
}

/// Shape used for a labeled measurement result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasurementShape {
    /// A rounded D-shaped marker.
    D,
    /// A pointed tag marker.
    Tag,
}

/// Side on which a brace is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BraceSide {
    /// Left side only.
    Left,
    /// Right side only.
    Right,
    /// Both sides.
    Both,
}

/// Vertical placement of a note.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteSide {
    /// Above the selected wires.
    Above,
    /// Below the selected wires.
    Below,
}

/// Semantic operation rendered into one circuit column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationKind {
    /// A named gate with one or more targets and optional controls.
    Gate {
        /// Displayed gate label.
        label: String,
        /// Target wire indices.
        targets: Vec<usize>,
        /// Positive or negative controls.
        controls: Vec<Control>,
    },
    /// A measurement that changes each target to a classical wire.
    Measure {
        /// Measured wire indices.
        targets: Vec<usize>,
        /// Optional result-marker label.
        label: Option<String>,
        /// Marker shape used when a label is present.
        shape: MeasurementShape,
    },
    /// A visual swap between two wires.
    Swap {
        /// First wire index.
        left: usize,
        /// Second wire index.
        right: usize,
    },
    /// A dashed barrier across selected wires.
    Barrier {
        /// Selected wire indices, or all wires when empty.
        wires: Vec<usize>,
    },
    /// A persistent wire-kind change with an optional value marker.
    WireChange {
        /// Changed wire indices.
        wires: Vec<usize>,
        /// New wire kind.
        kind: WireKind,
        /// Optional known-value label.
        label: Option<String>,
    },
    /// A wire lifecycle start or end tick.
    Endpoint {
        /// Selected wire indices, or all wires when empty.
        wires: Vec<usize>,
        /// `true` for a start and `false` for an end.
        start: bool,
        /// Optional endpoint label.
        label: Option<String>,
    },
    /// Text centered over a selected wire span.
    Label {
        /// Selected wire indices, or all wires when empty.
        wires: Vec<usize>,
        /// Displayed text.
        label: String,
        /// Optional surrounding brace placement.
        brace: Option<BraceSide>,
    },
    /// A bundle-count slash on one wire.
    Bundle {
        /// Wire index.
        wire: usize,
        /// Displayed bundle count.
        label: String,
    },
    /// A persistent visual reordering of selected wires.
    Permute {
        /// Wire indices listed in their destination-row order.
        wires: Vec<usize>,
    },
    /// Invisible space reserved on selected wires.
    Phantom {
        /// Selected wire indices, or all wires when empty.
        wires: Vec<usize>,
    },
    /// An alignment-only operation, optionally drawn as a slice.
    Touch {
        /// Selected wire indices, or all wires when empty.
        wires: Vec<usize>,
    },
    /// Per-wire labels at one column.
    WireLabels {
        /// Selected wire indices, or all wires when empty.
        wires: Vec<usize>,
        /// One repeated label or one label per selected wire.
        labels: Vec<String>,
    },
    /// A brace and label spanning selected wires.
    Brace {
        /// Selected wire indices, or all wires when empty.
        wires: Vec<usize>,
        /// Displayed brace label.
        label: String,
        /// Brace placement.
        side: BraceSide,
    },
    /// Free text above or below selected wires.
    Note {
        /// Selected wire indices, or all wires when empty.
        wires: Vec<usize>,
        /// Displayed note text.
        text: String,
        /// Note placement.
        side: NoteSide,
    },
    /// A dashed stage separator across selected wires.
    Cut {
        /// Selected wire indices, or all wires when empty.
        wires: Vec<usize>,
        /// Optional separator label.
        label: Option<String>,
    },
}

/// One positive or negative gate control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Control {
    /// Control wire index.
    pub wire: usize,
    /// `true` for a closed control and `false` for an open control.
    pub positive: bool,
}

impl OperationKind {
    pub(crate) fn occupied_wires(&self, wire_count: usize) -> Cow<'_, [usize]> {
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
            | Self::Cut { wires, .. } => {
                if wires.is_empty() {
                    Cow::Owned((0..wire_count).collect())
                } else {
                    Cow::Borrowed(wires)
                }
            }
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
