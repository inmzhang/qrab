use std::collections::HashMap;
use std::fmt::Write;

use serde_json::{Map, Value};
use thiserror::Error;

const MAX_WIRES: usize = 16;
const MIN_WIRES: usize = 2;

/// An invalid or unsupported Quirk circuit URL.
#[derive(Debug, Error)]
#[error("{message}")]
pub struct QuirkImportError {
    message: String,
}

impl QuirkImportError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

struct GateDefinition {
    label: String,
    height: usize,
}

struct GateCell {
    id: String,
    label: String,
    height: usize,
    custom_effect: bool,
    builtin: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InitialState {
    Zero,
    One,
    Plus,
    Minus,
    I,
    MinusI,
}

impl InitialState {
    fn label(self) -> Option<&'static str> {
        match self {
            Self::Zero => None,
            Self::One => Some("|1>"),
            Self::Plus => Some("|+>"),
            Self::Minus => Some("|->"),
            Self::I => Some("|i>"),
            Self::MinusI => Some("|-i>"),
        }
    }
}

#[derive(Clone, Copy)]
enum ControlKind {
    Positive,
    Negative,
    ParityX,
    ParityY,
    ParityZ,
    XPositive,
    XNegative,
    YPositive,
    YNegative,
}

struct ImportedControl {
    wire: usize,
    kind: ControlKind,
}

/// Converts an escaped or unescaped Quirk circuit URL into checked qrab source.
///
/// Read-only displays are omitted; unsupported state changes return an error.
pub fn from_quirk_url(url: &str) -> Result<String, QuirkImportError> {
    let encoded = url
        .split_once("#circuit=")
        .map(|(_, circuit)| circuit)
        .ok_or_else(|| QuirkImportError::new("expected a Quirk URL containing `#circuit=`"))?
        .trim();
    let value: Value = match serde_json::from_str(encoded) {
        Ok(value) => value,
        Err(_) => {
            let decoded = percent_decode(encoded)?;
            serde_json::from_str(&decoded).map_err(|error| {
                QuirkImportError::new(format!("invalid Quirk circuit JSON: {error}"))
            })?
        }
    };
    let circuit = value
        .as_object()
        .ok_or_else(|| QuirkImportError::new("Quirk circuit JSON must be an object"))?;
    let source = render_source(circuit)?;
    crate::parse(&source).map_err(|error| {
        QuirkImportError::new(format!(
            "Quirk circuit cannot be represented as valid qrab: {error}"
        ))
    })?;
    Ok(source)
}

fn percent_decode(value: &str) -> Result<String, QuirkImportError> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            decoded.push(bytes[index]);
            index += 1;
            continue;
        }
        let Some(pair) = bytes.get(index + 1..index + 3) else {
            return Err(QuirkImportError::new(format!(
                "incomplete percent escape at byte {index}"
            )));
        };
        let high = hex_digit(pair[0]).ok_or_else(|| {
            QuirkImportError::new(format!("invalid percent escape at byte {index}"))
        })?;
        let low = hex_digit(pair[1]).ok_or_else(|| {
            QuirkImportError::new(format!("invalid percent escape at byte {index}"))
        })?;
        decoded.push(high << 4 | low);
        index += 3;
    }
    String::from_utf8(decoded)
        .map_err(|_| QuirkImportError::new("percent-decoded Quirk circuit is not UTF-8"))
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn render_source(circuit: &Map<String, Value>) -> Result<String, QuirkImportError> {
    let definitions = custom_gate_definitions(circuit.get("gates"))?;
    let columns = circuit
        .get("cols")
        .and_then(Value::as_array)
        .ok_or_else(|| QuirkImportError::new("Quirk circuit field `cols` must be an array"))?;
    let mut states = initial_states(circuit.get("init"))?;
    let mut wire_count = states.len().max(MIN_WIRES);
    for (column_index, column) in columns.iter().enumerate() {
        let cells = column.as_array().ok_or_else(|| {
            QuirkImportError::new(format!("Quirk column {column_index} must be an array"))
        })?;
        for (wire, value) in cells.iter().enumerate() {
            if let Some(gate) = gate_cell(value, &definitions)? {
                wire_count = wire_count.max(wire + gate.height);
            }
        }
    }
    if wire_count > MAX_WIRES {
        return Err(QuirkImportError::new(format!(
            "Quirk circuits support at most {MAX_WIRES} wires, found {wire_count}"
        )));
    }
    states.resize(wire_count, InitialState::Zero);

    let shared_state = states
        .iter()
        .all(|state| *state == states[0])
        .then_some(states[0]);
    let array_wires = shared_state.is_some();
    let mut output = String::from("circuit quirk {\n");
    if let Some(state) = shared_state {
        write!(output, "  qubit q[{wire_count}]").expect("writing to a String cannot fail");
        if let Some(label) = state.label() {
            write!(output, ": {}", quote(label)).expect("writing to a String cannot fail");
        }
        output.push('\n');
    } else {
        for (wire, state) in states.iter().enumerate() {
            write!(output, "  qubit q{wire}").expect("writing to a String cannot fail");
            if let Some(label) = state.label() {
                write!(output, ": {}", quote(label)).expect("writing to a String cannot fail");
            }
            output.push('\n');
        }
    }

    for column in columns {
        let cells = column
            .as_array()
            .expect("columns were validated while counting wires");
        let layers = column_layers(cells, &definitions, array_wires, wire_count)?;
        for layer in layers {
            output.push('\n');
            output.push_str("  parallel {\n");
            for operation in layer {
                writeln!(output, "    {operation}").expect("writing to a String cannot fail");
            }
            output.push_str("  }\n");
        }
    }
    output.push_str("}\n");
    Ok(output)
}

