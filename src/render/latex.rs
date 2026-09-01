use std::{collections::BTreeSet, fmt::Write as _};

use super::*;

pub(super) fn render_latex(circuit: &Circuit) -> String {
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
        emit!(
            output,
            "\\definecolor{{{}}}{{HTML}}{{{}}}",
            latex_color(color),
            &color[1..]
        );
    }
    append_raw(&mut output, &circuit.escapes.latex.preamble);
    output.push_str("\\begin{document}\n");
    let rotation = if circuit.layout.orientation == Orientation::Vertical {
        ",rotate=90"
    } else {
        ""
    };
    emit!(
        output,
        "\\begin{{tikzpicture}}[line cap=round,line join=round,font=\\sffamily,scale={:.3}{rotation}]",
        circuit.layout.scale
    );
    emit!(output, "% circuit: {}", latex_comment(&circuit.name));
    append_raw(&mut output, &circuit.escapes.latex.before);
    if circuit.layout.background != "white" {
        emit!(
            output,
            "  \\fill[{}] ({:.3},{:.3}) rectangle ({:.3},{:.3});",
            latex_color(&circuit.layout.background),
            -circuit.layout.column_gap,
            -(circuit.wires.len() as f32) * circuit.layout.wire_gap,
            end_x + circuit.layout.column_gap,
            circuit.layout.wire_gap
        );
    }

    for (group_index, group) in circuit.groups.iter().enumerate() {
        let (first_column, last_column, first_row, last_row) =
            group_bounds(group, &scheduled, circuit.wires.len());
        let left = first_column as f32 * circuit.layout.column_gap - 0.52;
        let right = last_column as f32 * circuit.layout.column_gap + 0.52;
        let top = -(first_row as f32) * circuit.layout.wire_gap + 0.48;
        let bottom = -(last_row as f32) * circuit.layout.wire_gap - 0.48;
        emit!(
            output,
            "  \\draw{} ({left:.3},{top:.3}) rectangle ({right:.3},{bottom:.3});",
            latex_group_options(&group.style)
        );
        emit!(
            output,
            "  \\node[anchor=south west] at ({left:.3},{:.3}) {{{}}};",
            top + group_index as f32 * 0.24,
            latex_text(&group.label)
        );
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
                emit!(
                    output,
                    "  \\draw{} ({x:.3},{left_y:.3}) -- ({x:.3},{right_y:.3});",
                    latex_line_options(operation.style)
                );
                draw_latex_cross(&mut output, x, left_y, operation.style);
                draw_latex_cross(&mut output, x, right_y, operation.style);
            }
            OperationKind::Barrier { wires } => {
                let (first, last) = (operation.first, operation.last);
                let top = -(first as f32) * circuit.layout.wire_gap + 0.42;
                let bottom = -(last as f32) * circuit.layout.wire_gap - 0.42;
                let mut barrier_style = operation.style.clone();
                barrier_style.dashed = true;
                emit!(
                    output,
                    "  \\draw{} ({x:.3},{top:.3}) -- ({x:.3},{bottom:.3}); % barrier on {} wire(s)",
                    latex_line_options(&barrier_style),
                    if wires.is_empty() {
                        circuit.wires.len()
                    } else {
                        wires.len()
                    }
                );
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
                    emit!(
                        output,
                        "  \\draw{} ({x:.3},{:.3}) -- ({x:.3},{:.3});",
                        latex_line_options(operation.style),
                        y - 0.13,
                        y + 0.13
                    );
                    if let Some(label) = label {
                        emit!(
                            output,
                            "  \\node{} at ({x:.3},{y:.3}) {{{}}};",
                            latex_endpoint_label_options(operation.style, *start),
                            latex_linked_text(label, operation.style)
                        );
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
                    emit!(
                        output,
                        "  \\node{} at ({x:.3},{y:.3}) {{{}}};",
                        latex_label_options(operation.style),
                        latex_linked_text(label, operation.style)
                    );
                }
            }
            OperationKind::Bundle { wire, label } => {
                let y = -(operation.positions[*wire] as f32) * circuit.layout.wire_gap;
                emit!(
                    output,
                    "  \\draw{} ({:.3},{:.3}) -- ({:.3},{:.3});",
                    latex_line_options(operation.style),
                    x - 0.10,
                    y - 0.15,
                    x + 0.10,
                    y + 0.15
                );
                emit!(
                    output,
                    "  \\node[anchor=south west,font=\\scriptsize] at ({:.3},{:.3}) {{{}}};",
                    x + 0.08,
                    y + 0.08,
                    latex_text(label)
                );
            }
            OperationKind::Permute { .. } => {}
            OperationKind::Phantom { wires } => {
                if operation.style.width.is_some() || operation.style.height.is_some() {
                    for wire in expanded_wires(wires, circuit.wires.len()) {
                        let y = -(operation.positions[wire] as f32) * circuit.layout.wire_gap;
                        emit!(
                            output,
                            "  \\node[inner sep=0pt,minimum width={:.3}pt,minimum height={:.3}pt] at ({x:.3},{y:.3}) {{}};",
                            operation.style.width.unwrap_or(0.0),
                            operation.style.height.unwrap_or(0.0)
                        );
                    }
                }
            }
            OperationKind::Touch { .. } => {
                if has_line_style(operation.style) {
                    let (first, last) = (operation.first, operation.last);
                    let top = -(first as f32) * circuit.layout.wire_gap + 0.35;
                    let bottom = -(last as f32) * circuit.layout.wire_gap - 0.35;
                    emit!(
                        output,
                        "  \\draw{} ({x:.3},{top:.3}) -- ({x:.3},{bottom:.3});",
                        latex_line_options(operation.style)
                    );
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
                    emit!(
                        output,
                        "  \\node{} at ({x:.3},{y:.3}) {{{}}};",
                        latex_label_options(operation.style),
                        latex_linked_text(label, operation.style)
                    );
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
            OperationKind::Note { wires, text, side } => {
                let mut rows = selected_wires(wires, circuit.wires.len())
                    .iter()
                    .map(|wire| operation.positions[*wire])
                    .collect::<Vec<_>>();
                rows.sort_unstable();
                let row = if *side == NoteSide::Above {
                    *rows.first().expect("circuit has a wire") as f32
                } else {
                    *rows.last().expect("circuit has a wire") as f32
                };
                let y = -row * circuit.layout.wire_gap
                    + if *side == NoteSide::Above {
                        0.42
                    } else {
                        -0.42
                    };
                emit!(
                    output,
                    "  \\node[anchor={},text width={:.3}pt,align=center] at ({x:.3},{:.3}) {{{}}};",
                    if *side == NoteSide::Above {
                        "south"
                    } else {
                        "north"
                    },
                    circuit.layout.comment_width,
                    y,
                    latex_text(text)
                );
            }
            OperationKind::Cut { label, .. } => {
                let top = -(operation.first as f32) * circuit.layout.wire_gap + 0.42;
                let bottom = -(operation.last as f32) * circuit.layout.wire_gap - 0.42;
                let mut cut_style = operation.style.clone();
                cut_style.dashed = true;
                emit!(
                    output,
                    "  \\draw{} ({x:.3},{top:.3}) -- ({x:.3},{bottom:.3});",
                    latex_line_options(&cut_style)
                );
                if let Some(label) = label {
                    emit!(
                        output,
                        "  \\node[anchor=south] at ({x:.3},{top:.3}) {{{}}};",
                        latex_text(label)
                    );
                }
            }
        }
    }

    append_raw(&mut output, &circuit.escapes.latex.after);
    output.push_str("\\end{tikzpicture}\n");
    output.push_str("\\end{document}\n");
    output
}

