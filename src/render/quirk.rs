use std::fmt::Write;

use crate::ast::{Circuit, Control, OperationKind, ParityBasis, Shape, Style};

use super::{LabelSpan, label_spans, label_text, schedule};

const QUIRK_URL: &str = "https://algassert.com/quirk#circuit=";
const MAX_WIRES: usize = 16;

#[derive(Debug, PartialEq, Eq)]
struct CustomGate {
    id: String,
    name: String,
    height: usize,
}

pub(super) fn render_quirk(circuit: &Circuit) -> String {
    let wire_count = circuit.wires.len().min(MAX_WIRES);
    let mut columns = Vec::new();
    let mut custom_gates = Vec::new();

    let (mut scheduled, _) = schedule(circuit);
    scheduled.sort_by_key(|operation| operation.column);
    let mut layer = None;
    let mut packed = None;
    for operation in scheduled {
        if layer != Some(operation.column) {
            layer = Some(operation.column);
            packed = None;
        }
        let mut column = vec![None; wire_count];
        match operation.kind {
            OperationKind::Gate {
                label,
                targets,
                controls,
            } if targets.len() == 1
                && targets[0] < wire_count
                && controls.iter().all(|control| control.wire < wire_count) =>
            {
                let label = quirk_label(label);
                let gate = native_gate(&label, operation.style)
                    .map_or_else(|| custom_gate(&mut custom_gates, &label, 1), str::to_owned);
                column[targets[0]] = Some(gate);
                for control in controls {
                    column[control.wire] = Some(control_gate(control).into());
                }
            }
            OperationKind::Gate {
                label,
                targets,
                controls,
            } if !targets.is_empty()
                && targets.iter().all(|wire| *wire < wire_count)
                && controls.iter().all(|control| control.wire < wire_count) =>
            {
                let first = *targets.iter().min().expect("gate has targets");
                let last = *targets.iter().max().expect("gate has targets");
                // Quirk rejects controls inside a multi-wire gate.
                if controls
                    .iter()
                    .all(|control| control.wire < first || control.wire > last)
                {
                    column[first] = Some(custom_gate(
                        &mut custom_gates,
                        &quirk_label(label),
                        last - first + 1,
                    ));
                    for control in controls {
                        column[control.wire] = Some(control_gate(control).into());
                    }
                }
            }
            OperationKind::Measure { targets, .. } => {
                for target in targets
                    .iter()
                    .copied()
                    .filter(|target| *target < wire_count)
                {
                    column[target] = Some("Measure".into());
                }
            }
            OperationKind::Swap { left, right } if *left < wire_count && *right < wire_count => {
                column[*left] = Some("Swap".into());
                column[*right] = Some("Swap".into());
            }
            _ => {}
        }
        if !column.iter().any(Option::is_some) {
            continue;
        }

        // Quirk's controls and swaps apply to the whole column. Keep those and
        // spanning gates isolated, but pack independent unary gates.
        let isolated = match operation.kind {
            OperationKind::Gate {
                targets, controls, ..
            } => targets.len() != 1 || !controls.is_empty(),
            OperationKind::Swap { .. } => true,
            _ => false,
        };
        if isolated {
            columns.push(column);
            continue;
        }

        let index = if let Some(index) = packed {
            index
        } else {
            columns.push(vec![None; wire_count]);
            let index = columns.len() - 1;
            packed = Some(index);
            index
        };
        for (target, gate) in columns[index].iter_mut().zip(column) {
            if gate.is_some() {
                debug_assert!(target.is_none());
                *target = gate;
            }
        }
    }

    let mut json = String::from("{\"cols\":");
    push_columns(&mut json, &columns);
    if !custom_gates.is_empty() {
        json.push_str(",\"gates\":[");
        for (index, gate) in custom_gates.iter().enumerate() {
            if index > 0 {
                json.push(',');
            }
            push_custom_gate(&mut json, gate);
        }
        json.push(']');
    }
    push_initial_states(&mut json, circuit);
    json.push('}');

    let fragment = if json.contains('%') || json.contains('&') {
        encode_uri_component(&json)
    } else {
        json
    };
    format!("{QUIRK_URL}{fragment}")
}

fn control_gate(control: &Control) -> &'static str {
    match control.parity {
        Some(ParityBasis::X) => "xpar",
        Some(ParityBasis::Y) => "ypar",
        Some(ParityBasis::Z) => "zpar",
        None if control.positive => "•",
        None => "◦",
    }
}

