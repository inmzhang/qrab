#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Circuit {
    pub name: String,
    pub wires: Vec<Wire>,
    pub operations: Vec<Operation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireKind {
    Quantum,
    Classical,
    Hidden,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wire {
    pub name: String,
    pub kind: WireKind,
    pub input: Option<String>,
    pub output: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    pub kind: OperationKind,
    pub span: Span,
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
