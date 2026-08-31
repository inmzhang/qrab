use std::fmt::Write as _;

use crate::ast::{Circuit, Control, OperationKind, WireKind};

const COLUMN_WIDTH: f32 = 1.5;

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
    let end_x = (last_column + 2) as f32 * COLUMN_WIDTH;
    let mut output = String::new();
    output.push_str("\\documentclass[tikz,border=6pt]{standalone}\n");
    output.push_str("\\usepackage{tikz}\n");
    output.push_str("\\begin{document}\n");
    output.push_str("\\begin{tikzpicture}[line cap=round,line join=round,font=\\sffamily]\n");
    writeln!(output, "% circuit: {}", latex_comment(&circuit.name))
        .expect("writing to a String cannot fail");

    for (wire_index, wire) in circuit.wires.iter().enumerate() {
        let y = -(wire_index as f32);
        let measured_at = scheduled.iter().find_map(|operation| match operation.kind {
            OperationKind::Measure { targets, .. } if targets.contains(&wire_index) => {
                Some((operation.column + 1) as f32 * COLUMN_WIDTH + 0.34)
            }
            _ => None,
        });

        match (wire.kind, measured_at) {
            (WireKind::Hidden, _) => {}
            (WireKind::Classical, _) => draw_classical_wire(&mut output, 0.0, end_x, y),
            (WireKind::Quantum, Some(change_x)) => {
                writeln!(output, "  \\draw (0,{y:.3}) -- ({change_x:.3},{y:.3});")
                    .expect("writing to a String cannot fail");
                draw_classical_wire(&mut output, change_x, end_x, y);
            }
            (WireKind::Quantum, None) => {
                writeln!(output, "  \\draw (0,{y:.3}) -- ({end_x:.3},{y:.3});")
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
        let x = (operation.column + 1) as f32 * COLUMN_WIDTH;
        match operation.kind {
            OperationKind::Gate {
                label,
                targets,
                controls,
            } => draw_latex_gate(&mut output, x, label, targets, controls),
            OperationKind::Measure { targets, label } => {
                for target in targets {
                    draw_latex_measurement(&mut output, x, *target, label.as_deref());
                }
            }
            OperationKind::Swap { left, right } => {
                let left_y = -(*left as f32);
                let right_y = -(*right as f32);
                writeln!(
                    output,
                    "  \\draw ({x:.3},{left_y:.3}) -- ({x:.3},{right_y:.3});"
                )
                .expect("writing to a String cannot fail");
                draw_latex_cross(&mut output, x, left_y);
                draw_latex_cross(&mut output, x, right_y);
            }
            OperationKind::Barrier { wires } => {
                let (first, last) = operation.kind.occupied_interval(circuit.wires.len());
                let top = -(first as f32) + 0.42;
                let bottom = -(last as f32) - 0.42;
                writeln!(
                    output,
                    "  \\draw[dashed] ({x:.3},{top:.3}) -- ({x:.3},{bottom:.3}); % barrier on {} wire(s)",
                    if wires.is_empty() { circuit.wires.len() } else { wires.len() }
                )
                .expect("writing to a String cannot fail");
            }
        }
    }

    output.push_str("\\end{tikzpicture}\n");
    output.push_str("\\end{document}\n");
    output
}

fn draw_classical_wire(output: &mut String, start_x: f32, end_x: f32, y: f32) {
    for offset in [-0.035, 0.035] {
        let line_y = y + offset;
        writeln!(
            output,
            "  \\draw ({start_x:.3},{line_y:.3}) -- ({end_x:.3},{line_y:.3});"
        )
        .expect("writing to a String cannot fail");
    }
}

fn draw_latex_gate(
    output: &mut String,
    x: f32,
    label: &str,
    targets: &[usize],
    controls: &[Control],
) {
    if !controls.is_empty() {
        let (first, last) = occupied_bounds(targets, controls);
        writeln!(
            output,
            "  \\draw ({x:.3},{:.3}) -- ({x:.3},{:.3});",
            -(first as f32),
            -(last as f32)
        )
        .expect("writing to a String cannot fail");
        for control in controls {
            let y = -(control.wire as f32);
            if control.positive {
                writeln!(output, "  \\fill ({x:.3},{y:.3}) circle[radius=2.2pt];")
                    .expect("writing to a String cannot fail");
            } else {
                writeln!(
                    output,
                    "  \\draw[fill=white] ({x:.3},{y:.3}) circle[radius=2.2pt];"
                )
                .expect("writing to a String cannot fail");
            }
        }
    }

    if targets.len() > 1 {
        let first = *targets.iter().min().expect("gate has a target");
        let last = *targets.iter().max().expect("gate has a target");
        let midpoint = -((first + last) as f32) / 2.0;
        let height = (last - first) as f32 + 0.72;
        writeln!(
            output,
            "  \\node[draw,fill=white,minimum width=10mm,minimum height={height:.3}cm] at ({x:.3},{midpoint:.3}) {{{}}};",
            latex_text(label)
        )
        .expect("writing to a String cannot fail");
        return;
    }

    let y = -(targets[0] as f32);
    if !controls.is_empty() && label == "X" {
        writeln!(
            output,
            "  \\draw[fill=white] ({x:.3},{y:.3}) circle[radius=4.0pt];"
        )
        .expect("writing to a String cannot fail");
        writeln!(
            output,
            "  \\draw ({:.3},{y:.3}) -- ({:.3},{y:.3}) ({x:.3},{:.3}) -- ({x:.3},{:.3});",
            x - 0.14,
            x + 0.14,
            y - 0.14,
            y + 0.14
        )
        .expect("writing to a String cannot fail");
    } else if !controls.is_empty() && label == "Z" {
        writeln!(output, "  \\fill ({x:.3},{y:.3}) circle[radius=2.2pt];")
            .expect("writing to a String cannot fail");
    } else {
        writeln!(
            output,
            "  \\node[draw,fill=white,minimum width=8mm,minimum height=7mm] at ({x:.3},{y:.3}) {{{}}};",
            latex_text(label)
        )
        .expect("writing to a String cannot fail");
    }
}

fn draw_latex_measurement(output: &mut String, x: f32, target: usize, label: Option<&str>) {
    let y = -(target as f32);
    writeln!(
        output,
        "  \\draw[fill=white] ({:.3},{:.3}) rectangle ({:.3},{:.3});",
        x - 0.34,
        y - 0.28,
        x + 0.34,
        y + 0.28
    )
    .expect("writing to a String cannot fail");
    writeln!(
        output,
        "  \\draw ({:.3},{:.3}) arc[start angle=180,end angle=0,radius=0.22];",
        x - 0.22,
        y + 0.10
    )
    .expect("writing to a String cannot fail");
    writeln!(
        output,
        "  \\draw[->] ({x:.3},{:.3}) -- ({:.3},{:.3});",
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

fn draw_latex_cross(output: &mut String, x: f32, y: f32) {
    writeln!(
        output,
        "  \\draw ({:.3},{:.3}) -- ({:.3},{:.3}) ({:.3},{:.3}) -- ({:.3},{:.3});",
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
    let mut output = String::from(
        "#set page(width: auto, height: auto, margin: 6pt)\n#import \"@preview/quill:0.8.0\" as quill\n\n#quill.quantum-circuit(\n",
    );
    writeln!(output, "  wires: {},", circuit.wires.len()).expect("writing to a String cannot fail");

    for (wire_index, wire) in circuit.wires.iter().enumerate() {
        let count = match wire.kind {
            WireKind::Hidden => 0,
            WireKind::Quantum => 1,
            WireKind::Classical => 2,
        };
        if count != 1 {
            writeln!(output, "  quill.setwire({count}, x: 0, y: {wire_index}),")
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
            } => draw_typst_gate(&mut output, x, label, targets, controls),
            OperationKind::Measure { targets, label } => {
                for target in targets {
                    let label_argument = label.as_ref().map_or_else(String::new, |value| {
                        format!(", label: text(\"{}\")", typst_string(value))
                    });
                    writeln!(
                        output,
                        "  quill.meter(x: {x}, y: {target}{label_argument}),"
                    )
                    .expect("writing to a String cannot fail");
                    writeln!(output, "  quill.setwire(2, x: {}, y: {target}),", x + 1)
                        .expect("writing to a String cannot fail");
                }
            }
            OperationKind::Swap { left, right } => {
                let distance = *right as isize - *left as isize;
                writeln!(output, "  quill.swap({distance}, x: {x}, y: {left}),")
                    .expect("writing to a String cannot fail");
                writeln!(output, "  quill.swap(x: {x}, y: {right}),")
                    .expect("writing to a String cannot fail");
            }
            OperationKind::Barrier { .. } => {
                let (first, last) = operation.kind.occupied_interval(circuit.wires.len());
                writeln!(
                    output,
                    "  quill.slice(n: {}, x: {x}, y: {first}, stroke: (paint: black, thickness: 0.7pt, dash: \"dashed\")),",
                    last - first + 1
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
    output
}

fn draw_typst_gate(
    output: &mut String,
    x: usize,
    label: &str,
    targets: &[usize],
    controls: &[Control],
) {
    if !controls.is_empty() {
        let (first, last) = occupied_bounds(targets, controls);
        let anchor = controls
            .iter()
            .find(|control| control.wire == first)
            .or_else(|| controls.iter().find(|control| control.wire == last))
            .expect("controlled gate has a control");
        let destination = if anchor.wire == first { last } else { first };
        for control in controls {
            let distance = if control.wire == anchor.wire {
                destination as isize - control.wire as isize
            } else {
                0
            };
            writeln!(
                output,
                "  quill.ctrl({distance}, open: {}, x: {x}, y: {}),",
                !control.positive, control.wire
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
            "  quill.mqgate(text(\"{}\"), n: {}, x: {x}, y: {first}{pass_through}),",
            typst_string(label),
            last - first + 1
        )
        .expect("writing to a String cannot fail");
        return;
    }

    let target = targets[0];
    if !controls.is_empty() && label == "X" {
        writeln!(output, "  quill.targ(x: {x}, y: {target}),")
            .expect("writing to a String cannot fail");
    } else if !controls.is_empty() && label == "Z" {
        writeln!(output, "  quill.ctrl(x: {x}, y: {target}),")
            .expect("writing to a String cannot fail");
    } else {
        writeln!(
            output,
            "  quill.gate(text(\"{}\"), x: {x}, y: {target}),",
            typst_string(label)
        )
        .expect("writing to a String cannot fail");
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