fn draw_latex_wire(
    output: &mut String,
    circuit: &Circuit,
    scheduled: &[Scheduled<'_>],
    wire_index: usize,
    wire: &Wire,
    end_x: f32,
) {
    let initial_kind = initial_wire_kind(circuit, scheduled, wire_index);
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
                .map_or(0.45, |width| width / (2.0 * POINTS_PER_CENTIMETER))
                .min(circuit.layout.column_gap * 0.45);
            let next_row = operation.permuted_row(wire_index);
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

        if let Some((transition_x, next_kind)) = wire_transition(circuit, operation, wire_index) {
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
        emit!(
            output,
            "  \\node[anchor=east] at (0,{input_y:.3}) {{{}}};",
            latex_text(input)
        );
    }
    if (kind != WireKind::Hidden || wire.ellipsis)
        && let Some(label) = &wire.output
    {
        emit!(
            output,
            "  \\node[anchor=west] at ({end_x:.3},{y:.3}) {{{}}};",
            latex_text(label)
        );
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
        emit!(
            output,
            "  \\draw{} ({:.3},{bottom:.3}) -- ({:.3},{top:.3});",
            latex_options(options),
            x + offset,
            x + offset
        );
    }
    let mut label_style = style.clone();
    label_style.fill.get_or_insert_with(|| background.into());
    emit!(
        output,
        "  \\node{} at ({x:.3},{:.3}) {{{}}};",
        latex_label_options(&label_style),
        (top + bottom) / 2.0,
        latex_linked_text(label, style)
    );
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
        emit!(
            output,
            "  \\draw{} ({x:.3},{:.3}) -- ({x:.3},{:.3});",
            latex_line_options(style),
            -(first as f32) * wire_gap,
            -(last as f32) * wire_gap
        );
        for control in controls {
            let y = -(control.wire as f32) * wire_gap;
            emit!(
                output,
                "  \\draw{} ({x:.3},{y:.3}) circle[radius=2.2pt];",
                latex_circle_options(style, control.positive)
            );
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
                    (last - first) as f32 * wire_gap + gate_size / POINTS_PER_CENTIMETER
                )
            },
            |height| format!("{height:.3}pt"),
        );
        let width = format!("{gate_size:.3}pt");
        emit!(
            output,
            "  \\node{} at ({x:.3},{midpoint:.3}) {{{}}};",
            latex_node_options(style, &width, &height),
            latex_linked_text(label, style)
        );
        return;
    }

    let y = -(targets[0] as f32) * wire_gap;
    if !controls.is_empty() && label == "X" && style.link.is_none() {
        emit!(
            output,
            "  \\draw{} ({x:.3},{y:.3}) circle[radius=4.0pt];",
            latex_circle_options(style, false)
        );
        emit!(
            output,
            "  \\draw{} ({:.3},{y:.3}) -- ({:.3},{y:.3}) ({x:.3},{:.3}) -- ({x:.3},{:.3});",
            latex_line_options(style),
            x - 0.14,
            x + 0.14,
            y - 0.14,
            y + 0.14
        );
    } else if !controls.is_empty() && label == "Z" && style.link.is_none() {
        emit!(
            output,
            "  \\draw{} ({x:.3},{y:.3}) circle[radius=2.2pt];",
            latex_circle_options(style, true)
        );
    } else {
        let size = format!("{gate_size:.3}pt");
        emit!(
            output,
            "  \\node{} at ({x:.3},{y:.3}) {{{}}};",
            latex_node_options(style, &size, &size),
            latex_linked_text(label, style)
        );
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
    let width = style.width.unwrap_or(layout.gate_size) / POINTS_PER_CENTIMETER;
    let height = style.height.unwrap_or(layout.gate_size) / POINTS_PER_CENTIMETER;
    emit!(
        output,
        "  \\draw{} ({:.3},{:.3}) rectangle ({:.3},{:.3});",
        latex_circle_options(style, false),
        x - width / 2.0,
        y - height / 2.0,
        x + width / 2.0,
        y + height / 2.0
    );
    emit!(
        output,
        "  \\draw{} ({:.3},{:.3}) arc[start angle=180,end angle=0,radius=0.22];",
        latex_line_options(style),
        x - 0.22,
        y + 0.10
    );
    emit!(
        output,
        "  \\draw{} ({x:.3},{:.3}) -- ({:.3},{:.3});",
        latex_arrow_options(style),
        y + 0.10,
        x + 0.17,
        y - 0.12
    );
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
    let width = style.width.unwrap_or(gate_size) / POINTS_PER_CENTIMETER;
    let height = style.height.unwrap_or(gate_size) / POINTS_PER_CENTIMETER;
    let left = x - width / 2.0;
    let right = x + width / 2.0;
    let top = y + height / 2.0;
    let bottom = y - height / 2.0;
    match shape {
        MeasurementShape::D => {
            let arc_x = right - height / 2.0;
            emit!(
                output,
                "  \\draw{} ({left:.3},{bottom:.3}) -- ({arc_x:.3},{bottom:.3}) arc[start angle=-90,end angle=90,radius={:.3}] -- ({left:.3},{top:.3}) -- cycle;",
                latex_circle_options(style, false),
                height / 2.0
            );
        }
        MeasurementShape::Tag => {
            let point = (height / 2.0).min(width / 3.0);
            emit!(
                output,
                "  \\draw{} ({left:.3},{y:.3}) -- ({:.3},{top:.3}) -- ({right:.3},{top:.3}) -- ({right:.3},{bottom:.3}) -- ({:.3},{bottom:.3}) -- cycle;",
                latex_circle_options(style, false),
                left + point,
                left + point
            );
        }
    }
    emit!(
        output,
        "  \\node at ({x:.3},{y:.3}) {{{}}};",
        latex_linked_text(label, style)
    );
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
    let width = style
        .width
        .map_or(0.48, |width| width / POINTS_PER_CENTIMETER);
    let height = style
        .height
        .map_or(0.34, |height| height / POINTS_PER_CENTIMETER);
    let left = x - width / 2.0;
    let right = x + width / 2.0;
    let mut fill_options = vec![format!(
        "fill={}",
        latex_color(style.fill.as_deref().unwrap_or(background))
    )];
    if let Some(opacity) = style.opacity {
        fill_options.push(format!("opacity={opacity:.3}"));
    }
    emit!(
        output,
        "  \\fill{} ({left:.3},{:.3}) rectangle ({right:.3},{:.3});",
        latex_options(fill_options),
        y - height / 2.0,
        y + height / 2.0
    );
    let edge = if kind == WireKind::Hidden {
        left
    } else {
        right
    };
    emit!(
        output,
        "  \\draw{} ({edge:.3},{:.3}) -- ({edge:.3},{:.3});",
        latex_line_options(style),
        y - height / 2.0,
        y + height / 2.0
    );
    let mut label_style = style.clone();
    label_style.fill = None;
    label_style.shape = None;
    emit!(
        output,
        "  \\node{} at ({x:.3},{y:.3}) {{{}}};",
        latex_label_options(&label_style),
        latex_linked_text(label, style)
    );
}

