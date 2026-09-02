use std::{borrow::Cow, collections::HashMap};

use ratex_layout::{LayoutOptions, layout as layout_math, to_display_list};
use ratex_parser::parse as parse_math;
use ratex_types::{color::Color, display_item::DisplayList, math_style::MathStyle};

use crate::ast::{
    BraceSide, Circuit, Control, Group, Layout, MeasurementShape, NoteSide, OperationKind,
    Orientation, ParityBasis, Shape, Style, Wire, WireKind,
};

const POINTS_PER_CENTIMETER: f32 = 28.45;
const FONT_SIZE: f32 = 10.0;

/// Generous 10pt monospace advance estimate; math uses measured RaTeX layout.
const CHARACTER_WIDTH: f32 = 0.62 * FONT_SIZE / POINTS_PER_CENTIMETER;

/// Room left between a label and the edge of the shape around it, in
/// centimetres. Matches the SVG canvas padding and TikZ's default `inner sep`.
const LABEL_PADDING: f32 = 4.0 / POINTS_PER_CENTIMETER;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LabelSpan<'a> {
    Text(&'a str),
    Math(&'a str),
}

#[derive(Debug, Clone, Copy)]
struct LabelDimensions {
    width: f32,
    height: f32,
}

/// Splits a label on paired `$`; unmatched dollars stay literal.
fn label_spans(label: &str) -> Vec<LabelSpan<'_>> {
    let mut delimiters = Vec::new();
    let mut backslashes = 0;
    for (offset, character) in label.char_indices() {
        if character == '$' && backslashes % 2 == 0 {
            delimiters.push(offset);
        }
        backslashes = if character == '\\' {
            backslashes + 1
        } else {
            0
        };
    }

    let paired = delimiters.len() / 2 * 2;
    let mut spans = Vec::with_capacity(paired + 1);
    let mut cursor = 0;
    for pair in delimiters[..paired].as_chunks::<2>().0 {
        if cursor < pair[0] {
            spans.push(LabelSpan::Text(&label[cursor..pair[0]]));
        }
        spans.push(LabelSpan::Math(&label[pair[0] + 1..pair[1]]));
        cursor = pair[1] + 1;
    }
    if cursor < label.len() {
        spans.push(LabelSpan::Text(&label[cursor..]));
    }
    if spans.is_empty() {
        spans.push(LabelSpan::Text(label));
    }
    spans
}

fn label_text(text: &str) -> Cow<'_, str> {
    if text.contains("\\$") {
        Cow::Owned(text.replace("\\$", "$"))
    } else {
        Cow::Borrowed(text)
    }
}

fn label_has_math(label: &str) -> bool {
    label_spans(label)
        .iter()
        .any(|span| matches!(span, LabelSpan::Math(_)))
}

fn circuit_has_math(circuit: &Circuit) -> bool {
    circuit.wires.iter().any(|wire| {
        wire.input.as_deref().is_some_and(label_has_math)
            || wire.output.as_deref().is_some_and(label_has_math)
    }) || circuit
        .groups
        .iter()
        .any(|group| label_has_math(&group.label))
        || circuit
            .operations
            .iter()
            .any(|operation| match &operation.kind {
                OperationKind::Gate { label, .. }
                | OperationKind::Label { label, .. }
                | OperationKind::Bundle { label, .. }
                | OperationKind::Brace { label, .. } => label_has_math(label),
                OperationKind::Measure { label, .. }
                | OperationKind::WireChange { label, .. }
                | OperationKind::Endpoint { label, .. }
                | OperationKind::Cut { label, .. } => label.as_deref().is_some_and(label_has_math),
                OperationKind::WireLabels { labels, .. } => {
                    labels.iter().any(|label| label_has_math(label))
                }
                OperationKind::Note { text, .. } => label_has_math(text),
                _ => false,
            })
}