fn custom_gate_definitions(
    value: Option<&Value>,
) -> Result<HashMap<String, GateDefinition>, QuirkImportError> {
    let Some(value) = value else {
        return Ok(HashMap::new());
    };
    let gates = value
        .as_array()
        .ok_or_else(|| QuirkImportError::new("Quirk circuit field `gates` must be an array"))?;
    let mut definitions = HashMap::new();
    for (index, value) in gates.iter().enumerate() {
        let gate = value.as_object().ok_or_else(|| {
            QuirkImportError::new(format!("Quirk custom gate {index} must be an object"))
        })?;
        let id = gate.get("id").and_then(Value::as_str).ok_or_else(|| {
            QuirkImportError::new(format!("Quirk custom gate {index} needs a string `id`"))
        })?;
        let label = gate
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
            .unwrap_or(id)
            .to_owned();
        let height = gate_height(gate, id, &definitions)?;
        definitions.insert(id.to_owned(), GateDefinition { label, height });
    }
    Ok(definitions)
}

fn initial_states(value: Option<&Value>) -> Result<Vec<InitialState>, QuirkImportError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let states = value
        .as_array()
        .ok_or_else(|| QuirkImportError::new("Quirk circuit field `init` must be an array"))?;
    states
        .iter()
        .enumerate()
        .map(|(wire, state)| match state {
            Value::Number(number) if number.as_u64() == Some(0) => Ok(InitialState::Zero),
            Value::Number(number) if number.as_u64() == Some(1) => Ok(InitialState::One),
            Value::String(state) if state == "+" => Ok(InitialState::Plus),
            Value::String(state) if state == "-" => Ok(InitialState::Minus),
            Value::String(state) if state == "i" => Ok(InitialState::I),
            Value::String(state) if state == "-i" => Ok(InitialState::MinusI),
            _ => Err(QuirkImportError::new(format!(
                "unsupported initial state on wire {wire}"
            ))),
        })
        .collect()
}

fn gate_cell(
    value: &Value,
    definitions: &HashMap<String, GateDefinition>,
) -> Result<Option<GateCell>, QuirkImportError> {
    if value.as_u64() == Some(1) {
        return Ok(None);
    }
    let (id, inline) = match value {
        Value::String(id) => (id.as_str(), None),
        Value::Object(gate) => {
            let id = gate
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| QuirkImportError::new("Quirk gate objects need a string `id`"))?;
            (id, Some(gate))
        }
        _ => {
            return Err(QuirkImportError::new(
                "Quirk column cells must be gate strings, gate objects, or `1`",
            ));
        }
    };
    if id.is_empty() {
        return Err(QuirkImportError::new("Quirk gate IDs cannot be empty"));
    }
    let definition = definitions.get(id);
    let mut label = inline
        .and_then(|gate| gate.get("name"))
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .or_else(|| definition.map(|gate| gate.label.clone()))
        .unwrap_or_else(|| id.to_owned());
    let argument = inline.and_then(|gate| gate.get("arg"));
    if let Some(argument) = argument {
        let argument = argument
            .as_str()
            .map_or_else(|| argument.to_string(), str::to_owned);
        label = format!("{label}({argument})");
    }
    let height = if let Some(gate) = inline {
        gate_height(gate, id, definitions)?
    } else {
        definition.map_or_else(|| suffix_height(id), |gate| gate.height)
    };
    let custom_effect =
        inline.is_some_and(|gate| gate.contains_key("matrix") || gate.contains_key("circuit"));
    Ok(Some(GateCell {
        id: id.to_owned(),
        label,
        height,
        custom_effect,
        builtin: !custom_effect && argument.is_none(),
    }))
}