fn draw_latex_cross(output: &mut String, x: f32, y: f32, style: &Style) {
    emit!(
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
    );
}

fn latex_line_options(style: &Style) -> String {
    let mut options = Vec::new();
    push_latex_line_style(&mut options, style);
    latex_options(options)
}

fn push_latex_line_style(options: &mut Vec<String>, style: &Style) {
    if let Some(stroke) = &style.stroke {
        options.push(format!("color={}", latex_color(stroke)));
    }
    push_latex_common(options, style);
}

fn push_latex_common(options: &mut Vec<String>, style: &Style) {
    if style.dashed {
        options.push("dashed".into());
    }
    if let Some(opacity) = style.opacity {
        options.push(format!("opacity={opacity:.3}"));
    }
}

fn latex_arrow_options(style: &Style) -> String {
    let mut options = vec!["->".into()];
    push_latex_line_style(&mut options, style);
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
    push_latex_common(&mut options, style);
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
    push_latex_common(&mut options, style);
    latex_options(options)
}

fn latex_label_options(style: &Style) -> String {
    latex_options(latex_label_option_values(style))
}

fn latex_endpoint_label_options(style: &Style, start: bool) -> String {
    let mut options = latex_label_option_values(style);
    options.push(format!("anchor={}", if start { "east" } else { "west" }));
    latex_options(options)
}

