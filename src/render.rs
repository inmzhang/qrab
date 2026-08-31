use std::fmt::Write as _;

use crate::ast::{Circuit, Control, OperationKind, Orientation, Shape, Style, Wire, WireKind};

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
    output.push_str("\\usetikzlibrary{shapes.geometric}\n");
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
    if circuit.layout.background != "white" {
        writeln!(
            output,
            "  \\fill[{}] ({:.3},{:.3}) rectangle ({:.3},1);",
            circuit.layout.background,
            -circuit.layout.column_gap,
            -(circuit.wires.len() as f32) * circuit.layout.wire_gap,
            end_x + circuit.layout.column_gap
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
                    circuit.layout.wire_gap,
                    label,
                    &targets,
                    &controls,
                    operation.style,
                );
            }
            OperationKind::Measure { targets, label } => {
                for target in targets {
                    draw_latex_measurement(
                        &mut output,
                        x,
                        circuit.layout.wire_gap,
                        operation.positions[*target],
                        label.as_deref(),
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
            OperationKind::WireChange { .. } => {}
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
            OperationKind::Label { wires, label } => {
                let mut rows = expanded_wires(wires, circuit.wires.len())
                    .iter()
                    .map(|wire| operation.positions[*wire])
                    .collect::<Vec<_>>();
                rows.sort_unstable();
                let first = *rows.first().expect("circuit has a wire");
                let last = *rows.last().expect("circuit has a wire");
                let y = -((first + last) as f32) * circuit.layout.wire_gap / 2.0;
                writeln!(
                    output,
                    "  \\node{} at ({x:.3},{y:.3}) {{{}}};",
                    latex_label_options(operation.style),
                    latex_text(label)
                )
                .expect("writing to a String cannot fail");
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
        }
    }

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
        OperationKind::WireChange { wires, kind }
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
            );
            row = next_row;
            start_x = x + half_width;
        }

        let transition = match operation.kind {
            OperationKind::Measure { targets, .. } if targets.contains(&wire_index) => {
                Some((x + circuit.layout.column_gap.min(0.34), WireKind::Classical))
            }
            OperationKind::WireChange { wires, kind }
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
    if initial_kind != WireKind::Hidden {
        let input = wire.input.as_deref().unwrap_or(&wire.name);
        let input_y = -(wire_index as f32) * circuit.layout.wire_gap;
        writeln!(
            output,
            "  \\node[anchor=east] at (0,{input_y:.3}) {{{}}};",
            latex_text(input)
        )
        .expect("writing to a String cannot fail");
    }
    if kind != WireKind::Hidden
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
) {
    let offsets: &[f32] = match kind {
        WireKind::Quantum => &[0.0],
        WireKind::Classical => &[-0.035, 0.035],
        WireKind::Hidden => return,
    };
    for offset in offsets {
        writeln!(
            output,
            "  \\draw{} ({start_x:.3},{:.3}) .. controls ({control_x:.3},{:.3}) and ({control_x:.3},{:.3}) .. ({end_x:.3},{:.3});",
            latex_line_options(style),
            source_y + offset,
            source_y + offset,
            destination_y + offset,
            destination_y + offset
        )
        .expect("writing to a String cannot fail");
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

fn includes_wire(wires: &[usize], wire: usize, wire_count: usize) -> bool {
    (wires.is_empty() && wire < wire_count) || wires.contains(&wire)
}

fn expanded_wires(wires: &[usize], wire_count: usize) -> Vec<usize> {
    let mut expanded = if wires.is_empty() {
        (0..wire_count).collect()
    } else {
        wires.to_vec()
    };
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
    wire_gap: f32,
    label: &str,
    targets: &[usize],
    controls: &[Control],
    style: &Style,
) {
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
            || format!("{:.3}cm", (last - first) as f32 * wire_gap + 0.72),
            |height| format!("{height:.3}pt"),
        );
        writeln!(
            output,
            "  \\node{} at ({x:.3},{midpoint:.3}) {{{}}};",
            latex_node_options(style, "10mm", &height),
            latex_text(label)
        )
        .expect("writing to a String cannot fail");
        return;
    }

    let y = -(targets[0] as f32) * wire_gap;
    if !controls.is_empty() && label == "X" {
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
    } else if !controls.is_empty() && label == "Z" {
        writeln!(
            output,
            "  \\draw{} ({x:.3},{y:.3}) circle[radius=2.2pt];",
            latex_circle_options(style, true)
        )
        .expect("writing to a String cannot fail");
    } else {
        writeln!(
            output,
            "  \\node{} at ({x:.3},{y:.3}) {{{}}};",
            latex_node_options(style, "8mm", "7mm"),
            latex_text(label)
        )
        .expect("writing to a String cannot fail");
    }
}