fn gate_height(
    gate: &Map<String, Value>,
    id: &str,
    definitions: &HashMap<String, GateDefinition>,
) -> Result<usize, QuirkImportError> {
    if let Some(matrix) = gate.get("matrix") {
        let matrix = matrix
            .as_str()
            .ok_or_else(|| QuirkImportError::new("custom gate `matrix` must be a string"))?;
        return matrix_height(matrix)
            .ok_or_else(|| QuirkImportError::new("cannot determine custom gate matrix size"));
    }
    if let Some(circuit) = gate.get("circuit") {
        let circuit = circuit
            .as_object()
            .ok_or_else(|| QuirkImportError::new("custom gate `circuit` must be an object"))?;
        return circuit_wire_count(circuit, definitions);
    }
    Ok(definitions
        .get(id)
        .map_or_else(|| suffix_height(id), |gate| gate.height))
}

fn circuit_wire_count(
    circuit: &Map<String, Value>,
    definitions: &HashMap<String, GateDefinition>,
) -> Result<usize, QuirkImportError> {
    let columns = circuit
        .get("cols")
        .and_then(Value::as_array)
        .ok_or_else(|| QuirkImportError::new("custom gate circuit needs an array `cols`"))?;
    let mut count = circuit
        .get("init")
        .and_then(Value::as_array)
        .map_or(1, Vec::len);
    for column in columns {
        let cells = column
            .as_array()
            .ok_or_else(|| QuirkImportError::new("custom gate column must be an array"))?;
        for (wire, value) in cells.iter().enumerate() {
            if let Some(gate) = gate_cell(value, definitions)? {
                count = count.max(wire + gate.height);
            }
        }
    }
    Ok(count)
}

fn matrix_height(matrix: &str) -> Option<usize> {
    let mut depth = 0_usize;
    let mut rows = 0_usize;
    for character in matrix.chars() {
        match character {
            '{' => {
                if depth == 1 {
                    rows += 1;
                }
                depth += 1;
            }
            '}' => depth = depth.checked_sub(1)?,
            _ => {}
        }
    }
    (depth == 0 && rows.is_power_of_two() && rows >= 2).then(|| rows.ilog2() as usize)
}

fn suffix_height(id: &str) -> usize {
    // ponytail: Quirk's scalable built-ins encode their height as an ASCII
    // suffix; replace this heuristic with a gate registry if that format changes.
    let digits = id.trim_start_matches(|character: char| !character.is_ascii_digit());
    digits
        .parse::<usize>()
        .ok()
        .filter(|height| (1..=MAX_WIRES).contains(height))
        .unwrap_or(1)
}