fn math_display_list(math: &str, color: Color) -> Option<DisplayList> {
    let parsed = parse_math(math).ok()?;
    let options = LayoutOptions {
        style: MathStyle::Text,
        color,
        ..LayoutOptions::default()
    };
    Some(to_display_list(&layout_math(&parsed, &options)))
}

/// Estimated dimensions of `label` when drawn, in centimetres.
fn label_dimensions(label: &str) -> LabelDimensions {
    let mut dimensions = LabelDimensions {
        width: 0.0,
        height: FONT_SIZE / POINTS_PER_CENTIMETER,
    };
    for span in label_spans(label) {
        match span {
            LabelSpan::Text(text) => {
                dimensions.width += label_text(text).chars().count() as f32 * CHARACTER_WIDTH;
            }
            LabelSpan::Math(math) => match math_display_list(math, Color::BLACK) {
                Some(display) => {
                    dimensions.width += display.width as f32 * FONT_SIZE / POINTS_PER_CENTIMETER;
                    dimensions.height = dimensions
                        .height
                        .max(display.total_height() as f32 * FONT_SIZE / POINTS_PER_CENTIMETER);
                }
                None => dimensions.width += math.chars().count() as f32 * CHARACTER_WIDTH,
            },
        }
    }
    dimensions
}

/// Estimated width of `label` when drawn, in centimetres.
fn label_width(label: &str) -> f32 {
    label_dimensions(label).width
}

macro_rules! emit {
    ($output:expr, $($argument:tt)*) => {
        writeln!($output, $($argument)*).expect("writing to a String cannot fail")
    };
}

mod latex;
mod quirk;
mod svg;
mod typst;

/// Output format produced by [`render`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// A standalone LaTeX document using TikZ.
    Latex,
    /// A standalone Typst document using Quill.
    Typst,
    /// A URL that opens the circuit in the interactive Quirk simulator.
    ///
    /// Quirk supports at most 16 qubits. Visual-only constructs without a
    /// Quirk equivalent are omitted, while arbitrary named gates are retained
    /// as labeled no-op custom gates because qrab assigns them no unitary.
    Quirk,
    /// A standalone SVG image.
    ///
    /// Unlike the other two targets this one needs no external toolchain, so it
    /// is what the WebAssembly playground renders. It reuses the LaTeX
    /// backend's geometry but approximates text metrics, which SVG cannot
    /// measure; see the module documentation for the resulting differences.
    Svg,
}

/// Renders a parsed circuit as a standalone document for `target`.
pub fn render(circuit: &Circuit, target: Target) -> String {
    match target {
        Target::Latex => latex::render_latex(circuit),
        Target::Typst => typst::render_typst(circuit),
        Target::Quirk => quirk::render_quirk(circuit),
        Target::Svg => svg::render_svg(circuit),
    }
}

#[derive(Debug, Clone)]
struct Scheduled<'a> {
    kind: &'a OperationKind,
    style: &'a Style,
    column: usize,
    positions: Vec<usize>,
    permutation: Option<Vec<usize>>,
    first: usize,
    last: usize,
}

impl Scheduled<'_> {
    fn permuted_row(&self, wire: usize) -> usize {
        self.permutation
            .as_deref()
            .expect("permutation operation has a row mapping")[self.positions[wire]]
    }
}