fn latex_label_option_values(style: &Style) -> Vec<String> {
    let mut options = vec!["inner sep=2pt".into()];
    if let Some(stroke) = &style.stroke {
        options.push(format!("text={}", latex_color(stroke)));
    }
    if let Some(fill) = &style.fill {
        options.push(format!("fill={}", latex_color(fill)));
    }
    push_latex_common(&mut options, style);
    options
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
    push_latex_common(&mut options, style);
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

fn latex_color(color: &str) -> String {
    color
        .strip_prefix('#')
        .map_or_else(|| color.into(), |hex| format!("qrab{hex}"))
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
    match &style.link {
        Some(link) => format!("\\href{{{}}}{{{text}}}", latex_url(link)),
        None => text,
    }
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
    // Callers may replace the parsed circuit's public name before rendering.
    value.replace(['\n', '\r'], " ").replace('%', "")
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
            emit!(
                output,
                "  \\draw{} ({start_x:.3},{:.3}) -- ({end_x:.3},{:.3});",
                latex_line_options(style),
                source_y + offset,
                destination_y + offset
            );
        } else {
            let bend = (corner_radius / 4.0).min(1.0);
            let first_control = start_x + (control_x - start_x) * bend;
            let second_control = end_x - (end_x - control_x) * bend;
            emit!(
                output,
                "  \\draw{} ({start_x:.3},{:.3}) .. controls ({first_control:.3},{:.3}) and ({second_control:.3},{:.3}) .. ({end_x:.3},{:.3});",
                latex_line_options(style),
                source_y + offset,
                source_y + offset,
                destination_y + offset,
                destination_y + offset
            );
        }
    }
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
            emit!(
                output,
                "  \\draw{} ({start_x:.3},{y:.3}) -- ({end_x:.3},{y:.3});",
                latex_line_options(style)
            );
        }
        WireKind::Classical => draw_classical_wire(output, start_x, end_x, y, style),
        WireKind::Hidden => {}
    }
}

fn draw_classical_wire(output: &mut String, start_x: f32, end_x: f32, y: f32, style: &Style) {
    for offset in [-0.035, 0.035] {
        let line_y = y + offset;
        emit!(
            output,
            "  \\draw{} ({start_x:.3},{line_y:.3}) -- ({end_x:.3},{line_y:.3});",
            latex_line_options(style)
        );
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
