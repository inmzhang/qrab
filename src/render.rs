use std::fmt::Write as _;

use crate::ast::{Circuit, Control, OperationKind, Orientation, Shape, Style, WireKind};

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

#[derive(Debug, Clone, Copy)]
struct Scheduled<'a> {
    kind: &'a OperationKind,
    style: &'a Style,
    column: usize,
}

fn schedule(circuit: &Circuit) -> Vec<Scheduled<'_>> {
    let mut tracks = vec![0; circuit.wires.len()];
    circuit
        .operations
        .iter()
        .map(|operation| {
            let (first, last) = operation.kind.occupied_interval(circuit.wires.len());
            let column = tracks[first..=last].iter().copied().max().unwrap_or(0);
            tracks[first..=last].fill(column + 1);
            Scheduled {
                kind: &operation.kind,
                style: &operation.style,
                column,
            }
        })
        .collect()
}

fn render_latex(circuit: &Circuit) -> String {
    let scheduled = schedule(circuit);
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
        let y = -(wire_index as f32) * circuit.layout.wire_gap;
        let measured_at = scheduled.iter().find_map(|operation| match operation.kind {
            OperationKind::Measure { targets, .. } if targets.contains(&wire_index) => Some(
                (operation.column + 1) as f32 * circuit.layout.column_gap
                    + circuit.layout.column_gap.min(0.34),
            ),
            _ => None,
        });

        match (wire.kind, measured_at) {
            (WireKind::Hidden, _) => {}
            (WireKind::Classical, _) => {
                draw_classical_wire(&mut output, 0.0, end_x, y, &wire.style);
            }
            (WireKind::Quantum, Some(change_x)) => {
                writeln!(
                    output,
                    "  \\draw{} (0,{y:.3}) -- ({change_x:.3},{y:.3});",
                    latex_line_options(&wire.style)
                )
                .expect("writing to a String cannot fail");
                draw_classical_wire(&mut output, change_x, end_x, y, &wire.style);
            }
            (WireKind::Quantum, None) => {
                writeln!(
                    output,
                    "  \\draw{} (0,{y:.3}) -- ({end_x:.3},{y:.3});",
                    latex_line_options(&wire.style)
                )
                .expect("writing to a String cannot fail");
            }
        }

        let input = wire.input.as_deref().unwrap_or(&wire.name);
        writeln!(
            output,
            "  \\node[anchor=east] at (0,{y:.3}) {{{}}};",
            latex_text(input)
        )
        .expect("writing to a String cannot fail");
        if let Some(label) = &wire.output {
            writeln!(
                output,
                "  \\node[anchor=west] at ({end_x:.3},{y:.3}) {{{}}};",
                latex_text(label)
            )
            .expect("writing to a String cannot fail");
        }
    }

    for operation in &scheduled {
        let x = (operation.column + 1) as f32 * circuit.layout.column_gap;
        match operation.kind {
            OperationKind::Gate {
                label,
                targets,
                controls,
            } => draw_latex_gate(
                &mut output,
                x,
                circuit.layout.wire_gap,
                label,
                targets,
                controls,
                operation.style,
            ),
            OperationKind::Measure { targets, label } => {
                for target in targets {
                    draw_latex_measurement(
                        &mut output,
                        x,
                        circuit.layout.wire_gap,
                        *target,
                        label.as_deref(),
                        operation.style,
                    );
                }
            }
            OperationKind::Swap { left, right } => {
                let left_y = -(*left as f32) * circuit.layout.wire_gap;
                let right_y = -(*right as f32) * circuit.layout.wire_gap;
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
                let (first, last) = operation.kind.occupied_interval(circuit.wires.len());
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
        }
    }

    output.push_str("\\end{tikzpicture}\n");
    output.push_str("\\end{document}\n");
    output
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
    let scheduled = schedule(circuit);
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
        let count = match wire.kind {
            WireKind::Hidden => 0,
            WireKind::Quantum => 1,
            WireKind::Classical => 2,
        };
        let stroke = typst_stroke(&wire.style);
        if count != 1 || stroke.is_some() {
            let stroke = stroke.map_or_else(String::new, |value| format!(", stroke: {value}"));
            writeln!(
                output,
                "  quill.setwire({count}{stroke}, x: 0, y: {wire_index}),"
            )
            .expect("writing to a String cannot fail");
        }
        let input = wire.input.as_deref().unwrap_or(&wire.name);
        writeln!(
            output,
            "  quill.lstick(text(\"{}\"), x: 0, y: {wire_index}),",
            typst_string(input)
        )
        .expect("writing to a String cannot fail");
        if let Some(label) = &wire.output {
            writeln!(
                output,
                "  quill.rstick(text(\"{}\"), x: {end_column}, y: {wire_index}),",
                typst_string(label)
            )
            .expect("writing to a String cannot fail");
        }
    }

    for operation in &scheduled {
        let x = operation.column + 1;
        match operation.kind {
            OperationKind::Gate {
                label,
                targets,
                controls,
            } => draw_typst_gate(&mut output, x, label, targets, controls, operation.style),
            OperationKind::Measure { targets, label } => {
                for target in targets {
                    let label_argument = label.as_ref().map_or_else(String::new, |value| {
                        format!(", label: text(\"{}\")", typst_string(value))
                    });
                    writeln!(
                        output,
                        "  quill.meter(x: {x}, y: {target}{label_argument}{}),",
                        typst_measure_style(operation.style)
                    )
                    .expect("writing to a String cannot fail");
                    writeln!(output, "  quill.setwire(2, x: {}, y: {target}),", x + 1)
                        .expect("writing to a String cannot fail");
                }
            }
            OperationKind::Swap { left, right } => {
                let distance = *right as isize - *left as isize;
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
                let (first, last) = operation.kind.occupied_interval(circuit.wires.len());
                writeln!(
                    output,
                    "  quill.slice(n: {}, x: {x}, y: {first}, stroke: {}),",
                    last - first + 1,
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
}