fn quirk_label(label: &str) -> String {
    let mut output = String::new();
    for span in label_spans(label) {
        match span {
            LabelSpan::Text(text) => output.push_str(&label_text(text)),
            // Quirk cannot typeset TeX, but the source without `$` delimiters
            // remains a useful and deterministic custom-gate name.
            LabelSpan::Math(math) => output.push_str(math),
        }
    }
    output
}

fn native_gate<'a>(label: &'a str, style: &Style) -> Option<&'a str> {
    match label {
        "H" | "X" | "Y" | "Z" => Some(label),
        "S" => Some("Z^½"),
        "T" => Some("Z^¼"),
        "S^-1" | "S†" => Some("Z^-½"),
        "T^-1" | "T†" => Some("Z^-¼"),
        _ if style.shape == Some(Shape::Circle) => match label.trim() {
            "1" => Some("Z"),
            "2" => Some("Z^½"),
            "3" => Some("Z^¼"),
            "4" => Some("Z^⅛"),
            "5" => Some("Z^⅟₁₆"),
            "6" => Some("Z^⅟₃₂"),
            "7" => Some("Z^⅟₆₄"),
            "8" => Some("Z^⅟₁₂₈"),
            _ => None,
        },
        _ => None,
    }
}

fn custom_gate(gates: &mut Vec<CustomGate>, name: &str, height: usize) -> String {
    // ponytail: custom gate sets are diagram-sized; index by (name, height) if
    // generated circuits ever make this linear lookup measurable.
    if let Some(gate) = gates
        .iter()
        .find(|gate| gate.name == name && gate.height == height)
    {
        return gate.id.clone();
    }
    let id = format!("~{}", gates.len());
    gates.push(CustomGate {
        id: id.clone(),
        name: name.into(),
        height,
    });
    id
}

fn push_columns(output: &mut String, columns: &[Vec<Option<String>>]) {
    output.push('[');
    for (column_index, column) in columns.iter().enumerate() {
        if column_index > 0 {
            output.push(',');
        }
        output.push('[');
        let length = column
            .iter()
            .rposition(Option::is_some)
            .map_or(0, |index| index + 1);
        for (row, gate) in column[..length].iter().enumerate() {
            if row > 0 {
                output.push(',');
            }
            if let Some(gate) = gate {
                push_json_string(output, gate);
            } else {
                output.push('1');
            }
        }
        output.push(']');
    }
    output.push(']');
}

fn push_custom_gate(output: &mut String, gate: &CustomGate) {
    output.push_str("{\"id\":");
    push_json_string(output, &gate.id);
    output.push_str(",\"name\":");
    push_json_string(output, &gate.name);
    output.push_str(",\"circuit\":{\"cols\":[");
    if gate.height > 1 {
        output.push('[');
        for _ in 1..gate.height {
            output.push_str("1,");
        }
        push_json_string(output, "…");
        output.push(']');
    }
    output.push_str("]}}");
}

fn push_initial_states(output: &mut String, circuit: &Circuit) {
    let states = circuit
        .wires
        .iter()
        .take(MAX_WIRES)
        .map(|wire| initial_state(wire.input.as_deref()))
        .collect::<Vec<_>>();
    let Some(last) = states.iter().rposition(|state| *state != "0") else {
        return;
    };
    output.push_str(",\"init\":[");
    for (index, state) in states[..=last].iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        if matches!(*state, "0" | "1") {
            output.push_str(state);
        } else {
            push_json_string(output, state);
        }
    }
    output.push(']');
}

fn initial_state(label: Option<&str>) -> &'static str {
    let label = label.map(quirk_label);
    let compact = label.as_deref().map(|label| {
        label
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>()
    });
    match compact.as_deref() {
        Some("1" | "|1>" | "|1⟩") => "1",
        Some("\\ket{1}" | "\\lvert1\\rangle") => "1",
        Some("+" | "|+>" | "|+⟩") => "+",
        Some("\\ket{+}" | "\\lvert+\\rangle") => "+",
        Some("-" | "|->" | "|-⟩") => "-",
        Some("\\ket{-}" | "\\lvert-\\rangle") => "-",
        Some("i" | "+i" | "|i>" | "|+i>" | "|i⟩" | "|+i⟩") => "i",
        Some("-i" | "|-i>" | "|-i⟩") => "-i",
        _ => "0",
    }
}

