use std::fmt::Write as _;

use super::*;

pub(super) fn render_typst(circuit: &Circuit) -> String {
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
        output.push_str(
            "#rotate(-90deg, reflow: true)[\n#show text: it => rotate(90deg, reflow: true, it)\n",
        );
    }
    output.push_str("#quill.quantum-circuit(\n");
    emit!(output, "  wires: {},", circuit.wires.len());
    emit!(
        output,
        "  row-spacing: {:.3}pt,",
        circuit.layout.wire_gap * TYPST_ROW_POINTS_PER_UNIT
    );
    emit!(
        output,
        "  column-spacing: {:.3}pt,",
        circuit.layout.column_gap * TYPST_COLUMN_POINTS_PER_UNIT
    );
    emit!(output, "  scale: {:.3}%,", circuit.layout.scale * 100.0);
    emit!(
        output,
        "  gate-padding: {:.3}pt,",
        ((circuit.layout.gate_size - 10.0) / 2.0).max(0.0)
    );
    emit!(
        output,
        "  fill: {},",
        typst_color(&circuit.layout.background, None)
    );

    write_typst_wire_streams(&mut output, circuit, &scheduled, end_column);

    for (group_index, group) in circuit.groups.iter().enumerate() {
        let (first_column, last_column, first_row, last_row) =
            group_bounds(group, &scheduled, circuit.wires.len());
        emit!(
            output,
            "  quill.gategroup({}, {}, x: {first_column}, y: {first_row}, label: (content: text(\"{}\"), pos: top, dy: -{}pt){}),",
            last_row - first_row + 1,
            last_column - first_column + 1,
            typst_string(&group.label),
            (group_index + 1) * 12,
            typst_group_style(&group.style)
        );
    }

    // Quill lays out its own grid, so this backend only reads the wire kinds
    // out of the transitions; the coordinates it passes in go unused.
    let columns = column_positions(circuit, &scheduled);
    for (wire_index, wire) in circuit.wires.iter().enumerate() {
        let (initial_kind, transitions) =
            wire_transitions(circuit, &scheduled, wire_index, &columns);
        if initial_kind != WireKind::Hidden || wire.ellipsis {
            let input = wire.input.as_deref().unwrap_or(&wire.name);
            emit!(
                output,
                "  quill.lstick(text(\"{}\"), x: 0, y: {wire_index}),",
                typst_string(input)
            );
        }
        let final_kind = transitions
            .last()
            .map_or(initial_kind, |transition| transition.1);
        if (final_kind != WireKind::Hidden || wire.ellipsis)
            && let Some(label) = &wire.output
        {
            emit!(
                output,
                "  quill.rstick(text(\"{}\"), x: {end_column}, y: {}),",
                typst_string(label),
                final_positions[wire_index]
            );
        }
    }

    for (operation_index, operation) in scheduled.iter().enumerate() {
        let mut style = operation.style.clone();
        if circuit.layout.orientation == Orientation::Vertical {
            std::mem::swap(&mut style.width, &mut style.height);
        }
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
                draw_typst_gate(&mut output, x, label, &targets, &controls, &style);
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
                        &style,
                        &circuit.layout,
                    );
                }
            }
            OperationKind::Swap { left, right } => {
                let left = operation.positions[*left];
                let right = operation.positions[*right];
                let distance = right as isize - left as isize;
                emit!(
                    output,
                    "  quill.swap({distance}, x: {x}, y: {left}{}),",
                    typst_swap_style(&style)
                );
                emit!(
                    output,
                    "  quill.swap(x: {x}, y: {right}{}),",
                    typst_swap_style(&style)
                );
            }
            OperationKind::Barrier { .. } => {
                let (first, last) = (operation.first, operation.last);
                emit!(
                    output,
                    "  quill.slice(n: {}, x: {x}, y: {first}, stroke: {}),",
                    last - first + 1,
                    typst_barrier_stroke(&style)
                );
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
                            &style,
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
                    let row = operation.positions[wire];
                    let label = label.as_ref().map_or_else(String::new, |label| {
                        format!(
                            ", label: (content: {}, pos: {})",
                            typst_label_body(label, &style),
                            if *start { "left" } else { "right" }
                        )
                    });
                    emit!(
                        output,
                        "  quill.midstick(line(start: (0pt, -3.7pt), end: (0pt, 3.7pt), stroke: {}), x: {x}, y: {row}{}{label}),",
                        typst_stroke(&style).unwrap_or_else(|| "black".into()),
                        typst_label_style(&style)
                    );
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
                        &style,
                        &circuit.layout.background,
                    );
                } else {
                    emit!(
                        output,
                        "  quill.midstick({}, n: {}, x: {x}, y: {first}{}),",
                        typst_label_body(label, &style),
                        last - first + 1,
                        typst_label_style(&style)
                    );
                }
            }
            OperationKind::Bundle { wire, label } => {
                emit!(
                    output,
                    "  quill.nwire(text(\"{}\"), x: {x}, y: {}),",
                    typst_string(label),
                    operation.positions[*wire]
                );
            }
            OperationKind::Permute { .. } => {
                let (first, last) = (operation.first, operation.last);
                let mut row_wires = vec![0; circuit.wires.len()];
                for (wire, row) in operation.positions.iter().enumerate() {
                    row_wires[*row] = wire;
                }
                let span_wires = &row_wires[first..=last];
                let mapping = operation
                    .permutation
                    .as_deref()
                    .expect("permutation operation has a row mapping")[first..=last]
                    .iter()
                    .map(|destination| destination - first)
                    .collect::<Vec<_>>();
                emit!(
                    output,
                    "  quill.permute({}, x: {x}, y: {first}{}),",
                    mapping
                        .iter()
                        .map(usize::to_string)
                        .collect::<Vec<_>>()
                        .join(", "),
                    typst_permute_style(
                        &style,
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
                );
            }
            OperationKind::Phantom { wires } => {
                for wire in expanded_wires(wires, circuit.wires.len()) {
                    let row = operation.positions[wire];
                    emit!(
                        output,
                        "  quill.phantom(x: {x}, y: {row}, width: {:.3}pt, height: {:.3}pt),",
                        style.width.unwrap_or(0.0),
                        style.height.unwrap_or(0.0)
                    );
                }
            }
            OperationKind::Touch { .. } => {
                if has_line_style(&style) {
                    let (first, last) = (operation.first, operation.last);
                    emit!(
                        output,
                        "  quill.slice(n: {}, x: {x}, y: {first}, stroke: {}),",
                        last - first + 1,
                        typst_stroke(&style).unwrap_or_else(|| "black".into())
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
                    emit!(
                        output,
                        "  quill.midstick({}, x: {x}, y: {}{}),",
                        typst_label_body(label, &style),
                        operation.positions[*wire],
                        typst_label_style(&style)
                    );
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
                    &style,
                    &circuit.layout.background,
                );
            }
            OperationKind::Note { wires, text, side } => {
                let rows = selected_wires(wires, circuit.wires.len())
                    .iter()
                    .map(|wire| operation.positions[*wire])
                    .collect::<Vec<_>>();
                let row = if *side == NoteSide::Above {
                    rows.iter().min()
                } else {
                    rows.iter().max()
                }
                .expect("circuit has a wire");
                emit!(
                    output,
                    "  quill.gategroup(1, 1, x: {x}, y: {row}, padding: 0pt, stroke: none, label: (content: block(width: {:.3}pt, align(center, text(\"{}\"))), pos: {})),",
                    circuit.layout.comment_width,
                    typst_string(text),
                    if *side == NoteSide::Above {
                        "top"
                    } else {
                        "bottom"
                    },
                );
            }
            OperationKind::Cut { label, .. } => {
                let label = label.as_ref().map_or_else(String::new, |label| {
                    format!(", label: text(\"{}\")", typst_string(label))
                });
                emit!(
                    output,
                    "  quill.slice(n: {}, x: {x}, y: {}, stroke: {}{label}),",
                    operation.last - operation.first + 1,
                    operation.first,
                    typst_barrier_stroke(&style)
                );
            }
        }
    }

    if circuit.wires.iter().all(|wire| wire.output.is_none()) {
        emit!(
            output,
            "  quill.phantom(x: {end_column}, y: {}, width: 0pt, height: 0pt),",
            circuit.wires.len() - 1
        );
    }
    output.push_str(")\n");
    if circuit.layout.orientation == Orientation::Vertical {
        output.push_str("]\n");
    }
    append_raw(&mut output, &circuit.escapes.typst.after);
    output
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
                    operation.permuted_row(*wire)
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
        emit!(output, "  quill.setwire({count}, stroke: {stroke}),");
        let mut cursor = 0;
        let mut drew_segment = false;
        for (column, count, stroke) in &row_events[1..] {
            let length = if drew_segment {
                column - cursor
            } else {
                column + 1
            };
            if length > 0 {
                emit!(output, "  {length},");
                drew_segment = true;
                cursor = *column;
            }
            emit!(output, "  quill.setwire({count}, stroke: {stroke}),");
        }
        let remaining = if drew_segment {
            end_column - cursor
        } else {
            end_column + 1
        };
        if remaining > 0 {
            emit!(output, "  {remaining},");
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
            emit!(
                output,
                "  quill.ctrl({distance}, open: {}, x: {x}, y: {}{}),",
                !control.positive,
                control.wire,
                typst_control_style(style)
            );
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
        emit!(
            output,
            "  quill.mqgate({}, n: {}, x: {x}, y: {first}{pass_through}{}),",
            typst_gate_body(label, style),
            last - first + 1,
            typst_gate_style(style)
        );
        return;
    }

    let target = targets[0];
    if !controls.is_empty() && label == "X" && style.link.is_none() {
        emit!(
            output,
            "  quill.targ(x: {x}, y: {target}{}),",
            typst_control_style(style)
        );
    } else if !controls.is_empty() && label == "Z" && style.link.is_none() {
        emit!(
            output,
            "  quill.ctrl(x: {x}, y: {target}{}),",
            typst_control_style(style)
        );
    } else {
        emit!(
            output,
            "  quill.gate({}, x: {x}, y: {target}{}),",
            typst_gate_body(label, style),
            typst_gate_style(style)
        );
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
        emit!(
            output,
            "  quill.meter(x: {x}, y: {row}{}),",
            typst_measure_style(style)
        );
        return;
    };

    match shape {
        MeasurementShape::D => {
            let mut gate_style = style.clone();
            gate_style.width.get_or_insert(layout.gate_size);
            emit!(
                output,
                "  quill.gate({}, x: {x}, y: {row}{}, radius: (top-right: {FULLY_ROUNDED_RADIUS}, bottom-right: {FULLY_ROUNDED_RADIUS})),",
                typst_gate_body(label, &gate_style),
                typst_gate_style(&gate_style)
            );
        }
        MeasurementShape::Tag => {
            let width = style.width.unwrap_or(layout.gate_size);
            emit!(
                output,
                "  quill.gate({}, x: {x}, y: {row}, box: false, width: {width:.3}pt),",
                typst_measure_tag_body(label, style, &layout.background, layout.gate_size)
            );
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
    emit!(
        output,
        "  quill.gate({}, x: {x}, y: {row}, box: false, width: {width:.3}pt),",
        typst_value_transition_body(label, kind, style, background)
    );
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
    match style.height {
        Some(height) => format!("box(height: {height:.3}pt, {text})"),
        None => text,
    }
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
        arguments.push(format!("radius: {FULLY_ROUNDED_RADIUS}"));
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
        arguments.push(format!("radius: {FULLY_ROUNDED_RADIUS}"));
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

fn typst_label_body(label: &str, style: &Style) -> String {
    let text = if style.stroke.is_some() || style.opacity.is_some() {
        format!(
            "text(fill: {}, \"{}\")",
            typst_color(style.stroke.as_deref().unwrap_or("black"), style.opacity),
            typst_string(label)
        )
    } else {
        format!("text(\"{}\")", typst_string(label))
    };
    match &style.link {
        Some(link) => format!("link(\"{}\", {text})", typst_string(link)),
        None => text,
    }
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
    emit!(
        output,
        "  quill.mqgate({}, n: {n}, x: {x}, y: {first}, fill: {}, stroke: none),",
        typst_brace_body(label, side, n, style),
        typst_color(style.fill.as_deref().unwrap_or(background), style.opacity)
    );
}

fn typst_brace_body(label: &str, side: BraceSide, wires: usize, style: &Style) -> String {
    let size = (wires as f32 * 12.0).max(18.0);
    let color = typst_color(style.stroke.as_deref().unwrap_or("black"), style.opacity);
    let left = format!("#text(size: {size:.3}pt, fill: {color}, \"{{\")");
    let right = format!("#text(size: {size:.3}pt, fill: {color}, \"}}\")");
    let label = format!("#{}", typst_label_body(label, style));
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
    match opacity {
        Some(opacity) => format!("{base}.transparentize({:.3}%)", (1.0 - opacity) * 100.0),
        None => base,
    }
}

fn typst_arguments(arguments: Vec<String>) -> String {
    if arguments.is_empty() {
        String::new()
    } else {
        format!(", {}", arguments.join(", "))
    }
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
    match &style.link {
        Some(link) => format!("link(\"{}\", {text})", typst_string(link)),
        None => text,
    }
}
const TYPST_ROW_POINTS_PER_UNIT: f32 = 12.0;
const TYPST_COLUMN_POINTS_PER_UNIT: f32 = 8.0;
const FULLY_ROUNDED_RADIUS: &str = "999pt";

fn wire_count(kind: WireKind) -> usize {
    match kind {
        WireKind::Quantum => 1,
        WireKind::Classical => 2,
        WireKind::Hidden => 0,
    }
}
