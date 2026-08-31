use std::{collections::BTreeSet, fmt::Write as _};

use crate::ast::{
    BraceSide, Circuit, Control, Group, Layout, MeasurementShape, OperationKind, Orientation,
    Shape, Style, Wire, WireKind,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Latex,
    Typst,
}

pub fn render(circuit: &Circuit, target: Target) -> String {
    match target {
        Target::Latex => render_latex(circuit),
        Target::Typst => render_typst(circuit),
    }
}

#[derive(Debug, Clone)]
struct Scheduled<'a> {
    kind: &'a OperationKind,
    style: &'a Style,
    column: usize,
    positions: Vec<usize>,
    first: usize,
    last: usize,
}

fn schedule(circuit: &Circuit) -> (Vec<Scheduled<'_>>, Vec<usize>) {
    let mut tracks = vec![0_usize; circuit.wires.len()];
    let mut order = (0..circuit.wires.len()).collect::<Vec<_>>();
    let mut positions = order.clone();
    let mut scheduled = Vec::with_capacity(circuit.operations.len());
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
        let column = if matches!(operation.kind, OperationKind::Touch { .. }) {
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
        scheduled.push(Scheduled {
            kind: &operation.kind,
            style: &operation.style,
            column,
            positions: positions.clone(),
            first,
            last,
        });
        if let OperationKind::Permute { wires } = &operation.kind {
            let mut rows = wires
                .iter()
                .map(|wire| positions[*wire])
                .collect::<Vec<_>>();
            rows.sort_unstable();
            for (row, wire) in rows.into_iter().zip(wires) {
                order[row] = *wire;
            }
            for (row, wire) in order.iter().enumerate() {
                positions[*wire] = row;
            }
        }
    }
    (scheduled, positions)
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
                    permuted_row(*wire, permutation, &operation.positions)
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

fn render_latex(circuit: &Circuit) -> String {
    let (scheduled, _) = schedule(circuit);
    let last_column = scheduled
        .iter()
        .map(|operation| operation.column)
        .max()
        .unwrap_or(0);
    let end_x = (last_column + 2) as f32 * circuit.layout.column_gap;
    let mut output = String::new();
    output.push_str("\\documentclass[tikz,border=6pt]{standalone}\n");
    output.push_str("\\usepackage{tikz}\n");
    if circuit
        .operations
        .iter()
        .any(|operation| operation.style.link.is_some())
    {
        output.push_str("\\usepackage{hyperref}\n");
    }
    output.push_str("\\usetikzlibrary{decorations.pathreplacing,shapes.geometric}\n");
    for color in circuit_hex_colors(circuit) {
        writeln!(
            output,
            "\\definecolor{{{}}}{{HTML}}{{{}}}",
            latex_color(color),
            &color[1..]
        )
        .expect("writing to a String cannot fail");
    }
    append_raw(&mut output, &circuit.escapes.latex.preamble);
    output.push_str("\\begin{document}\n");
    let rotation = if circuit.layout.orientation == Orientation::Vertical {
        ",rotate=90"
    } else {
        ""
    };
    writeln!(
        output,
        "\\begin{{tikzpicture}}[line cap=round,line join=round,font=\\sffamily,scale={:.3}{rotation}]",
        circuit.layout.scale
    )
    .expect("writing to a String cannot fail");
    writeln!(output, "% circuit: {}", latex_comment(&circuit.name))
        .expect("writing to a String cannot fail");
    append_raw(&mut output, &circuit.escapes.latex.before);
    if circuit.layout.background != "white" {
        writeln!(
            output,
            "  \\fill[{}] ({:.3},{:.3}) rectangle ({:.3},1);",
            latex_color(&circuit.layout.background),
            -circuit.layout.column_gap,
            -(circuit.wires.len() as f32) * circuit.layout.wire_gap,
            end_x + circuit.layout.column_gap
        )
        .expect("writing to a String cannot fail");
    }

    for (group_index, group) in circuit.groups.iter().enumerate() {
        let (first_column, last_column, first_row, last_row) =
            group_bounds(group, &scheduled, circuit.wires.len());
        let left = first_column as f32 * circuit.layout.column_gap - 0.52;
        let right = last_column as f32 * circuit.layout.column_gap + 0.52;
        let top = -(first_row as f32) * circuit.layout.wire_gap + 0.48;
        let bottom = -(last_row as f32) * circuit.layout.wire_gap - 0.48;
        writeln!(
            output,
            "  \\draw{} ({left:.3},{top:.3}) rectangle ({right:.3},{bottom:.3});",
            latex_group_options(&group.style)
        )
        .expect("writing to a String cannot fail");
        writeln!(
            output,
            "  \\node[anchor=south west] at ({left:.3},{:.3}) {{{}}};",
            top + group_index as f32 * 0.24,
            latex_text(&group.label)
        )
        .expect("writing to a String cannot fail");
    }

    for (wire_index, wire) in circuit.wires.iter().enumerate() {
        draw_latex_wire(&mut output, circuit, &scheduled, wire_index, wire, end_x);
    }

    for operation in &scheduled {
        let x = (operation.column + 1) as f32 * circuit.layout.column_gap;
        match operation.kind {
            OperationKind::Gate {
                label,
                targets,
                controls,
            } => {
                let targets = targets
                    .iter()
                    .map(|wire| operation.positions[*wire])
                    .collect::<Vec<_>>();
                let controls = controls
                    .iter()
                    .map(|control| Control {
                        wire: operation.positions[control.wire],
                        positive: control.positive,
                    })
                    .collect::<Vec<_>>();
                draw_latex_gate(
                    &mut output,
                    x,
                    &circuit.layout,
                    label,
                    &targets,
                    &controls,
                    operation.style,
                );
            }
            OperationKind::Measure {
                targets,
                label,
                shape,
            } => {
                for target in targets {
                    draw_latex_measurement(
                        &mut output,
                        x,
                        &circuit.layout,
                        operation.positions[*target],
                        label.as_deref(),
                        *shape,
                        operation.style,
                    );
                }
            }
            OperationKind::Swap { left, right } => {
                let left_y = -(operation.positions[*left] as f32) * circuit.layout.wire_gap;
                let right_y = -(operation.positions[*right] as f32) * circuit.layout.wire_gap;
                writeln!(
                    output,
                    "  \\draw{} ({x:.3},{left_y:.3}) -- ({x:.3},{right_y:.3});",
                    latex_line_options(operation.style)
                )
                .expect("writing to a String cannot fail");
                draw_latex_cross(&mut output, x, left_y, operation.style);
                draw_latex_cross(&mut output, x, right_y, operation.style);
            }
            OperationKind::Barrier { wires } => {
                let (first, last) = (operation.first, operation.last);
                let top = -(first as f32) * circuit.layout.wire_gap + 0.42;
                let bottom = -(last as f32) * circuit.layout.wire_gap - 0.42;
                let mut barrier_style = operation.style.clone();
                barrier_style.dashed = true;
                writeln!(
                    output,
                    "  \\draw{} ({x:.3},{top:.3}) -- ({x:.3},{bottom:.3}); % barrier on {} wire(s)",
                    latex_line_options(&barrier_style),
                    if wires.is_empty() {
                        circuit.wires.len()
                    } else {
                        wires.len()
                    }
                )
                .expect("writing to a String cannot fail");
            }
            OperationKind::WireChange { wires, kind, label } => {
                if let Some(label) = label {
                    for wire in wires {
                        draw_latex_value_transition(
                            &mut output,
                            x,
                            -(operation.positions[*wire] as f32) * circuit.layout.wire_gap,
                            label,
                            *kind,
                            operation.style,
                            &circuit.layout.background,
                        );
                    }
                }
            }
            OperationKind::Endpoint {
                wires,
                start,
                label,
            } => {
                for wire in expanded_wires(wires, circuit.wires.len()) {
                    let y = -(operation.positions[wire] as f32) * circuit.layout.wire_gap;
                    writeln!(
                        output,
                        "  \\draw{} ({x:.3},{:.3}) -- ({x:.3},{:.3});",
                        latex_line_options(operation.style),
                        y - 0.13,
                        y + 0.13
                    )
                    .expect("writing to a String cannot fail");
                    if let Some(label) = label {
                        writeln!(
                            output,
                            "  \\node[anchor={}] at ({x:.3},{y:.3}) {{{}}};",
                            if *start { "east" } else { "west" },
                            latex_text(label)
                        )
                        .expect("writing to a String cannot fail");
                    }
                }
            }
            OperationKind::Label {
                wires,
                label,
                brace,
            } => {
                let mut rows = expanded_wires(wires, circuit.wires.len())
                    .iter()
                    .map(|wire| operation.positions[*wire])
                    .collect::<Vec<_>>();
                rows.sort_unstable();
                let first = *rows.first().expect("circuit has a wire");
                let last = *rows.last().expect("circuit has a wire");
                if let Some(side) = brace {
                    draw_latex_brace(
                        &mut output,
                        x,
                        circuit.layout.wire_gap,
                        first,
                        last,
                        label,
                        *side,
                        operation.style,
                        &circuit.layout.background,
                    );
                } else {
                    let y = -((first + last) as f32) * circuit.layout.wire_gap / 2.0;
                    writeln!(
                        output,
                        "  \\node{} at ({x:.3},{y:.3}) {{{}}};",
                        latex_label_options(operation.style),
                        latex_text(label)
                    )
                    .expect("writing to a String cannot fail");
                }
            }
            OperationKind::Bundle { wire, label } => {
                let y = -(operation.positions[*wire] as f32) * circuit.layout.wire_gap;
                writeln!(
                    output,
                    "  \\draw{} ({:.3},{:.3}) -- ({:.3},{:.3});",
                    latex_line_options(operation.style),
                    x - 0.10,
                    y - 0.15,
                    x + 0.10,
                    y + 0.15
                )
                .expect("writing to a String cannot fail");
                writeln!(
                    output,
                    "  \\node[anchor=south west,font=\\scriptsize] at ({:.3},{:.3}) {{{}}};",
                    x + 0.08,
                    y + 0.08,
                    latex_text(label)
                )
                .expect("writing to a String cannot fail");
            }
            OperationKind::Permute { .. } => {}
            OperationKind::Phantom { .. } => {}
            OperationKind::Touch { .. } => {
                if *operation.style != Style::default() {
                    let (first, last) = (operation.first, operation.last);
                    let top = -(first as f32) * circuit.layout.wire_gap + 0.35;
                    let bottom = -(last as f32) * circuit.layout.wire_gap - 0.35;
                    writeln!(
                        output,
                        "  \\draw{} ({x:.3},{top:.3}) -- ({x:.3},{bottom:.3});",
                        latex_line_options(operation.style)
                    )
                    .expect("writing to a String cannot fail");
                }
            }
            OperationKind::WireLabels { wires, labels } => {
                let wires = selected_wires(wires, circuit.wires.len());
                for (index, wire) in wires.iter().enumerate() {
                    let label = if labels.len() == 1 {
                        &labels[0]
                    } else {
                        &labels[index]
                    };
                    let y = -(operation.positions[*wire] as f32) * circuit.layout.wire_gap;
                    writeln!(
                        output,
                        "  \\node{} at ({x:.3},{y:.3}) {{{}}};",
                        latex_label_options(operation.style),
                        latex_text(label)
                    )
                    .expect("writing to a String cannot fail");
                }
            }
            OperationKind::Brace { wires, label, side } => {
                let mut rows = selected_wires(wires, circuit.wires.len())
                    .iter()
                    .map(|wire| operation.positions[*wire])
                    .collect::<Vec<_>>();
                rows.sort_unstable();
                draw_latex_brace(
                    &mut output,
                    x,
                    circuit.layout.wire_gap,
                    *rows.first().expect("circuit has a wire"),
                    *rows.last().expect("circuit has a wire"),
                    label,
                    *side,
                    operation.style,
                    &circuit.layout.background,
                );
            }
            OperationKind::Note { wires, text } => {
                let mut rows = selected_wires(wires, circuit.wires.len())
                    .iter()
                    .map(|wire| operation.positions[*wire])
                    .collect::<Vec<_>>();
                rows.sort_unstable();
                let midpoint = (*rows.first().expect("circuit has a wire")
                    + *rows.last().expect("circuit has a wire"))
                    as f32
                    * circuit.layout.wire_gap
                    / -2.0;
                writeln!(
                    output,
                    "  \\node[anchor=south,text width={:.3}pt,align=center] at ({x:.3},{:.3}) {{{}}};",
                    circuit.layout.comment_width,
                    midpoint + 0.24,
                    latex_text(text)
                )
                .expect("writing to a String cannot fail");
            }
            OperationKind::Cut { label, .. } => {
                let top = -(operation.first as f32) * circuit.layout.wire_gap + 0.42;
                let bottom = -(operation.last as f32) * circuit.layout.wire_gap - 0.42;
                let mut cut_style = operation.style.clone();
                cut_style.dashed = true;
                writeln!(
                    output,
                    "  \\draw{} ({x:.3},{top:.3}) -- ({x:.3},{bottom:.3});",
                    latex_line_options(&cut_style)
                )
                .expect("writing to a String cannot fail");
                if let Some(label) = label {
                    writeln!(
                        output,
                        "  \\node[anchor=south] at ({x:.3},{top:.3}) {{{}}};",
                        latex_text(label)
                    )
                    .expect("writing to a String cannot fail");
                }
            }
        }
    }

    append_raw(&mut output, &circuit.escapes.latex.after);
    output.push_str("\\end{tikzpicture}\n");
    output.push_str("\\end{document}\n");
    output
}

fn wire_transitions(
    circuit: &Circuit,
    scheduled: &[Scheduled<'_>],
    wire: usize,
) -> (WireKind, Vec<(f32, WireKind)>) {
    let initial = initial_wire_kind(circuit, scheduled, wire);
    let mut transitions = scheduled
        .iter()
        .filter_map(|operation| {
            let x = (operation.column + 1) as f32 * circuit.layout.column_gap;
            wire_kind_transition(circuit, operation.kind, wire).map(|kind| {
                let x = if matches!(operation.kind, OperationKind::Measure { .. }) {
                    x + circuit.layout.column_gap.min(0.34)
                } else {
                    x
                };
                (x, kind)
            })
        })
        .collect::<Vec<_>>();
    transitions.sort_by(|left, right| left.0.total_cmp(&right.0));
    (initial, transitions)
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

fn wire_kind_before(
    circuit: &Circuit,
    scheduled: &[Scheduled<'_>],
    operation_index: usize,
    wire: usize,
) -> WireKind {
    scheduled[..operation_index]
        .iter()
        .rev()
        .filter_map(|operation| wire_kind_transition(circuit, operation.kind, wire))
        .next()
        .unwrap_or_else(|| initial_wire_kind(circuit, scheduled, wire))
}

fn draw_latex_wire(
    output: &mut String,
    circuit: &Circuit,
    scheduled: &[Scheduled<'_>],
    wire_index: usize,
    wire: &Wire,
    end_x: f32,
) {
    let (initial_kind, _) = wire_transitions(circuit, scheduled, wire_index);
    let mut kind = initial_kind;
    let mut row = wire_index;
    let mut start_x = 0.0;

    for operation in scheduled {
        let x = (operation.column + 1) as f32 * circuit.layout.column_gap;
        if let OperationKind::Permute { wires } = operation.kind
            && wires.contains(&wire_index)
        {
            let half_width = operation
                .style
                .width
                .map_or(0.45, |width| width / 56.9)
                .min(circuit.layout.column_gap * 0.45);
            let next_row = permuted_row(wire_index, wires, &operation.positions);
            let source_y = -(row as f32) * circuit.layout.wire_gap;
            let destination_y = -(next_row as f32) * circuit.layout.wire_gap;
            draw_wire_segment(output, kind, start_x, x - half_width, source_y, &wire.style);
            draw_wire_curve(
                output,
                kind,
                x - half_width,
                x,
                x + half_width,
                source_y,
                destination_y,
                &merged_line_style(&wire.style, operation.style),
                circuit.layout.corner_radius,
            );
            row = next_row;
            start_x = x + half_width;
        }

        let transition = match operation.kind {
            OperationKind::Measure { targets, .. } if targets.contains(&wire_index) => {
                Some((x + circuit.layout.column_gap.min(0.34), WireKind::Classical))
            }
            OperationKind::WireChange { wires, kind, .. }
                if includes_wire(wires, wire_index, circuit.wires.len()) =>
            {
                Some((x, *kind))
            }
            OperationKind::Endpoint {
                wires, start: true, ..
            } if includes_wire(wires, wire_index, circuit.wires.len()) => Some((x, wire.kind)),
            OperationKind::Endpoint {
                wires,
                start: false,
                ..
            } if includes_wire(wires, wire_index, circuit.wires.len()) => {
                Some((x, WireKind::Hidden))
            }
            _ => None,
        };
        if let Some((transition_x, next_kind)) = transition {
            let y = -(row as f32) * circuit.layout.wire_gap;
            draw_wire_segment(output, kind, start_x, transition_x, y, &wire.style);
            kind = next_kind;
            start_x = transition_x;
        }
    }

    let y = -(row as f32) * circuit.layout.wire_gap;
    draw_wire_segment(output, kind, start_x, end_x, y, &wire.style);
    if initial_kind != WireKind::Hidden || wire.ellipsis {
        let input = wire.input.as_deref().unwrap_or(&wire.name);
        let input_y = -(wire_index as f32) * circuit.layout.wire_gap;
        writeln!(
            output,
            "  \\node[anchor=east] at (0,{input_y:.3}) {{{}}};",
            latex_text(input)
        )
        .expect("writing to a String cannot fail");
    }
    if (kind != WireKind::Hidden || wire.ellipsis)
        && let Some(label) = &wire.output
    {
        writeln!(
            output,
            "  \\node[anchor=west] at ({end_x:.3},{y:.3}) {{{}}};",
            latex_text(label)
        )
        .expect("writing to a String cannot fail");
    }
}

fn permuted_row(wire: usize, wires: &[usize], positions: &[usize]) -> usize {
    let mut rows = wires
        .iter()
        .map(|wire| positions[*wire])
        .collect::<Vec<_>>();
    rows.sort_unstable();
    rows[wires
        .iter()
        .position(|candidate| *candidate == wire)
        .expect("permutation contains the wire")]
}

#[allow(clippy::too_many_arguments)]
fn draw_wire_curve(
    output: &mut String,
    kind: WireKind,
    start_x: f32,
    control_x: f32,
    end_x: f32,
    source_y: f32,
    destination_y: f32,
    style: &Style,
    corner_radius: f32,
) {
    let offsets: &[f32] = match kind {
        WireKind::Quantum => &[0.0],
        WireKind::Classical => &[-0.035, 0.035],
        WireKind::Hidden => return,
    };
    for offset in offsets {
        if corner_radius == 0.0 {
            writeln!(
                output,
                "  \\draw{} ({start_x:.3},{:.3}) -- ({end_x:.3},{:.3});",
                latex_line_options(style),
                source_y + offset,
                destination_y + offset
            )
            .expect("writing to a String cannot fail");
        } else {
            let bend = (corner_radius / 4.0).min(1.0);
            let first_control = start_x + (control_x - start_x) * bend;
            let second_control = end_x - (end_x - control_x) * bend;
            writeln!(
                output,
                "  \\draw{} ({start_x:.3},{:.3}) .. controls ({first_control:.3},{:.3}) and ({second_control:.3},{:.3}) .. ({end_x:.3},{:.3});",
                latex_line_options(style),
                source_y + offset,
                source_y + offset,
                destination_y + offset,
                destination_y + offset
            )
            .expect("writing to a String cannot fail");
        }
    }
}

fn merged_line_style(base: &Style, overlay: &Style) -> Style {
    Style {
        stroke: overlay.stroke.clone().or_else(|| base.stroke.clone()),
        dashed: base.dashed || overlay.dashed,
        opacity: overlay.opacity.or(base.opacity),
        ..Style::default()
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_latex_brace(
    output: &mut String,
    x: f32,
    wire_gap: f32,
    first: usize,
    last: usize,
    label: &str,
    side: BraceSide,
    style: &Style,
    background: &str,
) {
    let top = -(first as f32) * wire_gap + 0.34;
    let bottom = -(last as f32) * wire_gap - 0.34;
    for (offset, mirror) in match side {
        BraceSide::Left => [Some((-0.22, false)), None],
        BraceSide::Right => [Some((0.22, true)), None],
        BraceSide::Both => [Some((-0.22, false)), Some((0.22, true))],
    }
    .into_iter()
    .flatten()
    {
        let mut options = vec!["decorate".into()];
        options.push(format!(
            "decoration={{brace,amplitude=4pt{}}}",
            if mirror { ",mirror" } else { "" }
        ));
        if let Some(stroke) = &style.stroke {
            options.push(format!("color={}", latex_color(stroke)));
        }
        if let Some(opacity) = style.opacity {
            options.push(format!("opacity={opacity:.3}"));
        }
        writeln!(
            output,
            "  \\draw{} ({:.3},{bottom:.3}) -- ({:.3},{top:.3});",
            latex_options(options),
            x + offset,
            x + offset
        )
        .expect("writing to a String cannot fail");
    }
    let mut label_style = style.clone();
    label_style.fill.get_or_insert_with(|| background.into());
    writeln!(
        output,
        "  \\node{} at ({x:.3},{:.3}) {{{}}};",
        latex_label_options(&label_style),
        (top + bottom) / 2.0,
        latex_text(label)
    )
    .expect("writing to a String cannot fail");
}

fn includes_wire(wires: &[usize], wire: usize, wire_count: usize) -> bool {
    (wires.is_empty() && wire < wire_count) || wires.contains(&wire)
}

fn selected_wires(wires: &[usize], wire_count: usize) -> Vec<usize> {
    if wires.is_empty() {
        (0..wire_count).collect()
    } else {
        wires.to_vec()
    }
}

fn expanded_wires(wires: &[usize], wire_count: usize) -> Vec<usize> {
    let mut expanded = selected_wires(wires, wire_count);
    expanded.sort_unstable();
    expanded
}

fn draw_wire_segment(
    output: &mut String,
    kind: WireKind,
    start_x: f32,
    end_x: f32,
    y: f32,
    style: &Style,
) {
    if end_x <= start_x {
        return;
    }
    match kind {
        WireKind::Quantum => {
            writeln!(
                output,
                "  \\draw{} ({start_x:.3},{y:.3}) -- ({end_x:.3},{y:.3});",
                latex_line_options(style)
            )
            .expect("writing to a String cannot fail");
        }
        WireKind::Classical => draw_classical_wire(output, start_x, end_x, y, style),
        WireKind::Hidden => {}
    }
}

fn draw_classical_wire(output: &mut String, start_x: f32, end_x: f32, y: f32, style: &Style) {
    for offset in [-0.035, 0.035] {
        let line_y = y + offset;
        writeln!(
            output,
            "  \\draw{} ({start_x:.3},{line_y:.3}) -- ({end_x:.3},{line_y:.3});",
            latex_line_options(style)
        )
        .expect("writing to a String cannot fail");
    }
}

fn draw_latex_gate(
    output: &mut String,
    x: f32,
    layout: &Layout,
    label: &str,
    targets: &[usize],
    controls: &[Control],
    style: &Style,
) {
    let wire_gap = layout.wire_gap;
    let gate_size = layout.gate_size;
    if !controls.is_empty() {
        let (first, last) = occupied_bounds(targets, controls);
        writeln!(
            output,
            "  \\draw{} ({x:.3},{:.3}) -- ({x:.3},{:.3});",
            latex_line_options(style),
            -(first as f32) * wire_gap,
            -(last as f32) * wire_gap
        )
        .expect("writing to a String cannot fail");
        for control in controls {
            let y = -(control.wire as f32) * wire_gap;
            writeln!(
                output,
                "  \\draw{} ({x:.3},{y:.3}) circle[radius=2.2pt];",
                latex_circle_options(style, control.positive)
            )
            .expect("writing to a String cannot fail");
        }
    }

    if targets.len() > 1 {
        let first = *targets.iter().min().expect("gate has a target");
        let last = *targets.iter().max().expect("gate has a target");
        let midpoint = -((first + last) as f32) * wire_gap / 2.0;
        let height = style.height.map_or_else(
            || {
                format!(
                    "{:.3}cm",
                    (last - first) as f32 * wire_gap + gate_size / 28.45
                )
            },
            |height| format!("{height:.3}pt"),
        );
        let width = format!("{gate_size:.3}pt");
        writeln!(
            output,
            "  \\node{} at ({x:.3},{midpoint:.3}) {{{}}};",
            latex_node_options(style, &width, &height),
            latex_linked_text(label, style)
        )
        .expect("writing to a String cannot fail");
        return;
    }

    let y = -(targets[0] as f32) * wire_gap;
    if !controls.is_empty() && label == "X" && style.link.is_none() {
        writeln!(
            output,
            "  \\draw{} ({x:.3},{y:.3}) circle[radius=4.0pt];",
            latex_circle_options(style, false)
        )
        .expect("writing to a String cannot fail");
        writeln!(
            output,
            "  \\draw{} ({:.3},{y:.3}) -- ({:.3},{y:.3}) ({x:.3},{:.3}) -- ({x:.3},{:.3});",
            latex_line_options(style),
            x - 0.14,
            x + 0.14,
            y - 0.14,
            y + 0.14
        )
        .expect("writing to a String cannot fail");
    } else if !controls.is_empty() && label == "Z" && style.link.is_none() {
        writeln!(
            output,
            "  \\draw{} ({x:.3},{y:.3}) circle[radius=2.2pt];",
            latex_circle_options(style, true)
        )
        .expect("writing to a String cannot fail");
    } else {
        let size = format!("{gate_size:.3}pt");
        writeln!(
            output,
            "  \\node{} at ({x:.3},{y:.3}) {{{}}};",
            latex_node_options(style, &size, &size),
            latex_linked_text(label, style)
        )
        .expect("writing to a String cannot fail");
    }
}

fn draw_latex_measurement(
    output: &mut String,
    x: f32,
    layout: &Layout,
    target: usize,
    label: Option<&str>,
    shape: MeasurementShape,
    style: &Style,
) {
    let y = -(target as f32) * layout.wire_gap;
    if let Some(label) = label {
        draw_latex_named_measurement(output, x, y, label, shape, style, layout.gate_size);
        return;
    }
    let width = style.width.unwrap_or(layout.gate_size) / 28.45;
    let height = style.height.unwrap_or(layout.gate_size) / 28.45;
    writeln!(
        output,
        "  \\draw{} ({:.3},{:.3}) rectangle ({:.3},{:.3});",
        latex_circle_options(style, false),
        x - width / 2.0,
        y - height / 2.0,
        x + width / 2.0,
        y + height / 2.0
    )
    .expect("writing to a String cannot fail");
    writeln!(
        output,
        "  \\draw{} ({:.3},{:.3}) arc[start angle=180,end angle=0,radius=0.22];",
        latex_line_options(style),
        x - 0.22,
        y + 0.10
    )
    .expect("writing to a String cannot fail");
    writeln!(
        output,
        "  \\draw{} ({x:.3},{:.3}) -- ({:.3},{:.3});",
        latex_arrow_options(style),
        y + 0.10,
        x + 0.17,
        y - 0.12
    )
    .expect("writing to a String cannot fail");
}

fn draw_latex_named_measurement(
    output: &mut String,
    x: f32,
    y: f32,
    label: &str,
    shape: MeasurementShape,
    style: &Style,
    gate_size: f32,
) {
    let width = style.width.unwrap_or(gate_size) / 28.45;
    let height = style.height.unwrap_or(gate_size) / 28.45;
    let left = x - width / 2.0;
    let right = x + width / 2.0;
    let top = y + height / 2.0;
    let bottom = y - height / 2.0;
    match shape {
        MeasurementShape::D => {
            let arc_x = right - height / 2.0;
            writeln!(
                output,
                "  \\draw{} ({left:.3},{bottom:.3}) -- ({arc_x:.3},{bottom:.3}) arc[start angle=-90,end angle=90,radius={:.3}] -- ({left:.3},{top:.3}) -- cycle;",
                latex_circle_options(style, false),
                height / 2.0
            )
            .expect("writing to a String cannot fail");
        }
        MeasurementShape::Tag => {
            let point = (height / 2.0).min(width / 3.0);
            writeln!(
                output,
                "  \\draw{} ({left:.3},{y:.3}) -- ({:.3},{top:.3}) -- ({right:.3},{top:.3}) -- ({right:.3},{bottom:.3}) -- ({:.3},{bottom:.3}) -- cycle;",
                latex_circle_options(style, false),
                left + point,
                left + point
            )
            .expect("writing to a String cannot fail");
        }
    }
    writeln!(
        output,
        "  \\node at ({x:.3},{y:.3}) {{{}}};",
        latex_linked_text(label, style)
    )
    .expect("writing to a String cannot fail");
}

fn draw_latex_value_transition(
    output: &mut String,
    x: f32,
    y: f32,
    label: &str,
    kind: WireKind,
    style: &Style,
    background: &str,
) {
    let width = style.width.map_or(0.48, |width| width / 28.45);
    let height = style.height.map_or(0.34, |height| height / 28.45);
    let left = x - width / 2.0;
    let right = x + width / 2.0;
    let mut fill_options = vec![format!(
        "fill={}",
        latex_color(style.fill.as_deref().unwrap_or(background))
    )];
    if let Some(opacity) = style.opacity {
        fill_options.push(format!("opacity={opacity:.3}"));
    }
    writeln!(
        output,
        "  \\fill{} ({left:.3},{:.3}) rectangle ({right:.3},{:.3});",
        latex_options(fill_options),
        y - height / 2.0,
        y + height / 2.0
    )
    .expect("writing to a String cannot fail");
    let edge = if kind == WireKind::Hidden {
        left
    } else {
        right
    };
    writeln!(
        output,
        "  \\draw{} ({edge:.3},{:.3}) -- ({edge:.3},{:.3});",
        latex_line_options(style),
        y - height / 2.0,
        y + height / 2.0
    )
    .expect("writing to a String cannot fail");
    let mut label_style = style.clone();
    label_style.fill = None;
    label_style.shape = None;
    writeln!(
        output,
        "  \\node{} at ({x:.3},{y:.3}) {{{}}};",
        latex_label_options(&label_style),
        latex_text(label)
    )
    .expect("writing to a String cannot fail");
}

fn draw_latex_cross(output: &mut String, x: f32, y: f32, style: &Style) {
    writeln!(
        output,
        "  \\draw{} ({:.3},{:.3}) -- ({:.3},{:.3}) ({:.3},{:.3}) -- ({:.3},{:.3});",
        latex_line_options(style),
        x - 0.11,
        y - 0.11,
        x + 0.11,
        y + 0.11,
        x - 0.11,
        y + 0.11,
        x + 0.11,
        y - 0.11
    )
    .expect("writing to a String cannot fail");
}

fn latex_line_options(style: &Style) -> String {
    let mut options = Vec::new();
    if let Some(stroke) = &style.stroke {
        options.push(format!("color={}", latex_color(stroke)));
    }
    if style.dashed {
        options.push("dashed".into());
    }
    if let Some(opacity) = style.opacity {
        options.push(format!("opacity={opacity:.3}"));
    }
    latex_options(options)
}

fn latex_arrow_options(style: &Style) -> String {
    let mut options = vec!["->".into()];
    if let Some(stroke) = &style.stroke {
        options.push(format!("color={}", latex_color(stroke)));
    }
    if style.dashed {
        options.push("dashed".into());
    }
    if let Some(opacity) = style.opacity {
        options.push(format!("opacity={opacity:.3}"));
    }
    latex_options(options)
}

fn latex_circle_options(style: &Style, filled: bool) -> String {
    let stroke = style.stroke.as_deref().unwrap_or("black");
    let fill = style
        .fill
        .as_deref()
        .unwrap_or(if filled { stroke } else { "white" });
    let mut options = vec![
        format!("draw={}", latex_color(stroke)),
        format!("fill={}", latex_color(fill)),
    ];
    if style.dashed {
        options.push("dashed".into());
    }
    if let Some(opacity) = style.opacity {
        options.push(format!("opacity={opacity:.3}"));
    }
    latex_options(options)
}

fn latex_node_options(style: &Style, default_width: &str, default_height: &str) -> String {
    let mut options = match style.shape {
        Some(Shape::None) => vec!["draw=none".into(), "fill=none".into()],
        _ => vec![
            format!(
                "draw={}",
                latex_color(style.stroke.as_deref().unwrap_or("black"))
            ),
            format!(
                "fill={}",
                latex_color(style.fill.as_deref().unwrap_or("white"))
            ),
        ],
    };
    match style.shape {
        Some(Shape::Circle) => options.push("circle".into()),
        Some(Shape::Ellipse) => options.push("ellipse".into()),
        Some(Shape::Box | Shape::None) | None => {}
    }
    options.push(format!(
        "minimum width={}",
        style
            .width
            .map_or_else(|| default_width.into(), |width| format!("{width:.3}pt"))
    ));
    options.push(format!(
        "minimum height={}",
        style
            .height
            .map_or_else(|| default_height.into(), |height| format!("{height:.3}pt"))
    ));
    if style.dashed {
        options.push("dashed".into());
    }
    if let Some(opacity) = style.opacity {
        options.push(format!("opacity={opacity:.3}"));
    }
    latex_options(options)
}

fn latex_label_options(style: &Style) -> String {
    let mut options = vec!["inner sep=2pt".into()];
    if let Some(stroke) = &style.stroke {
        options.push(format!("text={}", latex_color(stroke)));
    }
    if let Some(fill) = &style.fill {
        options.push(format!("fill={}", latex_color(fill)));
    }
    match style.shape {
        Some(Shape::Box) => options.push(format!(
            "draw={}",
            latex_color(style.stroke.as_deref().unwrap_or("black"))
        )),
        Some(Shape::Circle) => {
            options.push("circle".into());
            options.push(format!(
                "draw={}",
                latex_color(style.stroke.as_deref().unwrap_or("black"))
            ));
        }
        Some(Shape::Ellipse) => {
            options.push("ellipse".into());
            options.push(format!(
                "draw={}",
                latex_color(style.stroke.as_deref().unwrap_or("black"))
            ));
        }
        Some(Shape::None) | None => {}
    }
    if let Some(opacity) = style.opacity {
        options.push(format!("opacity={opacity:.3}"));
    }
    latex_options(options)
}

fn latex_group_options(style: &Style) -> String {
    let mut options = vec![
        format!(
            "draw={}",
            latex_color(if style.shape == Some(Shape::None) {
                "none"
            } else {
                style.stroke.as_deref().unwrap_or("black")
            })
        ),
        format!(
            "fill={}",
            latex_color(style.fill.as_deref().unwrap_or("none"))
        ),
    ];
    if style.dashed {
        options.push("dashed".into());
    }
    if let Some(opacity) = style.opacity {
        options.push(format!("opacity={opacity:.3}"));
    }
    if matches!(style.shape, Some(Shape::Circle | Shape::Ellipse)) {
        options.push("rounded corners=5pt".into());
    }
    latex_options(options)
}

fn latex_options(options: Vec<String>) -> String {
    if options.is_empty() {
        String::new()
    } else {
        format!("[{}]", options.join(","))
    }
}

fn circuit_hex_colors(circuit: &Circuit) -> BTreeSet<&str> {
    let mut colors = BTreeSet::new();
    if circuit.layout.background.starts_with('#') {
        colors.insert(circuit.layout.background.as_str());
    }
    for style in circuit
        .wires
        .iter()
        .map(|wire| &wire.style)
        .chain(circuit.operations.iter().map(|operation| &operation.style))
        .chain(circuit.groups.iter().map(|group| &group.style))
    {
        for color in [style.stroke.as_deref(), style.fill.as_deref()]
            .into_iter()
            .flatten()
            .filter(|color| color.starts_with('#'))
        {
            colors.insert(color);
        }
    }
    colors
}

fn latex_color(color: &str) -> String {
    color
        .strip_prefix('#')
        .map_or_else(|| color.into(), |hex| format!("qrab{hex}"))
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

fn render_typst(circuit: &Circuit) -> String {
    let (scheduled, final_positions) = schedule(circuit);
    let last_column = scheduled
        .iter()
        .map(|operation| operation.column)
        .max()
        .unwrap_or(0);
    let end_column = last_column + 2;
    let mut output = format!(
        "#set page(width: auto, height: auto, margin: 6pt, fill: {})\n#import \"@preview/quill:0.8.0\" as quill\n\n",
        typst_color(&circuit.layout.background, None)
    );
    append_raw(&mut output, &circuit.escapes.typst.preamble);
    append_raw(&mut output, &circuit.escapes.typst.before);
    if circuit.layout.orientation == Orientation::Vertical {
        output.push_str("#rotate(90deg, reflow: true)[\n");
    }
    output.push_str("#quill.quantum-circuit(\n");
    writeln!(output, "  wires: {},", circuit.wires.len()).expect("writing to a String cannot fail");
    writeln!(
        output,
        "  row-spacing: {:.3}pt,",
        circuit.layout.wire_gap * 12.0
    )
    .expect("writing to a String cannot fail");
    writeln!(
        output,
        "  column-spacing: {:.3}pt,",
        circuit.layout.column_gap * 8.0
    )
    .expect("writing to a String cannot fail");
    writeln!(output, "  scale: {:.3}%,", circuit.layout.scale * 100.0)
        .expect("writing to a String cannot fail");
    writeln!(
        output,
        "  gate-padding: {:.3}pt,",
        ((circuit.layout.gate_size - 10.0) / 2.0).max(0.0)
    )
    .expect("writing to a String cannot fail");
    writeln!(
        output,
        "  fill: {},",
        typst_color(&circuit.layout.background, None)
    )
    .expect("writing to a String cannot fail");

    write_typst_wire_streams(&mut output, circuit, &scheduled, end_column);

    for (group_index, group) in circuit.groups.iter().enumerate() {
        let (first_column, last_column, first_row, last_row) =
            group_bounds(group, &scheduled, circuit.wires.len());
        writeln!(
            output,
            "  quill.gategroup({}, {}, x: {first_column}, y: {first_row}, label: (content: text(\"{}\"), pos: top, dy: -{}pt){}),",
            last_row - first_row + 1,
            last_column - first_column + 1,
            typst_string(&group.label),
            (group_index + 1) * 12,
            typst_group_style(&group.style)
        )
        .expect("writing to a String cannot fail");
    }

    for (wire_index, wire) in circuit.wires.iter().enumerate() {
        let (initial_kind, transitions) = wire_transitions(circuit, &scheduled, wire_index);
        if initial_kind != WireKind::Hidden || wire.ellipsis {
            let input = wire.input.as_deref().unwrap_or(&wire.name);
            writeln!(
                output,
                "  quill.lstick(text(\"{}\"), x: 0, y: {wire_index}),",
                typst_string(input)
            )
            .expect("writing to a String cannot fail");
        }
        let final_kind = transitions
            .last()
            .map_or(initial_kind, |transition| transition.1);
        if (final_kind != WireKind::Hidden || wire.ellipsis)
            && let Some(label) = &wire.output
        {
            writeln!(
                output,
                "  quill.rstick(text(\"{}\"), x: {end_column}, y: {}),",
                typst_string(label),
                final_positions[wire_index]
            )
            .expect("writing to a String cannot fail");
        }
    }

    for (operation_index, operation) in scheduled.iter().enumerate() {
        let x = operation.column + 1;
        match operation.kind {
            OperationKind::Gate {
                label,
                targets,
                controls,
            } => {
                let targets = targets
                    .iter()
                    .map(|wire| operation.positions[*wire])
                    .collect::<Vec<_>>();
                let controls = controls
                    .iter()
                    .map(|control| Control {
                        wire: operation.positions[control.wire],
                        positive: control.positive,
                    })
                    .collect::<Vec<_>>();
                draw_typst_gate(&mut output, x, label, &targets, &controls, operation.style);
            }
            OperationKind::Measure {
                targets,
                label,
                shape,
            } => {
                for target in targets {
                    let row = operation.positions[*target];
                    draw_typst_measurement(
                        &mut output,
                        x,
                        row,
                        label.as_deref(),
                        *shape,
                        operation.style,
                        &circuit.layout,
                    );
                }
            }
            OperationKind::Swap { left, right } => {
                let left = operation.positions[*left];
                let right = operation.positions[*right];
                let distance = right as isize - left as isize;
                writeln!(
                    output,
                    "  quill.swap({distance}, x: {x}, y: {left}{}),",
                    typst_swap_style(operation.style)
                )
                .expect("writing to a String cannot fail");
                writeln!(
                    output,
                    "  quill.swap(x: {x}, y: {right}{}),",
                    typst_swap_style(operation.style)
                )
                .expect("writing to a String cannot fail");
            }
            OperationKind::Barrier { .. } => {
                let (first, last) = (operation.first, operation.last);
                writeln!(
                    output,
                    "  quill.slice(n: {}, x: {x}, y: {first}, stroke: {}),",
                    last - first + 1,
                    typst_barrier_stroke(operation.style)
                )
                .expect("writing to a String cannot fail");
            }
            OperationKind::WireChange { wires, kind, label } => {
                if let Some(label) = label {
                    for wire in wires {
                        draw_typst_value_transition(
                            &mut output,
                            x,
                            operation.positions[*wire],
                            label,
                            *kind,
                            operation.style,
                            &circuit.layout.background,
                        );
                    }
                }
            }
            OperationKind::Endpoint { wires, label, .. } => {
                for wire in expanded_wires(wires, circuit.wires.len()) {
                    let row = operation.positions[wire];
                    if let Some(label) = label {
                        writeln!(
                            output,
                            "  quill.midstick(text(\"{}\"), x: {x}, y: {row}{}),",
                            typst_string(label),
                            typst_label_style(operation.style)
                        )
                        .expect("writing to a String cannot fail");
                    }
                }
            }
            OperationKind::Label {
                wires,
                label,
                brace,
            } => {
                let mut rows = expanded_wires(wires, circuit.wires.len())
                    .iter()
                    .map(|wire| operation.positions[*wire])
                    .collect::<Vec<_>>();
                rows.sort_unstable();
                let first = *rows.first().expect("circuit has a wire");
                let last = *rows.last().expect("circuit has a wire");
                if let Some(side) = brace {
                    draw_typst_brace(
                        &mut output,
                        x,
                        first,
                        last,
                        label,
                        *side,
                        operation.style,
                        &circuit.layout.background,
                    );
                } else {
                    writeln!(
                        output,
                        "  quill.midstick(text(\"{}\"), n: {}, x: {x}, y: {first}{}),",
                        typst_string(label),
                        last - first + 1,
                        typst_label_style(operation.style)
                    )
                    .expect("writing to a String cannot fail");
                }
            }
            OperationKind::Bundle { wire, label } => {
                writeln!(
                    output,
                    "  quill.nwire(text(\"{}\"), x: {x}, y: {}),",
                    typst_string(label),
                    operation.positions[*wire]
                )
                .expect("writing to a String cannot fail");
            }
            OperationKind::Permute { wires } => {
                let (first, last) = (operation.first, operation.last);
                let mut row_wires = vec![0; circuit.wires.len()];
                for (wire, row) in operation.positions.iter().enumerate() {
                    row_wires[*row] = wire;
                }
                let span_wires = &row_wires[first..=last];
                let mut mapping = (0..=last - first).collect::<Vec<_>>();
                let mut sources = wires.clone();
                sources.sort_by_key(|wire| operation.positions[*wire]);
                for (source, destination) in sources.iter().zip(wires) {
                    mapping[operation.positions[*source] - first] =
                        operation.positions[*destination] - first;
                }
                writeln!(
                    output,
                    "  quill.permute({}, x: {x}, y: {first}{}),",
                    mapping
                        .iter()
                        .map(usize::to_string)
                        .collect::<Vec<_>>()
                        .join(", "),
                    typst_permute_style(
                        operation.style,
                        &span_wires
                            .iter()
                            .map(|wire| {
                                wire_count(wire_kind_before(
                                    circuit,
                                    &scheduled,
                                    operation_index,
                                    *wire,
                                ))
                            })
                            .collect::<Vec<_>>(),
                        circuit.layout.corner_radius,
                        &span_wires
                            .iter()
                            .map(|wire| {
                                typst_stroke(&circuit.wires[*wire].style)
                                    .unwrap_or_else(|| "black".into())
                            })
                            .collect::<Vec<_>>()
                    )
                )
                .expect("writing to a String cannot fail");
            }
            OperationKind::Phantom { wires } => {
                for wire in expanded_wires(wires, circuit.wires.len()) {
                    let row = operation.positions[wire];
                    writeln!(
                        output,
                        "  quill.phantom(x: {x}, y: {row}, width: {:.3}pt, height: {:.3}pt),",
                        operation.style.width.unwrap_or(0.0),
                        operation.style.height.unwrap_or(0.0)
                    )
                    .expect("writing to a String cannot fail");
                }
            }
            OperationKind::Touch { .. } => {
                if *operation.style != Style::default() {
                    let (first, last) = (operation.first, operation.last);
                    writeln!(
                        output,
                        "  quill.slice(n: {}, x: {x}, y: {first}, stroke: {}),",
                        last - first + 1,
                        typst_stroke(operation.style).unwrap_or_else(|| "black".into())
                    )
                    .expect("writing to a String cannot fail");
                }
            }
            OperationKind::WireLabels { wires, labels } => {
                let wires = selected_wires(wires, circuit.wires.len());
                for (index, wire) in wires.iter().enumerate() {
                    let label = if labels.len() == 1 {
                        &labels[0]
                    } else {
                        &labels[index]
                    };
                    writeln!(
                        output,
                        "  quill.midstick(text(\"{}\"), x: {x}, y: {}{}),",
                        typst_string(label),
                        operation.positions[*wire],
                        typst_label_style(operation.style)
                    )
                    .expect("writing to a String cannot fail");
                }
            }
            OperationKind::Brace { wires, label, side } => {
                let mut rows = selected_wires(wires, circuit.wires.len())
                    .iter()
                    .map(|wire| operation.positions[*wire])
                    .collect::<Vec<_>>();
                rows.sort_unstable();
                let first = *rows.first().expect("circuit has a wire");
                let last = *rows.last().expect("circuit has a wire");
                draw_typst_brace(
                    &mut output,
                    x,
                    first,
                    last,
                    label,
                    *side,
                    operation.style,
                    &circuit.layout.background,
                );
            }
            OperationKind::Note { wires, text } => {
                let first = selected_wires(wires, circuit.wires.len())
                    .iter()
                    .map(|wire| operation.positions[*wire])
                    .min()
                    .expect("circuit has a wire");
                writeln!(
                    output,
                    "  quill.gate(none, box: false, x: {x}, y: {first}, label: (content: block(width: {:.3}pt, align(center, text(\"{}\"))), pos: top)),",
                    circuit.layout.comment_width,
                    typst_string(text),
                )
                .expect("writing to a String cannot fail");
            }
            OperationKind::Cut { label, .. } => {
                let label = label.as_ref().map_or_else(String::new, |label| {
                    format!(", label: text(\"{}\")", typst_string(label))
                });
                writeln!(
                    output,
                    "  quill.slice(n: {}, x: {x}, y: {}, stroke: {}{label}),",
                    operation.last - operation.first + 1,
                    operation.first,
                    typst_barrier_stroke(operation.style)
                )
                .expect("writing to a String cannot fail");
            }
        }
    }

    if circuit.wires.iter().all(|wire| wire.output.is_none()) {
        writeln!(
            output,
            "  quill.phantom(x: {end_column}, y: {}, width: 0pt, height: 0pt),",
            circuit.wires.len() - 1
        )
        .expect("writing to a String cannot fail");
    }
    output.push_str(")\n");
    if circuit.layout.orientation == Orientation::Vertical {
        output.push_str("]\n");
    }
    append_raw(&mut output, &circuit.escapes.typst.after);
    output
}

fn append_raw(output: &mut String, snippets: &[String]) {
    for snippet in snippets {
        output.push_str(snippet);
        if !snippet.ends_with('\n') {
            output.push('\n');
        }
    }
}

fn write_typst_wire_streams(
    output: &mut String,
    circuit: &Circuit,
    scheduled: &[Scheduled<'_>],
    end_column: usize,
) {
    let mut kinds = (0..circuit.wires.len())
        .map(|wire| initial_wire_kind(circuit, scheduled, wire))
        .collect::<Vec<_>>();
    let strokes = circuit
        .wires
        .iter()
        .map(|wire| typst_stroke(&wire.style).unwrap_or_else(|| "black".into()))
        .collect::<Vec<_>>();
    let mut events = kinds
        .iter()
        .enumerate()
        .map(|(wire, kind)| vec![(0, wire_count(*kind), strokes[wire].clone())])
        .collect::<Vec<_>>();

    for operation in scheduled {
        let column = operation.column + 1;
        for wire in 0..circuit.wires.len() {
            if let Some(kind) = wire_kind_transition(circuit, operation.kind, wire) {
                kinds[wire] = kind;
                events[operation.positions[wire]].push((
                    column,
                    wire_count(kind),
                    strokes[wire].clone(),
                ));
            }
        }
        if let OperationKind::Permute { wires } = operation.kind {
            let mut row_wires = vec![0; circuit.wires.len()];
            for (wire, row) in operation.positions.iter().enumerate() {
                row_wires[*row] = wire;
            }
            for wire in &row_wires[operation.first..=operation.last] {
                let row = if wires.contains(wire) {
                    permuted_row(*wire, wires, &operation.positions)
                } else {
                    operation.positions[*wire]
                };
                events[row].push((column, wire_count(kinds[*wire]), strokes[*wire].clone()));
            }
        }
    }

    let row_count = events.len();
    for (row, row_events) in events.iter_mut().enumerate() {
        row_events.sort_by_key(|event| event.0);
        let (_, count, stroke) = &row_events[0];
        writeln!(output, "  quill.setwire({count}, stroke: {stroke}),")
            .expect("writing to a String cannot fail");
        let mut cursor = 0;
        let mut drew_segment = false;
        for (column, count, stroke) in &row_events[1..] {
            let length = if drew_segment {
                column - cursor
            } else {
                column + 1
            };
            if length > 0 {
                writeln!(output, "  {length},").expect("writing to a String cannot fail");
                drew_segment = true;
                cursor = *column;
            }
            writeln!(output, "  quill.setwire({count}, stroke: {stroke}),")
                .expect("writing to a String cannot fail");
        }
        let remaining = if drew_segment {
            end_column - cursor
        } else {
            end_column + 1
        };
        if remaining > 0 {
            writeln!(output, "  {remaining},").expect("writing to a String cannot fail");
        }
        if row + 1 < row_count {
            output.push_str("  [\\ ],\n");
        }
    }
}

fn draw_typst_gate(
    output: &mut String,
    x: usize,
    label: &str,
    targets: &[usize],
    controls: &[Control],
    style: &Style,
) {
    if !controls.is_empty() {
        let (first, last) = occupied_bounds(targets, controls);
        let anchor = controls
            .iter()
            .find(|control| control.wire == first)
            .or_else(|| controls.iter().find(|control| control.wire == last))
            .or_else(|| controls.first())
            .expect("controlled gate has a control");
        let destination = if anchor.wire - first >= last - anchor.wire {
            first
        } else {
            last
        };
        for control in controls {
            let distance = if control.wire == anchor.wire {
                destination as isize - control.wire as isize
            } else {
                0
            };
            writeln!(
                output,
                "  quill.ctrl({distance}, open: {}, x: {x}, y: {}{}),",
                !control.positive,
                control.wire,
                typst_control_style(style)
            )
            .expect("writing to a String cannot fail");
        }
    }

    if targets.len() > 1 {
        let first = *targets.iter().min().expect("gate has a target");
        let last = *targets.iter().max().expect("gate has a target");
        let pass_through = (first..=last)
            .filter(|wire| !targets.contains(wire))
            .map(|wire| (wire - first).to_string())
            .collect::<Vec<_>>();
        let pass_through = if pass_through.is_empty() {
            String::new()
        } else {
            format!(", pass-through: ({},)", pass_through.join(", "))
        };
        writeln!(
            output,
            "  quill.mqgate({}, n: {}, x: {x}, y: {first}{pass_through}{}),",
            typst_gate_body(label, style),
            last - first + 1,
            typst_gate_style(style)
        )
        .expect("writing to a String cannot fail");
        return;
    }

    let target = targets[0];
    if !controls.is_empty() && label == "X" && style.link.is_none() {
        writeln!(
            output,
            "  quill.targ(x: {x}, y: {target}{}),",
            typst_control_style(style)
        )
        .expect("writing to a String cannot fail");
    } else if !controls.is_empty() && label == "Z" && style.link.is_none() {
        writeln!(
            output,
            "  quill.ctrl(x: {x}, y: {target}{}),",
            typst_control_style(style)
        )
        .expect("writing to a String cannot fail");
    } else {
        writeln!(
            output,
            "  quill.gate({}, x: {x}, y: {target}{}),",
            typst_gate_body(label, style),
            typst_gate_style(style)
        )
        .expect("writing to a String cannot fail");
    }
}

fn draw_typst_measurement(
    output: &mut String,
    x: usize,
    row: usize,
    label: Option<&str>,
    shape: MeasurementShape,
    style: &Style,
    layout: &Layout,
) {
    let Some(label) = label else {
        writeln!(
            output,
            "  quill.meter(x: {x}, y: {row}{}),",
            typst_measure_style(style)
        )
        .expect("writing to a String cannot fail");
        return;
    };

    match shape {
        MeasurementShape::D => {
            let mut gate_style = style.clone();
            gate_style.width.get_or_insert(layout.gate_size);
            writeln!(
                output,
                "  quill.gate({}, x: {x}, y: {row}{}, radius: (top-right: 999pt, bottom-right: 999pt)),",
                typst_gate_body(label, &gate_style),
                typst_gate_style(&gate_style)
            )
            .expect("writing to a String cannot fail");
        }
        MeasurementShape::Tag => {
            let width = style.width.unwrap_or(layout.gate_size);
            writeln!(
                output,
                "  quill.gate({}, x: {x}, y: {row}, box: false, width: {width:.3}pt),",
                typst_measure_tag_body(label, style, &layout.background, layout.gate_size)
            )
            .expect("writing to a String cannot fail");
        }
    }
}

fn typst_measure_tag_body(label: &str, style: &Style, background: &str, gate_size: f32) -> String {
    let width = style.width.unwrap_or(gate_size);
    let height = style.height.unwrap_or(gate_size * 0.7);
    let point = (height / 2.0).min(width / 3.0);
    let fill = typst_color(style.fill.as_deref().unwrap_or(background), style.opacity);
    let stroke = typst_stroke(style).unwrap_or_else(|| "black".into());
    format!(
        "box(width: {width:.3}pt, height: {height:.3}pt, inset: 0pt, [#place(polygon(fill: {fill}, stroke: {stroke}, (0pt, {:.3}pt), ({point:.3}pt, 0pt), ({width:.3}pt, 0pt), ({width:.3}pt, {height:.3}pt), ({point:.3}pt, {height:.3}pt))) #align(center + horizon, {})])",
        height / 2.0,
        typst_linked_text(label, style)
    )
}

fn draw_typst_value_transition(
    output: &mut String,
    x: usize,
    row: usize,
    label: &str,
    kind: WireKind,
    style: &Style,
    background: &str,
) {
    let width = style.width.unwrap_or(18.0);
    writeln!(
        output,
        "  quill.gate({}, x: {x}, y: {row}, box: false, width: {width:.3}pt),",
        typst_value_transition_body(label, kind, style, background)
    )
    .expect("writing to a String cannot fail");
}

fn typst_value_transition_body(
    label: &str,
    kind: WireKind,
    style: &Style,
    background: &str,
) -> String {
    let width = style.width.unwrap_or(18.0);
    let height = style.height.unwrap_or(12.0);
    let edge = if kind == WireKind::Hidden { 0.0 } else { width };
    let fill = typst_color(style.fill.as_deref().unwrap_or(background), style.opacity);
    let stroke = typst_stroke(style).unwrap_or_else(|| "black".into());
    let text_color = typst_color(style.stroke.as_deref().unwrap_or("black"), style.opacity);
    format!(
        "box(width: {width:.3}pt, height: {height:.3}pt, inset: 0pt, fill: {fill}, [#place(line(start: ({edge:.3}pt, 0pt), end: ({edge:.3}pt, {height:.3}pt), stroke: {stroke})) #align(center + horizon, text(fill: {text_color}, \"{}\"))])",
        typst_string(label)
    )
}

fn typst_gate_body(label: &str, style: &Style) -> String {
    let text = typst_linked_text(label, style);
    style.height.map_or(text.clone(), |height| {
        format!("box(height: {height:.3}pt, {text})")
    })
}

fn typst_gate_style(style: &Style) -> String {
    let mut arguments = Vec::new();
    match style.shape {
        Some(Shape::None) => {
            arguments.push("box: false".into());
            arguments.push("fill: none".into());
            arguments.push("stroke: none".into());
        }
        _ => {
            if let Some(fill) = &style.fill {
                arguments.push(format!("fill: {}", typst_color(fill, style.opacity)));
            }
            if let Some(stroke) = typst_stroke(style) {
                arguments.push(format!("stroke: {stroke}"));
            }
        }
    }
    if matches!(style.shape, Some(Shape::Circle | Shape::Ellipse)) {
        arguments.push("radius: 999pt".into());
    }
    if let Some(width) = style.width {
        arguments.push(format!("width: {width:.3}pt"));
    }
    typst_arguments(arguments)
}

fn typst_control_style(style: &Style) -> String {
    let mut arguments = Vec::new();
    if let Some(fill) = &style.fill {
        arguments.push(format!("fill: {}", typst_color(fill, style.opacity)));
    }
    if let Some(stroke) = typst_stroke(style) {
        arguments.push(format!("stroke: {stroke}"));
        arguments.push(format!("wire-stroke: {stroke}"));
    }
    if let Some(size) = style.width {
        arguments.push(format!("size: {:.3}pt", size / 2.0));
    }
    typst_arguments(arguments)
}

fn typst_measure_style(style: &Style) -> String {
    let mut arguments = Vec::new();
    if let Some(fill) = &style.fill {
        arguments.push(format!("fill: {}", typst_color(fill, style.opacity)));
    }
    if let Some(stroke) = typst_stroke(style) {
        arguments.push(format!("stroke: {stroke}"));
        arguments.push(format!("wire-stroke: {stroke}"));
    }
    if matches!(style.shape, Some(Shape::Circle | Shape::Ellipse)) {
        arguments.push("radius: 999pt".into());
    }
    typst_arguments(arguments)
}

fn typst_swap_style(style: &Style) -> String {
    let mut arguments = Vec::new();
    if let Some(stroke) = typst_stroke(style) {
        arguments.push(format!("stroke: {stroke}"));
        arguments.push(format!("wire-stroke: {stroke}"));
    }
    if let Some(size) = style.width {
        arguments.push(format!("size: {size:.3}pt"));
    }
    typst_arguments(arguments)
}

fn typst_barrier_stroke(style: &Style) -> String {
    let paint = style
        .stroke
        .as_deref()
        .map_or_else(|| "black".into(), |color| typst_color(color, style.opacity));
    format!("(paint: {paint}, thickness: 0.7pt, dash: \"dashed\")")
}

fn typst_label_style(style: &Style) -> String {
    style.fill.as_ref().map_or_else(String::new, |fill| {
        format!(", fill: {}", typst_color(fill, style.opacity))
    })
}

fn typst_group_style(style: &Style) -> String {
    let mut arguments = vec!["padding: 3pt".into()];
    arguments.push(format!(
        "stroke: {}",
        if style.shape == Some(Shape::None) {
            "none".into()
        } else {
            typst_stroke(style).unwrap_or_else(|| "black".into())
        }
    ));
    if let Some(fill) = &style.fill {
        arguments.push(format!("fill: {}", typst_color(fill, style.opacity)));
    }
    if matches!(style.shape, Some(Shape::Circle | Shape::Ellipse)) {
        arguments.push("radius: 5pt".into());
    }
    typst_arguments(arguments)
}

#[allow(clippy::too_many_arguments)]
fn draw_typst_brace(
    output: &mut String,
    x: usize,
    first: usize,
    last: usize,
    label: &str,
    side: BraceSide,
    style: &Style,
    background: &str,
) {
    let n = last - first + 1;
    writeln!(
        output,
        "  quill.mqgate({}, n: {n}, x: {x}, y: {first}, fill: {}, stroke: none),",
        typst_brace_body(label, side, n, style),
        typst_color(style.fill.as_deref().unwrap_or(background), style.opacity)
    )
    .expect("writing to a String cannot fail");
}

fn typst_brace_body(label: &str, side: BraceSide, wires: usize, style: &Style) -> String {
    let size = (wires as f32 * 12.0).max(18.0);
    let color = typst_color(style.stroke.as_deref().unwrap_or("black"), style.opacity);
    let left = format!("#text(size: {size:.3}pt, fill: {color}, \"{{\")");
    let right = format!("#text(size: {size:.3}pt, fill: {color}, \"}}\")");
    let label = format!("#text(\"{}\")", typst_string(label));
    let body = match side {
        BraceSide::Left => format!("{left} #h(3pt) {label}"),
        BraceSide::Right => format!("{label} #h(3pt) {right}"),
        BraceSide::Both => format!("{left} #h(3pt) {label} #h(3pt) {right}"),
    };
    format!("box([{body}])")
}

fn typst_permute_style(
    style: &Style,
    wire_counts: &[usize],
    corner_radius: f32,
    wire_strokes: &[String],
) -> String {
    let mut arguments = Vec::new();
    if let Some(width) = style.width {
        arguments.push(format!("width: {width:.3}pt"));
    }
    if let Some(stroke) = typst_stroke(style) {
        arguments.push(format!("stroke: {stroke}"));
    } else {
        arguments.push(format!("stroke: ({})", wire_strokes.join(", ")));
    }
    arguments.push(format!(
        "bend: {:.3}%",
        (corner_radius / 4.0).min(1.0) * 100.0
    ));
    arguments.push(format!(
        "wire-count: ({})",
        wire_counts
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    ));
    typst_arguments(arguments)
}

fn wire_count(kind: WireKind) -> usize {
    match kind {
        WireKind::Quantum => 1,
        WireKind::Classical => 2,
        WireKind::Hidden => 0,
    }
}

fn typst_stroke(style: &Style) -> Option<String> {
    if style.stroke.is_none() && !style.dashed && style.opacity.is_none() {
        return None;
    }
    let paint = style.stroke.as_deref().map_or_else(
        || typst_color("black", style.opacity),
        |color| typst_color(color, style.opacity),
    );
    if style.dashed {
        Some(format!("(paint: {paint}, dash: \"dashed\")"))
    } else {
        Some(paint)
    }
}

fn typst_color(color: &str, opacity: Option<f32>) -> String {
    let base = if color.starts_with('#') {
        format!("rgb(\"{}\")", typst_string(color))
    } else {
        color.into()
    };
    opacity.map_or(base.clone(), |opacity| {
        format!("{base}.transparentize({:.3}%)", (1.0 - opacity) * 100.0)
    })
}

fn typst_arguments(arguments: Vec<String>) -> String {
    if arguments.is_empty() {
        String::new()
    } else {
        format!(", {}", arguments.join(", "))
    }
}

fn latex_text(value: &str) -> String {
    let mut escaped = String::from("\\texttt{");
    for character in value.chars() {
        escaped.push_str(match character {
            '\\' => "\\textbackslash{}",
            '{' => "\\{",
            '}' => "\\}",
            '#' => "\\#",
            '$' => "\\$",
            '%' => "\\%",
            '&' => "\\&",
            '_' => "\\_",
            '^' => "\\textasciicircum{}",
            '~' => "\\textasciitilde{}",
            '\n' | '\r' => " ",
            _ => {
                escaped.push(character);
                continue;
            }
        });
    }
    escaped.push('}');
    escaped
}

fn latex_linked_text(value: &str, style: &Style) -> String {
    let text = latex_text(value);
    style.link.as_ref().map_or_else(
        || text.clone(),
        |link| format!("\\href{{{}}}{{{text}}}", latex_url(link)),
    )
}

fn latex_url(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        escaped.push_str(match character {
            '#' => "\\#",
            '$' => "\\$",
            '%' => "\\%",
            '&' => "\\&",
            '_' => "\\_",
            '~' => "\\string~",
            _ => {
                escaped.push(character);
                continue;
            }
        });
    }
    escaped
}

fn latex_comment(value: &str) -> String {
    value.replace(['\n', '\r'], " ").replace('%', "")
}

fn typst_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn typst_linked_text(value: &str, style: &Style) -> String {
    let text = format!("text(\"{}\")", typst_string(value));
    style.link.as_ref().map_or(text.clone(), |link| {
        format!("link(\"{}\", {text})", typst_string(link))
    })
}

#[cfg(test)]
mod tests {
    use crate::parse;

    use super::*;

    const BELL: &str = r#"
        circuit bell {
          qubit q[2]: "|0>" -> "bell"
          h q[0]
          x q[1] if q[0]
          measure q[0], q[1]
        }
    "#;

    #[test]
    fn emits_tikz_and_quill_from_the_same_circuit() {
        let circuit = parse(BELL).expect("valid circuit");
        let latex = render(&circuit, Target::Latex);
        let typst = render(&circuit, Target::Typst);

        assert!(latex.contains("\\begin{tikzpicture}"));
        assert!(latex.contains("circle[radius=4.0pt]"));
        assert!(typst.contains("@preview/quill:0.8.0"));
        assert!(typst.contains("quill.targ"));
    }

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

        assert_eq!(scheduled[1].positions[2], 0);
        assert_eq!(final_positions, vec![1, 2, 0]);
        assert!(render_typst(&circuit).contains("quill.gate(text(\"H\"), x: 2, y: 0)"));
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
    fn quill_wire_changes_stay_on_their_physical_row() {
        let circuit = parse(
            r#"
                circuit measured_second_wire {
                  qubit q[2]
                  measure q[1]
                }
            "#,
        )
        .expect("valid measurement");
        let typst = render_typst(&circuit);
        let rows = typst.split("[\\ ],").collect::<Vec<_>>();

        assert!(!rows[0].contains("quill.setwire(2"));
        assert!(rows[1].contains("quill.setwire(2, stroke: black)"));
        assert!(!typst.contains("setwire(2, x:"));
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
}