fn push_json_string(output: &mut String, value: &str) {
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\u{08}' => output.push_str("\\b"),
            '\u{0c}' => output.push_str("\\f"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character <= '\u{1f}' => {
                write!(output, "\\u{:04x}", character as u32)
                    .expect("writing to a String cannot fail");
            }
            character => output.push(character),
        }
    }
    output.push('"');
}

fn encode_uri_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')'
            )
        {
            encoded.push(char::from(byte));
        } else {
            write!(encoded, "%{byte:02X}").expect("writing to a String cannot fail");
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use crate::parse;

    use super::*;

    fn render(source: &str) -> String {
        render_quirk(&parse(source).expect("valid Quirk fixture"))
    }

    #[test]
    fn bell_circuit_uses_native_quirk_gates() {
        assert_eq!(
            render(
                r#"
                    circuit bell {
                      qubit q[2]
                      h q[0]
                      x q[1] if q[0]
                      measure q[0], q[1]
                    }
                "#,
            ),
            concat!(
                "https://algassert.com/quirk#circuit=",
                r#"{"cols":[["H"],["•","X"],["Measure","Measure"]]}"#,
            )
        );
    }

    #[test]
    fn roots_negative_controls_swaps_and_initial_states_are_native() {
        assert_eq!(
            render(
                r#"
                    circuit native {
                      qubit a: "|1>"
                      qubit b: "|+>"
                      qubit c: "|-i>"
                      s a
                      t b if !a
                      phase "4" on c
                      swap a, c
                    }
                "#,
            ),
            concat!(
                "https://algassert.com/quirk#circuit=",
                r#"{"cols":[["Z^½",1,"Z^⅛"],["◦","Z^¼"],["Swap",1,"Swap"]],"init":[1,"+","-i"]}"#,
            )
        );
    }

    #[test]
    fn ordinary_and_parity_controls_use_distinct_quirk_gates() {
        assert_eq!(
            render(
                r#"
                    circuit parity_controls {
                      qubit top
                      qubit middle
                      qubit bottom
                      s top if middle, bottom
                      s top if parity(middle, bottom)
                      s top if parity_x(middle, bottom)
                      s top if parity_y(middle, bottom)
                    }
                "#,
            ),
            concat!(
                "https://algassert.com/quirk#circuit=",
                r#"{"cols":[["Z^½","•","•"],["Z^½","zpar","zpar"],["Z^½","xpar","xpar"],["Z^½","ypar","ypar"]]}"#,
            )
        );
    }

    #[test]
    fn parallel_gates_share_columns_without_combining_controlled_gates() {
        assert_eq!(
            render(
                r#"
                    circuit layers {
                      qubit q[5]
                      parallel {
                        h q[0]
                        h q[1]
                        h q[2]
                        h q[3]
                      }
                      parallel {
                        x q[1] if q[0]
                        x q[4] if q[3]
                      }
                      parallel {
                        x q[0]
                        x q[1]
                        x q[2]
                        x q[3]
                        x q[4]
                      }
                    }
                "#,
            ),
            concat!(
                "https://algassert.com/quirk#circuit=",
                r#"{"cols":[["H","H","H","H"],["•","X"],[1,1,1,"•","X"],["X","X","X","X","X"]]}"#,
            )
        );
    }

    #[test]
    fn named_boxes_become_reusable_controllable_no_op_custom_gates() {
        assert_eq!(
            render(
                r#"
                    circuit custom {
                      qubit q[3]
                      gate "U" on q[0], q[1] if q[2]
                      gate "U" on q[0], q[1]
                    }
                "#,
            ),
            concat!(
                "https://algassert.com/quirk#circuit=",
                r#"{"cols":[["~0",1,"•"],["~0"]],"gates":[{"id":"~0","name":"U","circuit":{"cols":[[1,"…"]]}}]}"#,
            )
        );
    }

    #[test]
    fn unsafe_fragment_characters_are_encoded_and_visual_annotations_are_omitted() {
        let url = render(
            r#"
                circuit encoded {
                  qubit q
                  label "omitted & note" on q
                  gate "A&B" on q
                }
            "#,
        );
        assert!(url.starts_with("https://algassert.com/quirk#circuit=%7B"));
        assert!(url.contains("A%26B"));
        assert!(!url.contains("omitted"));
    }

    #[test]
    fn operations_beyond_quirks_sixteen_wire_limit_are_omitted() {
        let url = render(
            r#"
                circuit wide {
                  qubit q[17]
                  x q[15]
                  x q[16]
                }
            "#,
        );
        assert!(url.ends_with(r#"{"cols":[[1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,"X"]]}"#));
    }
}