fn draw_latex_measurement(
    output: &mut String,
    x: f32,
    wire_gap: f32,
    target: usize,
    label: Option<&str>,
    style: &Style,
) {
    let y = -(target as f32) * wire_gap;
    writeln!(
        output,
        "  \\draw{} ({:.3},{:.3}) rectangle ({:.3},{:.3});",
        latex_circle_options(style, false),
        x - 0.34,
        y - 0.28,
        x + 0.34,
        y + 0.28
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
    if let Some(label) = label {
        writeln!(
            output,
            "  \\node[anchor=south west,font=\\scriptsize] at ({:.3},{:.3}) {{{}}};",
            x + 0.26,
            y + 0.18,
            latex_text(label)
        )
        .expect("writing to a String cannot fail");
    }
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
        options.push(format!("color={stroke}"));
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
        options.push(format!("color={stroke}"));
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
    let mut options = vec![format!("draw={stroke}"), format!("fill={fill}")];
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
            format!("draw={}", style.stroke.as_deref().unwrap_or("black")),
            format!("fill={}", style.fill.as_deref().unwrap_or("white")),
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
        options.push(format!("text={stroke}"));
    }
    if let Some(fill) = &style.fill {
        options.push(format!("fill={fill}"));
    }
    match style.shape {
        Some(Shape::Box) => options.push(format!(
            "draw={}",
            style.stroke.as_deref().unwrap_or("black")
        )),
        Some(Shape::Circle) => {
            options.push("circle".into());
            options.push(format!(
                "draw={}",
                style.stroke.as_deref().unwrap_or("black")
            ));
        }
        Some(Shape::Ellipse) => {
            options.push("ellipse".into());
            options.push(format!(
                "draw={}",
                style.stroke.as_deref().unwrap_or("black")
            ));
        }
        Some(Shape::None) | None => {}
    }
    if let Some(opacity) = style.opacity {
        options.push(format!("opacity={opacity:.3}"));
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
        "  fill: {},",
        typst_color(&circuit.layout.background, None)
    )
    .expect("writing to a String cannot fail");

    for (wire_index, wire) in circuit.wires.iter().enumerate() {
        let (initial_kind, transitions) = wire_transitions(circuit, &scheduled, wire_index);
        let count = wire_count(initial_kind);
        let stroke = typst_stroke(&wire.style);
        if count != 1 || stroke.is_some() {
            let stroke = stroke.map_or_else(String::new, |value| format!(", stroke: {value}"));
            writeln!(
                output,
                "  quill.setwire({count}{stroke}, x: 0, y: {wire_index}),"
            )
            .expect("writing to a String cannot fail");
        }
        if initial_kind != WireKind::Hidden {
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
        if final_kind != WireKind::Hidden
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
            OperationKind::Measure { targets, label } => {
                for target in targets {
                    let row = operation.positions[*target];
                    let label_argument = label.as_ref().map_or_else(String::new, |value| {
                        format!(", label: text(\"{}\")", typst_string(value))
                    });
                    writeln!(
                        output,
                        "  quill.meter(x: {x}, y: {row}{label_argument}{}),",
                        typst_measure_style(operation.style)
                    )
                    .expect("writing to a String cannot fail");
                    writeln!(output, "  quill.setwire(2, x: {}, y: {row}),", x + 1)
                        .expect("writing to a String cannot fail");
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
            OperationKind::WireChange { wires, kind } => {
                for wire in expanded_wires(wires, circuit.wires.len()) {
                    let row = operation.positions[wire];
                    writeln!(
                        output,
                        "  quill.setwire({}{}, x: {x}, y: {row}),",
                        wire_count(*kind),
                        typst_setwire_style(operation.style)
                    )
                    .expect("writing to a String cannot fail");
                }
            }
            OperationKind::Endpoint {
                wires,
                start,
                label,
            } => {
                for wire in expanded_wires(wires, circuit.wires.len()) {
                    let row = operation.positions[wire];
                    let count = if *start {
                        wire_count(circuit.wires[wire].kind)
                    } else {
                        0
                    };
                    writeln!(
                        output,
                        "  quill.setwire({count}{}, x: {x}, y: {row}),",
                        typst_setwire_style(operation.style)
                    )
                    .expect("writing to a String cannot fail");
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
            OperationKind::Label { wires, label } => {
                let mut rows = expanded_wires(wires, circuit.wires.len())
                    .iter()
                    .map(|wire| operation.positions[*wire])
                    .collect::<Vec<_>>();
                rows.sort_unstable();
                let first = *rows.first().expect("circuit has a wire");
                let last = *rows.last().expect("circuit has a wire");
                writeln!(
                    output,
                    "  quill.midstick(text(\"{}\"), n: {}, x: {x}, y: {first}{}),",
                    typst_string(label),
                    last - first + 1,
                    typst_label_style(operation.style)
                )
                .expect("writing to a String cannot fail");
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

                for wire in span_wires {
                    let row = if wires.contains(wire) {
                        permuted_row(*wire, wires, &operation.positions)
                    } else {
                        operation.positions[*wire]
                    };
                    let count = wire_count(wire_kind_before(
                        circuit,
                        &scheduled,
                        operation_index,
                        *wire,
                    ));
                    let stroke =
                        typst_stroke(&circuit.wires[*wire].style).unwrap_or_else(|| "black".into());
                    writeln!(
                        output,
                        "  quill.setwire({count}, stroke: {stroke}, x: {}, y: {row}),",
                        x + 1
                    )
                    .expect("writing to a String cannot fail");
                }
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
    output
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
    if !controls.is_empty() && label == "X" {
        writeln!(
            output,
            "  quill.targ(x: {x}, y: {target}{}),",
            typst_control_style(style)
        )
        .expect("writing to a String cannot fail");
    } else if !controls.is_empty() && label == "Z" {
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

fn typst_gate_body(label: &str, style: &Style) -> String {
    let text = format!("text(\"{}\")", typst_string(label));
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

fn typst_setwire_style(style: &Style) -> String {
    typst_stroke(style).map_or_else(String::new, |stroke| format!(", stroke: {stroke}"))
}

fn typst_label_style(style: &Style) -> String {
    style.fill.as_ref().map_or_else(String::new, |fill| {
        format!(", fill: {}", typst_color(fill, style.opacity))
    })
}

fn typst_permute_style(style: &Style, wire_counts: &[usize], wire_strokes: &[String]) -> String {
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
}