fn column_layers(
    cells: &[Value],
    definitions: &HashMap<String, GateDefinition>,
    array_wires: bool,
    wire_count: usize,
) -> Result<Vec<Vec<String>>, QuirkImportError> {
    let mut controls = Vec::new();
    let mut gates = Vec::new();
    let mut swaps = Vec::new();
    let mut measurements = Vec::new();
    let mut omitted_display = false;
    for (wire, value) in cells.iter().enumerate() {
        let Some(gate) = gate_cell(value, definitions)? else {
            continue;
        };
        if gate.builtin && is_read_only_display(&gate.id) {
            omitted_display = true;
            continue;
        }
        if !gate.custom_effect && is_unsupported_state_change(&gate.id) {
            return Err(QuirkImportError::new(format!(
                "unsupported state-changing Quirk gate `{}` on wire {wire}; it cannot be safely omitted",
                gate.id
            )));
        }
        if gate.builtin
            && let Some(kind) = control_kind(&gate.id)
        {
            controls.push(ImportedControl { wire, kind });
        } else if gate.builtin && gate.id == "Swap" {
            swaps.push(wire);
        } else if gate.builtin && gate.id == "Measure" {
            measurements.push(wire);
        } else {
            gates.push((wire, gate));
        }
    }

    let wire = |index| wire_name(index, array_wires);
    let mut operations = Vec::new();
    let condition = control_clause(&controls, array_wires)?;
    if !controls.is_empty() && !measurements.is_empty() {
        return Err(QuirkImportError::new(
            "controlled Quirk measurements cannot be represented faithfully",
        ));
    }
    for (start, gate) in gates {
        if start + gate.height > wire_count {
            return Err(QuirkImportError::new(format!(
                "gate `{}` extends beyond Quirk's wire limit",
                gate.id
            )));
        }
        let targets = (start..start + gate.height)
            .map(&wire)
            .collect::<Vec<_>>()
            .join(", ");
        if controls
            .iter()
            .any(|control| (start..start + gate.height).contains(&control.wire))
        {
            return Err(QuirkImportError::new(format!(
                "control lies inside multi-wire gate `{}`",
                gate.id
            )));
        }
        let named = || format!("gate {} on {targets}{condition}", quote(&gate.label));
        let operation = if gate.builtin && gate.height == 1 {
            match gate.id.as_str() {
                "H" | "X" | "Y" | "Z" => {
                    format!("{} {}{condition}", gate.id.to_lowercase(), wire(start))
                }
                "Z^½" => format!("s {}{condition}", wire(start)),
                "Z^¼" => format!("t {}{condition}", wire(start)),
                "Z^-½" => format!("gate \"S†\" on {}{condition}", wire(start)),
                "Z^-¼" => format!("gate \"T†\" on {}{condition}", wire(start)),
                "Z^⅛" => format!("phase \"4\" on {}{condition}", wire(start)),
                "Z^⅟₁₆" => format!("phase \"5\" on {}{condition}", wire(start)),
                "Z^⅟₃₂" => format!("phase \"6\" on {}{condition}", wire(start)),
                "Z^⅟₆₄" => format!("phase \"7\" on {}{condition}", wire(start)),
                "Z^⅟₁₂₈" => format!("phase \"8\" on {}{condition}", wire(start)),
                _ => named(),
            }
        } else {
            named()
        };
        operations.push(operation);
    }
    if !measurements.is_empty() {
        operations.push(format!(
            "measure {}",
            measurements
                .into_iter()
                .map(&wire)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    for pair in swaps.chunks(2) {
        if let [left, right] = pair {
            if controls.is_empty() {
                operations.push(format!("swap {}, {}", wire(*left), wire(*right)));
            } else {
                operations.push(format!(
                    "gate \"Swap\" on {}, {}{condition}",
                    wire(*left),
                    wire(*right)
                ));
            }
        } else {
            operations.push(format!("gate \"Swap\" on {}{condition}", wire(pair[0])));
        }
    }

    if operations.is_empty() {
        if omitted_display {
            return Ok(Vec::new());
        }
        return Ok(vec![vec![format!("space {}", wire(0))]]);
    }

    let y_controls = controls
        .iter()
        .filter(|control| {
            matches!(
                control.kind,
                ControlKind::YPositive | ControlKind::YNegative
            )
        })
        .map(|control| control.wire)
        .collect::<Vec<_>>();
    let basis_controls = controls
        .iter()
        .filter(|control| {
            matches!(
                control.kind,
                ControlKind::XPositive
                    | ControlKind::XNegative
                    | ControlKind::YPositive
                    | ControlKind::YNegative
            )
        })
        .map(|control| control.wire)
        .collect::<Vec<_>>();
    let mut layers = Vec::new();
    if !y_controls.is_empty() {
        layers.push(
            y_controls
                .iter()
                .map(|wire_index| format!("gate \"S†\" on {}", wire(*wire_index)))
                .collect(),
        );
    }
    if !basis_controls.is_empty() {
        layers.push(
            basis_controls
                .iter()
                .map(|wire_index| format!("h {}", wire(*wire_index)))
                .collect(),
        );
    }
    layers.push(operations);
    if !basis_controls.is_empty() {
        layers.push(
            basis_controls
                .iter()
                .map(|wire_index| format!("h {}", wire(*wire_index)))
                .collect(),
        );
    }
    if !y_controls.is_empty() {
        layers.push(
            y_controls
                .iter()
                .map(|wire_index| format!("s {}", wire(*wire_index)))
                .collect(),
        );
    }
    Ok(layers)
}

fn is_read_only_display(id: &str) -> bool {
    // Mirrors Quirk's `Gates.Displays`, whose members promise no state-vector effect.
    matches!(id, "Bloch" | "Chance" | "Density" | "…")
        || numbered_family(id, "Amps", 1..=MAX_WIRES)
        || numbered_family(id, "Chance", 2..=MAX_WIRES)
        || numbered_family(id, "Sample", 1..=MAX_WIRES)
        || numbered_family(id, "Density", 2..=8)
}

fn numbered_family(id: &str, prefix: &str, range: std::ops::RangeInclusive<usize>) -> bool {
    id.strip_prefix(prefix)
        .and_then(|suffix| suffix.parse::<usize>().ok())
        .is_some_and(|size| range.contains(&size))
}

fn is_unsupported_state_change(id: &str) -> bool {
    // Detectors sample/collapse state (and some reset it), unlike Quirk's displays.
    matches!(
        id,
        "XDetector"
            | "YDetector"
            | "ZDetector"
            | "XDetectControlReset"
            | "YDetectControlReset"
            | "ZDetectControlReset"
            | "__error__"
    )
}

fn control_kind(id: &str) -> Option<ControlKind> {
    match id {
        "•" => Some(ControlKind::Positive),
        "◦" => Some(ControlKind::Negative),
        "xpar" => Some(ControlKind::ParityX),
        "ypar" => Some(ControlKind::ParityY),
        "zpar" => Some(ControlKind::ParityZ),
        "⊖" => Some(ControlKind::XPositive),
        "⊕" => Some(ControlKind::XNegative),
        "(/)" => Some(ControlKind::YPositive),
        "⊗" => Some(ControlKind::YNegative),
        _ => None,
    }
}

fn control_clause(
    controls: &[ImportedControl],
    array_wires: bool,
) -> Result<String, QuirkImportError> {
    if controls.is_empty() {
        return Ok(String::new());
    }
    let mut ordinary = Vec::new();
    let mut parity_x = Vec::new();
    let mut parity_y = Vec::new();
    let mut parity_z = Vec::new();
    for control in controls {
        let wire = wire_name(control.wire, array_wires);
        match control.kind {
            ControlKind::Positive | ControlKind::XPositive | ControlKind::YPositive => {
                ordinary.push(wire);
            }
            ControlKind::Negative | ControlKind::XNegative | ControlKind::YNegative => {
                ordinary.push(format!("!{wire}"));
            }
            ControlKind::ParityX => parity_x.push(wire),
            ControlKind::ParityY => parity_y.push(wire),
            ControlKind::ParityZ => parity_z.push(wire),
        }
    }
    if [&parity_x, &parity_y, &parity_z]
        .iter()
        .filter(|wires| !wires.is_empty())
        .count()
        > 1
    {
        return Err(QuirkImportError::new(
            "mixed-basis parity controls cannot be represented faithfully",
        ));
    }
    for (name, wires) in [
        ("parity_x", parity_x),
        ("parity_y", parity_y),
        ("parity", parity_z),
    ] {
        if !wires.is_empty() {
            ordinary.push(format!("{name}({})", wires.join(", ")));
        }
    }
    Ok(format!(" if {}", ordinary.join(", ")))
}

fn wire_name(index: usize, array_wires: bool) -> String {
    if array_wires {
        format!("q[{index}]")
    } else {
        format!("q{index}")
    }
}

fn quote(value: &str) -> String {
    let mut output = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            '$' => output.push_str("\\$"),
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn import(json: &str) -> Result<String, QuirkImportError> {
        from_quirk_url(&format!("https://algassert.com/quirk#circuit={json}"))
    }

    #[test]
    fn imports_raw_and_escaped_bell_urls_as_valid_qrab() {
        let raw = r#"{"cols":[["H"],["•","X"],["Measure","Measure"]]}"#;
        let escaped = "https://algassert.com/quirk#circuit=%7B%22cols%22%3A%5B%5B%22H%22%5D%2C%5B%22%E2%80%A2%22%2C%22X%22%5D%2C%5B%22Measure%22%2C%22Measure%22%5D%5D%7D";
        let source = import(raw).expect("import raw URL");
        assert_eq!(source, from_quirk_url(escaped).expect("import escaped URL"));
        assert!(source.contains("h q[0]") && source.contains("x q[1] if q[0]"));
    }

    #[test]
    fn imports_partially_escaped_url() {
        let url = concat!(
            "https://algassert.com/quirk#circuit={%22cols%22:",
            "[[1,1,%22H%22,%22H%22,%22H%22,%22H%22],",
            "[1,1,1,1,%22%E2%80%A2%22,%22%E2%80%A2%22,%22X%22],",
            "[1,1,1,1,%22Chance%22,%22Chance%22,%22%E2%80%A2%22],",
            "[1,1,1,1,%22X%22],",
            "[1,1,%22%E2%80%A2%22,%22%E2%80%A2%22,%22X%22],",
            "[1,1,%22Chance%22,%22Chance%22,%22%E2%80%A2%22,%22Chance%22,%22%E2%80%A2%22],",
            "[1,1,1,1,%22%E2%80%A2%22,1,%22%E2%80%A2%22,%22X%22],",
            "[1,1,%22%E2%80%A2%22,%22%E2%80%A2%22,%22%E2%80%A2%22,%22%E2%80%A2%22,1,%22Chance%22]]}",
        );
        let source = from_quirk_url(url).expect("import partially escaped URL");
        assert!(source.contains("h q[2]") && source.contains("x q[6] if q[4], q[5]"));
        assert!(!source.contains("Chance"));
    }

    #[test]
    fn imports_states_custom_gates_and_multi_wire_gates() {
        let source = import(
            r#"{"cols":[["~u"],["QFT3"]],"gates":[{"id":"~u","name":"U$1","circuit":{"cols":[[1,"…"]]}}],"init":[1,"+","-i"]}"#,
        )
        .expect("import custom circuit");
        assert!(source.contains("qubit q0: \"|1>\""));
        assert!(source.contains(r#"gate "U\$1" on q0, q1"#));
        assert!(source.contains(r#"gate "QFT3" on q0, q1, q2"#));
    }

    #[test]
    fn imports_axis_parity_and_swap_columns() {
        let source =
            import(r#"{"cols":[["⊕","X"],["⊗",1,"H"],["xpar","xpar","Z"],["Swap",1,"Swap"],[]]}"#)
                .expect("import control variants");
        assert!(source.contains("x q[1] if !q[0]"));
        assert!(source.contains("h q[2] if !q[0]"));
        assert!(source.contains("z q[2] if parity_x(q[0], q[1])"));
        assert!(source.contains("swap q[0], q[2]"));
        assert!(source.contains("space q[0]"));
    }

    #[test]
    fn omits_read_only_displays_but_rejects_detectors() {
        let source = import(
            r#"{"cols":[["Chance","H"],["Density"],["Bloch"],["Sample1"],["Amps1"],["…"]]}"#,
        )
        .expect("omit simulator displays");
        assert!(source.contains("h q[1]"));
        assert!(
            !["Chance", "Density", "Bloch", "Sample", "Amps", "…"]
                .iter()
                .any(|display| source.contains(display))
        );

        assert!(
            import(r#"{"cols":[["ZDetector"]]}"#)
                .expect_err("detector changes circuit state")
                .to_string()
                .contains("cannot be safely omitted")
        );
        assert!(import(r#"{"cols":[["•","Measure"]]}"#).is_err());
    }

    #[test]
    fn retains_parameterized_and_inline_custom_gates() {
        let source = import(
            r#"{"cols":[[{"id":"X^ft","arg":"sin(pi*t)"}],[{"id":"Chance","name":"C","matrix":"{{1,0},{0,1}}"}],[{"id":"X","name":"UX","matrix":"{{1,0},{0,1}}"}]]}"#,
        )
        .expect("retain state-changing gate details");
        assert!(source.contains(r#"gate "X^ft(sin(pi*t))" on q[0]"#));
        assert!(source.contains(r#"gate "C" on q[0]"#));
        assert!(source.contains(r#"gate "UX" on q[0]"#));
    }

    #[test]
    fn rejects_invalid_or_unrepresentable_inputs() {
        assert!(from_quirk_url("https://algassert.com/quirk").is_err());
        assert!(from_quirk_url("https://algassert.com/quirk#circuit=%ZZ").is_err());
        assert!(import(r#"{"cols":[[{}]]}"#).is_err());
        assert!(import(r#"{"cols":[[["invalid"]]]}"#).is_err());
        assert!(import(r#"{"cols":[["xpar","ypar","X"]]}"#).is_err());
    }
}