fn schedule(circuit: &Circuit) -> (Vec<Scheduled<'_>>, Vec<usize>) {
    let mut tracks = vec![0_usize; circuit.wires.len()];
    let mut positions = initial_rows(circuit);
    let mut order = positions.clone();
    let mut scheduled = Vec::with_capacity(circuit.operations.len());
    let mut overlay_columns = HashMap::new();
    for operation in &circuit.operations {
        let occupied = operation.kind.occupied_wires(circuit.wires.len());
        let first = occupied
            .iter()
            .map(|wire| positions[*wire])
            .min()
            .unwrap_or(0);
        let last = occupied
            .iter()
            .map(|wire| positions[*wire])
            .max()
            .unwrap_or(0);
        let interval = &order[first..=last];
        let column = if let Some(overlay) = operation.overlay {
            let column = *overlay_columns
                .entry(overlay)
                .or_insert_with(|| overlay_column(circuit, overlay, &tracks, &order, &positions));
            for wire in interval {
                tracks[*wire] = tracks[*wire].max(column + 1);
            }
            column
        } else if matches!(operation.kind, OperationKind::Note { .. }) {
            scheduled
                .last()
                .map_or(0, |operation: &Scheduled<'_>| operation.column)
        } else if matches!(operation.kind, OperationKind::Touch { .. }) {
            let previous_column = scheduled
                .last()
                .map_or(0, |operation: &Scheduled<'_>| operation.column);
            let target_column = interval
                .iter()
                .map(|wire| tracks[*wire])
                .max()
                .unwrap_or(0)
                .saturating_sub(1);
            let column = previous_column.max(target_column);
            for wire in interval {
                tracks[*wire] = column + 1;
            }
            column
        } else {
            let column = interval.iter().map(|wire| tracks[*wire]).max().unwrap_or(0);
            for wire in interval {
                tracks[*wire] = column + 1;
            }
            column
        };
        let scheduled_operation = Scheduled {
            kind: &operation.kind,
            style: &operation.style,
            column,
            // ponytail: snapshots keep render reads simple; intern if large permuted circuits
            // make this clone measurable.
            positions: positions.clone(),
            permutation: if let OperationKind::Permute { wires } = &operation.kind {
                Some(permutation_mapping(wires, &positions))
            } else {
                None
            },
            first,
            last,
        };
        if let Some(mapping) = &scheduled_operation.permutation {
            let previous_order = order.clone();
            for (source, destination) in mapping.iter().copied().enumerate() {
                order[destination] = previous_order[source];
            }
            for (row, wire) in order.iter().enumerate() {
                positions[*wire] = row;
            }
        }
        scheduled.push(scheduled_operation);
    }
    delay_starts(circuit, &mut scheduled);
    (scheduled, positions)
}

/// Row each wire starts in, before any `permute` moves it.
///
/// A vertical circuit is drawn horizontally and then rotated by a quarter turn.
/// The rotation that makes time run down the page, which is how qpic reads,
/// also swings the first wire over to the right-hand side, so vertical circuits
/// deal their rows out bottom-up to swing it back. `order` and `positions` are
/// each other's inverse and reversal is its own inverse, so one vector serves
/// as both.
fn initial_rows(circuit: &Circuit) -> Vec<usize> {
    (0..circuit.wires.len())
        .map(|wire| initial_row(circuit, wire))
        .collect()
}

/// Row `wire` starts in. See [`initial_rows`].
fn initial_row(circuit: &Circuit, wire: usize) -> usize {
    if circuit.layout.orientation == Orientation::Vertical {
        circuit.wires.len() - 1 - wire
    } else {
        wire
    }
}

fn overlay_column(
    circuit: &Circuit,
    overlay: usize,
    tracks: &[usize],
    order: &[usize],
    positions: &[usize],
) -> usize {
    circuit
        .operations
        .iter()
        .filter(|operation| operation.overlay == Some(overlay))
        .filter_map(|operation| {
            let occupied = operation.kind.occupied_wires(circuit.wires.len());
            let first = occupied.iter().map(|wire| positions[*wire]).min()?;
            let last = occupied.iter().map(|wire| positions[*wire]).max()?;
            order[first..=last].iter().map(|wire| tracks[*wire]).max()
        })
        .max()
        .unwrap_or(0)
}

fn delay_starts(circuit: &Circuit, scheduled: &mut [Scheduled<'_>]) {
    // ponytail: this quadratic scan is tiny for diagram-sized inputs; index by wire and
    // column only if profiling shows large generated circuits spend time here.
    let final_column = scheduled
        .iter()
        .map(|operation| operation.column)
        .max()
        .unwrap_or(0);
    for index in (0..scheduled.len()).rev() {
        let OperationKind::Endpoint {
            wires, start: true, ..
        } = scheduled[index].kind
        else {
            continue;
        };
        let wires = wires.as_slice();
        let next_column = scheduled[index + 1..]
            .iter()
            .filter(|operation| {
                operation
                    .kind
                    .occupied_wires(circuit.wires.len())
                    .iter()
                    .any(|wire| wires.contains(wire))
            })
            .map(|operation| operation.column)
            .min()
            .unwrap_or_else(|| final_column.saturating_add(1));
        let latest = next_column.saturating_sub(1);
        let (first, last, original) = (
            scheduled[index].first,
            scheduled[index].last,
            scheduled[index].column,
        );
        if let Some(column) = (original..=latest).rev().find(|column| {
            !scheduled
                .iter()
                .enumerate()
                .any(|(other_index, operation)| {
                    other_index != index
                        && operation.column == *column
                        && operation.first <= last
                        && first <= operation.last
                })
        }) {
            scheduled[index].column = column;
        }
    }
}

fn group_bounds(
    group: &Group,
    scheduled: &[Scheduled<'_>],
    wire_count: usize,
) -> (usize, usize, usize, usize) {
    let operations = &scheduled[group.start..group.end];
    let first_column = operations
        .iter()
        .map(|operation| operation.column + 1)
        .min()
        .expect("groups are not empty");
    let last_column = operations
        .iter()
        .map(|operation| operation.column + 1)
        .max()
        .expect("groups are not empty");
    let wires = selected_wires(&group.wires, wire_count);
    let mut rows = operations.iter().flat_map(|operation| {
        wires.iter().flat_map(move |wire| {
            let current = operation.positions[*wire];
            let moved = match operation.kind {
                OperationKind::Permute { wires: permutation } if permutation.contains(wire) => {
                    operation.permuted_row(*wire)
                }
                _ => current,
            };
            [current, moved]
        })
    });
    let first_row = rows.next().expect("groups select at least one wire");
    rows.fold(
        (first_column, last_column, first_row, first_row),
        |(first_column, last_column, first_row, last_row), row| {
            (
                first_column,
                last_column,
                first_row.min(row),
                last_row.max(row),
            )
        },
    )
}

fn wire_transitions(
    circuit: &Circuit,
    scheduled: &[Scheduled<'_>],
    wire: usize,
    positions: &[f32],
) -> (WireKind, Vec<(f32, WireKind)>) {
    let initial = initial_wire_kind(circuit, scheduled, wire);
    let mut transitions = scheduled
        .iter()
        .filter_map(|operation| wire_transition(circuit, operation, wire, positions))
        .collect::<Vec<_>>();
    transitions.sort_by(|left, right| left.0.total_cmp(&right.0));
    (initial, transitions)
}

/// Horizontal centre of every scheduled column, in centimetres, with one extra
/// entry past the last column for the right-hand edge of the diagram.
///
/// Columns are spaced evenly by `layout.column_gap`, which is what the abstract
/// gap means, until something in the column needs more room than that. The
/// Typst backend hands widths straight to Quill, whose grid sizes columns to
/// their contents, so the coordinate-based backends have to measure the same
/// thing themselves or wide gates land on top of their neighbours.
///
/// The extra width is split around the column that asked for it and shifts
/// everything after it, so the room is inserted rather than borrowed.
fn column_positions(circuit: &Circuit, scheduled: &[Scheduled<'_>]) -> Vec<f32> {
    let gap = circuit.layout.column_gap;
    let last = scheduled
        .iter()
        .map(|operation| operation.column)
        .max()
        .unwrap_or(0);

    let mut extra = vec![0.0_f32; last + 1];
    for operation in scheduled {
        let width = occupied_width(operation, &circuit.layout);
        extra[operation.column] = extra[operation.column].max(width - gap);
    }

    let mut positions = Vec::with_capacity(last + 2);
    let mut shift = 0.0;
    for (column, extra) in extra.iter().enumerate() {
        positions.push((column + 1) as f32 * gap + shift + extra / 2.0);
        shift += extra;
    }
    positions.push((last + 2) as f32 * gap + shift);
    positions
}

/// Whether a note sits on the low-row side of the wires it annotates.
///
/// Vertical circuits deal their rows out bottom-up (see [`initial_rows`]), so
/// the row on a given side of the span is the opposite one. Without the flip a
/// note would land between the wires instead of clear of them.
fn note_is_above(circuit: &Circuit, side: NoteSide) -> bool {
    (side == NoteSide::Above) != (circuit.layout.orientation == Orientation::Vertical)
}

/// Copies `style` with the diagram background as its fill.
///
/// Labels are drawn on top of the wires they annotate. qpic fills every label
/// node with the background colour so the wire breaks around the text instead
/// of striking through it; an explicit fill still wins.
fn masked_style(style: &Style, background: &str) -> Style {
    let mut masked = style.clone();
    masked.fill.get_or_insert_with(|| background.to_owned());
    masked
}

/// Horizontal room one operation needs, in centimetres.
///
/// Shapes grow to fit their label rather than clipping it, so the width a gate
/// finally occupies is whatever is largest: the requested width, the default
/// gate size, or the label plus its padding. `space` reserves its requested
/// width and nothing else; everything else is drawn small enough that the
/// column gap already covers it.
fn occupied_width(operation: &Scheduled<'_>, layout: &Layout) -> f32 {
    let requested = operation.style.width.unwrap_or(0.0) / POINTS_PER_CENTIMETER;
    let boxed = |label: &str| {
        requested
            .max(layout.gate_size / POINTS_PER_CENTIMETER)
            .max(label_width(label) + 2.0 * LABEL_PADDING)
    };
    match operation.kind {
        OperationKind::Gate { label, .. } => boxed(label),
        OperationKind::Measure {
            label: Some(label), ..
        } => boxed(label),
        OperationKind::Label { label, brace, .. } => {
            label_width(label)
                + if brace.is_some() {
                    4.0 * LABEL_PADDING
                } else {
                    0.0
                }
        }
        OperationKind::Brace { label, .. } => label_width(label) + 4.0 * LABEL_PADDING,
        // A lifecycle label hangs off one side of its column, so doubling it
        // leaves exactly the room it needs once the extra is split in two: a
        // `pause` ending and the `resume` after it each claim their own half.
        OperationKind::Endpoint {
            label: Some(label), ..
        } => 2.0 * (label_width(label) + LABEL_PADDING),
        _ => requested,
    }
}

fn initial_wire_kind(circuit: &Circuit, scheduled: &[Scheduled<'_>], wire: usize) -> WireKind {
    let starts_late = scheduled.iter().find_map(|operation| match operation.kind {
        OperationKind::Endpoint { wires, start, .. }
            if includes_wire(wires, wire, circuit.wires.len()) =>
        {
            Some(*start)
        }
        _ => None,
    });
    if starts_late == Some(true) {
        WireKind::Hidden
    } else {
        circuit.wires[wire].kind
    }
}

fn wire_kind_transition(
    circuit: &Circuit,
    operation: &OperationKind,
    wire: usize,
) -> Option<WireKind> {
    match operation {
        OperationKind::Measure { targets, .. } if targets.contains(&wire) => {
            Some(WireKind::Classical)
        }
        OperationKind::WireChange { wires, kind, .. }
            if includes_wire(wires, wire, circuit.wires.len()) =>
        {
            Some(*kind)
        }
        OperationKind::Endpoint {
            wires, start: true, ..
        } if includes_wire(wires, wire, circuit.wires.len()) => Some(circuit.wires[wire].kind),
        OperationKind::Endpoint {
            wires,
            start: false,
            ..
        } if includes_wire(wires, wire, circuit.wires.len()) => Some(WireKind::Hidden),
        _ => None,
    }
}

fn wire_transition(
    circuit: &Circuit,
    operation: &Scheduled<'_>,
    wire: usize,
    positions: &[f32],
) -> Option<(f32, WireKind)> {
    let kind = wire_kind_transition(circuit, operation.kind, wire)?;
    let x = positions[operation.column];
    Some((
        if matches!(operation.kind, OperationKind::Measure { .. }) {
            x + circuit.layout.column_gap.min(0.34)
        } else {
            x
        },
        kind,
    ))
}

fn wire_kind_before(
    circuit: &Circuit,
    scheduled: &[Scheduled<'_>],
    operation_index: usize,
    wire: usize,
) -> WireKind {
    scheduled[..operation_index]
        .iter()
        .rev()
        .find_map(|operation| wire_kind_transition(circuit, operation.kind, wire))
        .unwrap_or_else(|| initial_wire_kind(circuit, scheduled, wire))
}

fn permutation_mapping(wires: &[usize], positions: &[usize]) -> Vec<usize> {
    let mut destinations = wires
        .iter()
        .map(|wire| positions[*wire])
        .collect::<Vec<_>>();
    destinations.sort_unstable();
    let mut mapping = (0..positions.len()).collect::<Vec<_>>();
    for (wire, destination) in wires.iter().zip(destinations) {
        mapping[positions[*wire]] = destination;
    }
    mapping
}

fn merged_line_style(base: &Style, overlay: &Style) -> Style {
    Style {
        stroke: overlay.stroke.clone().or_else(|| base.stroke.clone()),
        dashed: base.dashed || overlay.dashed,
        opacity: overlay.opacity.or(base.opacity),
        ..Style::default()
    }
}

fn has_line_style(style: &Style) -> bool {
    style.stroke.is_some() || style.dashed || style.opacity.is_some()
}

// Empty selections remain "all wires" for callers constructing the public AST directly.
fn includes_wire(wires: &[usize], wire: usize, wire_count: usize) -> bool {
    (wires.is_empty() && wire < wire_count) || wires.contains(&wire)
}

fn selected_wires(wires: &[usize], wire_count: usize) -> Cow<'_, [usize]> {
    if wires.is_empty() {
        Cow::Owned((0..wire_count).collect())
    } else {
        Cow::Borrowed(wires)
    }
}

fn expanded_wires(wires: &[usize], wire_count: usize) -> Vec<usize> {
    let mut expanded = selected_wires(wires, wire_count).into_owned();
    expanded.sort_unstable();
    expanded
}

fn occupied_bounds(targets: &[usize], controls: &[Control]) -> (usize, usize) {
    let mut occupied = targets
        .iter()
        .copied()
        .chain(controls.iter().map(|control| control.wire));
    let first = occupied.next().expect("gate has a target");
    occupied.fold((first, first), |(min, max), wire| {
        (min.min(wire), max.max(wire))
    })
}

fn append_raw(output: &mut String, snippets: &[String]) {
    for snippet in snippets {
        output.push_str(snippet);
        if !snippet.ends_with('\n') {
            output.push('\n');
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::parse;

    use super::*;

    #[test]
    fn permutation_moves_later_operations_and_output_labels() {
        let circuit = parse(
            r#"
                circuit reorder {
                  qubit q[3] -> "out"
                  permute q[2], q[0], q[1]
                  h q[2]
                }
            "#,
        )
        .expect("valid permutation");
        let (scheduled, final_positions) = schedule(&circuit);

        assert_eq!(
            scheduled[0].permutation.as_deref(),
            Some([1, 2, 0].as_slice())
        );
        assert_eq!(scheduled[1].positions[2], 0);
        assert_eq!(final_positions, vec![1, 2, 0]);
    }

    #[test]
    fn a_vertical_circuit_reads_downward_with_the_first_wire_on_the_left() {
        // qpic reads a vertical circuit top to bottom. The quarter turn that
        // sends time downward also swings the first wire to the right, so the
        // rows are dealt out bottom-up to swing it back; both halves have to
        // hold together or the diagram comes out mirrored.
        let circuit = parse(
            r#"
                circuit vertical {
                  layout { orientation: vertical }
                  qubit a
                  qubit b
                  h a
                }
            "#,
        )
        .expect("valid vertical circuit");

        assert_eq!(initial_rows(&circuit), vec![1, 0]);

        let latex = render(&circuit, Target::Latex);
        assert!(latex.contains("rotate=-90"));
        // Wire `a` is one gap below wire `b`, which the turn reads as one gap
        // to its left.
        assert!(latex.contains("(0.000,-1.000) -- (3.000,-1.000)"));

        let svg = render(&circuit, Target::Svg);
        assert!(svg.contains("transform=\"rotate(90)\""));
    }

    #[test]
    fn a_wide_space_inserts_room_and_shifts_the_columns_after_it() {
        // No fixture in the corpus asks for more than the default 1.5cm gap,
        // so this is the only thing standing between `space` and being a
        // silent no-op in the two coordinate-based backends again.
        let source = |width: u32| {
            format!(
                r#"
                circuit spaced {{
                  qubit q[2]
                  h q[0]
                  space q[0] with width: {width}
                  h q[0]
                }}
            "#
            )
        };

        let narrow = parse(&source(20)).expect("valid narrow space");
        let (scheduled, _) = schedule(&narrow);
        let columns = column_positions(&narrow, &scheduled);
        // 20pt is less than the 1.5cm gap the column already provides, so the
        // grid stays uniform: the abstract gap is a floor, not a target.
        assert_eq!(columns, vec![1.5, 3.0, 4.5, 6.0]);

        let wide = parse(&source(200)).expect("valid wide space");
        let (scheduled, _) = schedule(&wide);
        let columns = column_positions(&wide, &scheduled);
        let extra = 200.0 / POINTS_PER_CENTIMETER - 1.5;
        assert_eq!(columns[0], 1.5);
        assert_eq!(columns[1], 3.0 + extra / 2.0);
        assert_eq!(columns[2], 4.5 + extra);
        assert_eq!(columns[3], 6.0 + extra);

        // The rendered documents have to move with it, not just the model.
        let latex = render(&wide, Target::Latex);
        let svg = render(&wide, Target::Svg);
        assert!(latex.contains(&format!("({:.3},", columns[2])));
        assert!(svg.contains(&format!("\"{:.3}\"", columns[2] * POINTS_PER_CENTIMETER)));
    }

    #[test]
    fn touch_aligns_with_the_previous_operation_not_the_deepest_wire() {
        let circuit = parse(
            r#"
                circuit touch {
                  qubit q[2]
                  h q[0]
                  h q[0]
                  h q[1]
                  touch q[1]
                  x q[1]
                }
            "#,
        )
        .expect("valid touch circuit");
        let (scheduled, _) = schedule(&circuit);

        assert_eq!(
            scheduled
                .iter()
                .map(|operation| operation.column)
                .collect::<Vec<_>>(),
            [0, 1, 0, 0, 1]
        );
    }

    #[test]
    fn notes_annotate_without_advancing_the_schedule() {
        let circuit = parse(
            r#"
                circuit comments {
                  qubit q
                  h q
                  note below "prepared" on q
                  x q
                }
            "#,
        )
        .expect("valid note");
        let (scheduled, _) = schedule(&circuit);

        assert_eq!(
            scheduled
                .iter()
                .map(|operation| operation.column)
                .collect::<Vec<_>>(),
            [0, 0, 1]
        );
    }

    #[test]
    fn start_is_placed_immediately_before_its_first_use() {
        let circuit = parse(
            r#"
                circuit late_start {
                  qubit q[2]
                  start q[1]
                  h q[0]
                  h q[0]
                  h q[0]
                  x q[1] if q[0]
                }
            "#,
        )
        .expect("valid deferred wire");
        let (scheduled, _) = schedule(&circuit);

        assert_eq!(scheduled[0].column, 2);
        assert_eq!(scheduled[4].column, 3);
    }

    #[test]
    fn parallel_block_aligns_independent_wires_after_prior_work() {
        let circuit = parse(
            r#"
                circuit parallel_work {
                  qubit q[2]
                  h q[0]
                  h q[0]
                  parallel {
                    h q[0]
                    h q[1]
                  }
                }
            "#,
        )
        .expect("valid parallel block");
        let (scheduled, _) = schedule(&circuit);

        assert_eq!(
            scheduled
                .iter()
                .map(|operation| operation.column)
                .collect::<Vec<_>>(),
            [0, 1, 1, 2, 2, 2]
        );
    }

    #[test]
    fn overlay_forces_colliding_operations_into_one_column() {
        let circuit = parse(
            r#"
                circuit forced_overlap {
                  qubit q[3]
                  h q[0]
                  h q[0]
                  repeat 2 {
                    overlay {
                      x q[0] if q[2]
                      h q[1]
                    }
                  }
                  x q[0]
                }
            "#,
        )
        .expect("valid overlays");
        let (scheduled, _) = schedule(&circuit);

        assert_eq!(
            scheduled
                .iter()
                .map(|operation| operation.column)
                .collect::<Vec<_>>(),
            [0, 1, 2, 2, 3, 3, 4]
        );
        assert!(
            parse("circuit bad { qubit q; overlay { h q; x q } }")
                .expect_err("one Quill cell cannot hold two gates")
                .message
                .contains("cannot share wire")
        );
    }

    #[test]
    fn renders_parity_markers() {
        let circuit = parse(include_str!("../docs/manual/examples/gates-controls.qrab"))
            .expect("valid parity fixture");
        for target in [Target::Latex, Target::Typst, Target::Svg] {
            assert!(render(&circuit, target).contains("par"), "{target:?}");
        }
    }

    #[test]
    fn renders_math_labels() {
        let circuit =
            parse(include_str!("../examples/math-labels.qrab")).expect("valid math-label fixture");
        let [latex, typst, svg, quirk] = [Target::Latex, Target::Typst, Target::Svg, Target::Quirk]
            .map(|target| render(&circuit, target));
        assert!(
            latex.contains(r"\texttt{input }\(\lvert\psi\rangle\)") && latex.contains(r"\(U_f\)")
        );
        assert!(
            typst.contains("@preview/mitex:0.2.7") && typst.contains(r#"qrab-math(raw("U_f"))"#)
        );
        assert!(svg.contains("<g transform=\"translate(") && svg.contains("<path"));
        assert!(quirk.contains("U_f") && !quirk.contains("$U_f$"));

        let mut vertical = circuit.clone();
        vertical.layout.orientation = Orientation::Vertical;
        let typst = render(&vertical, Target::Typst);
        assert!(typst.contains("qrab-math(body) = rotate(-90deg"));
        assert!(typst.contains("parts.rev().join()"));

        let dollars = parse(r#"circuit dollars { qubit q; label "cost \$5" on q }"#)
            .expect("valid escaped dollar");
        assert!(render(&dollars, Target::Svg).contains(">cost $5</text>"));

        let empty = parse(r#"circuit empty { qubit q; label "a$$b" on q }"#)
            .expect("valid empty math span");
        assert!(render(&empty, Target::Typst).contains("qrab-math(raw(\"\"))"));
    }
}
