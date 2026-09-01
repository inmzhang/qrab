use std::fmt::Write as _;

use super::*;

// The TikZ backend already lays every element out in absolute centimetres, so
// this backend reuses that geometry verbatim and only swaps the primitives it
// emits. Coordinates arrive here in TikZ convention (centimetres, y pointing
// up) and are converted once, at emit time, into SVG pixels with y pointing
// down.
//
// One pixel per TeX point keeps `style.width` / `style.height` — which the
// language specifies in points — a one-to-one mapping.
//
// There is no `escape svg { … }` block, so `circuit.escapes` is ignored here:
// raw snippets are backend-specific by construction, and SVG has no equivalent
// of a LaTeX preamble or a Typst import.
const PIXELS_PER_CENTIMETER: f32 = POINTS_PER_CENTIMETER;

/// Stroke width in pixels. TikZ defaults to 0.4pt, which is legible in a
/// high-resolution PDF but nearly invisible on a screen at 1x, so the preview
/// backend draws slightly heavier lines.
const STROKE_WIDTH: f32 = 1.0;
const FONT_SIZE: f32 = 10.0;
/// Advance width of one character as a fraction of the font size. SVG has no
/// text metrics, so gate boxes and the drawing bounds are sized from this
/// estimate; it is deliberately generous so labels never overflow their box.
const CHARACTER_ADVANCE: f32 = 0.62;
/// Pixel equivalent of the TikZ `inner sep` that pads node text.
const TEXT_PADDING: f32 = 4.0;
/// Margin around the drawing, matching `\documentclass[border=6pt]`.
const BORDER: f32 = 6.0;
const FONT_FAMILY: &str = "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace";

/// Renders `circuit` as a standalone SVG document.
pub(super) fn render_svg(circuit: &Circuit) -> String {
    let (scheduled, _) = schedule(circuit);
    let last_column = scheduled
        .iter()
        .map(|operation| operation.column)
        .max()
        .unwrap_or(0);
    let end_x = (last_column + 2) as f32 * circuit.layout.column_gap;

    let mut canvas = Canvas::default();
    for (group_index, group) in circuit.groups.iter().enumerate() {
        draw_group(&mut canvas, circuit, &scheduled, group_index, group);
    }
    for (wire_index, wire) in circuit.wires.iter().enumerate() {
        draw_wire(&mut canvas, circuit, &scheduled, wire_index, wire, end_x);
    }
    for operation in &scheduled {
        draw_operation(&mut canvas, circuit, operation);
    }

    assemble(circuit, canvas)
}

// ==============================================================================
// Document assembly
// ==============================================================================

fn assemble(circuit: &Circuit, canvas: Canvas) -> String {
    // An empty circuit still has to produce a valid document, so fall back to a
    // one-pixel box rather than an inverted or zero-area viewBox.
    let bounds = canvas.bounds.unwrap_or(Bounds {
        left: 0.0,
        top: 0.0,
        right: 1.0,
        bottom: 1.0,
    });

    // Vertical circuits are drawn horizontally and then rotated, exactly as the
    // TikZ backend does with `rotate=90`. In SVG's y-down space the equivalent
    // rotation maps (x, y) to (y, -x), which is `rotate(-90)`.
    let vertical = circuit.layout.orientation == Orientation::Vertical;
    let (left, top, width, height) = if vertical {
        (
            bounds.top,
            -bounds.right,
            bounds.bottom - bounds.top,
            bounds.right - bounds.left,
        )
    } else {
        (
            bounds.left,
            bounds.top,
            bounds.right - bounds.left,
            bounds.bottom - bounds.top,
        )
    };
    let (left, top) = (left - BORDER, top - BORDER);
    let (width, height) = (width + 2.0 * BORDER, height + 2.0 * BORDER);
    let scale = circuit.layout.scale;

    let mut output = String::new();
    emit!(
        output,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" \
         width=\"{:.3}\" height=\"{:.3}\" viewBox=\"{left:.3} {top:.3} {width:.3} {height:.3}\" \
         font-family=\"{FONT_FAMILY}\" font-size=\"{FONT_SIZE}\">",
        width * scale,
        height * scale
    );
    emit!(output, "  <title>{}</title>", escape_text(&circuit.name));
    emit!(
        output,
        "  <rect x=\"{left:.3}\" y=\"{top:.3}\" width=\"{width:.3}\" height=\"{height:.3}\" fill=\"{}\"/>",
        svg_color(&circuit.layout.background)
    );
    if vertical {
        output.push_str("  <g transform=\"rotate(-90)\">\n");
    } else {
        output.push_str("  <g>\n");
    }
    output.push_str(&canvas.body);
    output.push_str("  </g>\n");
    output.push_str("</svg>\n");
    output
}

// ==============================================================================
// Canvas
// ==============================================================================

#[derive(Clone, Copy)]
struct Bounds {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

/// Accumulates SVG primitives together with the bounding box they occupy.
///
/// Every method takes TikZ coordinates in centimetres; the conversion to SVG
/// pixels happens in [`Canvas::point`] so that the ported drawing code reads the
/// same as its TikZ counterpart.
#[derive(Default)]
struct Canvas {
    body: String,
    bounds: Option<Bounds>,
}

impl Canvas {
    fn point(&mut self, x: f32, y: f32) -> (f32, f32) {
        let point = (x * PIXELS_PER_CENTIMETER, -y * PIXELS_PER_CENTIMETER);
        self.cover(point.0, point.1);
        point
    }

    fn cover(&mut self, x: f32, y: f32) {
        self.bounds = Some(match self.bounds {
            None => Bounds {
                left: x,
                top: y,
                right: x,
                bottom: y,
            },
            Some(bounds) => Bounds {
                left: bounds.left.min(x),
                top: bounds.top.min(y),
                right: bounds.right.max(x),
                bottom: bounds.bottom.max(y),
            },
        });
    }

    fn line(&mut self, start_x: f32, start_y: f32, end_x: f32, end_y: f32, attributes: &str) {
        let (x1, y1) = self.point(start_x, start_y);
        let (x2, y2) = self.point(end_x, end_y);
        emit!(
            self.body,
            "    <line x1=\"{x1:.3}\" y1=\"{y1:.3}\" x2=\"{x2:.3}\" y2=\"{y2:.3}\"{attributes}/>"
        );
    }

    /// Draws a rectangle from two opposite corners, in TikZ order.
    fn rectangle(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, radius: f32, attributes: &str) {
        let (left, top) = self.point(x1.min(x2), y1.max(y2));
        let (right, bottom) = self.point(x1.max(x2), y1.min(y2));
        let corner = if radius > 0.0 {
            format!(" rx=\"{radius:.3}\"")
        } else {
            String::new()
        };
        emit!(
            self.body,
            "    <rect x=\"{left:.3}\" y=\"{top:.3}\" width=\"{:.3}\" height=\"{:.3}\"{corner}{attributes}/>",
            right - left,
            bottom - top
        );
    }

    fn circle(&mut self, x: f32, y: f32, radius: f32, attributes: &str) {
        let (cx, cy) = self.point(x, y);
        self.cover(cx - radius, cy - radius);
        self.cover(cx + radius, cy + radius);
        emit!(
            self.body,
            "    <circle cx=\"{cx:.3}\" cy=\"{cy:.3}\" r=\"{radius:.3}\"{attributes}/>"
        );
    }

    fn ellipse(&mut self, x: f32, y: f32, radius_x: f32, radius_y: f32, attributes: &str) {
        let (cx, cy) = self.point(x, y);
        self.cover(cx - radius_x, cy - radius_y);
        self.cover(cx + radius_x, cy + radius_y);
        emit!(
            self.body,
            "    <ellipse cx=\"{cx:.3}\" cy=\"{cy:.3}\" rx=\"{radius_x:.3}\" ry=\"{radius_y:.3}\"{attributes}/>"
        );
    }

    /// Emits a path whose `d` was built from already-converted pixel points.
    fn path(&mut self, data: &str, attributes: &str) {
        emit!(self.body, "    <path d=\"{data}\"{attributes}/>");
    }

    #[allow(clippy::too_many_arguments)]
    fn text(&mut self, x: f32, y: f32, value: &str, anchor: Anchor, style: &Style, boxed: bool) {
        let (px, py) = self.point(x, y);
        let width = text_width(value);
        let (offset_x, offset_y) = anchor.offset(width);
        let (px, py) = (px + offset_x, py + offset_y);
        self.cover(px - width / 2.0, py - FONT_SIZE * 0.8);
        self.cover(px + width / 2.0, py + FONT_SIZE * 0.8);

        if boxed && let Some(fill) = &style.fill {
            let mut attributes = format!(" fill=\"{}\"", svg_color(fill));
            push_opacity(&mut attributes, style);
            emit!(
                self.body,
                "    <rect x=\"{:.3}\" y=\"{:.3}\" width=\"{:.3}\" height=\"{:.3}\"{attributes}/>",
                px - width / 2.0 - TEXT_PADDING / 2.0,
                py - FONT_SIZE * 0.72,
                width + TEXT_PADDING,
                FONT_SIZE * 1.1
            );
        }

        let mut attributes = format!(
            " text-anchor=\"middle\" fill=\"{}\"",
            svg_color(style.stroke.as_deref().unwrap_or("black"))
        );
        push_opacity(&mut attributes, style);
        let body = escape_text(value);
        let body = match &style.link {
            Some(link) => format!(
                "<a xlink:href=\"{0}\" href=\"{0}\">{body}</a>",
                escape_text(link)
            ),
            None => body,
        };
        emit!(
            self.body,
            "    <text x=\"{px:.3}\" y=\"{py:.3}\"{attributes}>{body}</text>"
        );
    }
}

/// Where a text node sits relative to its anchor point, mirroring the TikZ
/// `anchor=` options used by the LaTeX backend.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Anchor {
    Center,
    East,
    West,
    South,
    North,
    SouthWest,
}

impl Anchor {
    /// Offsets the baseline point so the estimated glyph box lands on the
    /// requested side. SVG's `dominant-baseline` is unevenly supported outside
    /// browsers, so vertical placement uses explicit baseline arithmetic.
    fn offset(self, width: f32) -> (f32, f32) {
        let middle = FONT_SIZE * 0.35;
        match self {
            Anchor::Center => (0.0, middle),
            Anchor::East => (-width / 2.0 - TEXT_PADDING, middle),
            Anchor::West => (width / 2.0 + TEXT_PADDING, middle),
            Anchor::South => (0.0, -TEXT_PADDING * 0.5),
            Anchor::North => (0.0, FONT_SIZE * 0.85),
            Anchor::SouthWest => (width / 2.0, -TEXT_PADDING * 0.5),
        }
    }
}

fn text_width(value: &str) -> f32 {
    value.chars().count() as f32 * CHARACTER_ADVANCE * FONT_SIZE
}

// ==============================================================================
// Groups, wires, and operations
// ==============================================================================

fn draw_group(
    canvas: &mut Canvas,
    circuit: &Circuit,
    scheduled: &[Scheduled<'_>],
    group_index: usize,
    group: &Group,
) {
    let (first_column, last_column, first_row, last_row) =
        group_bounds(group, scheduled, circuit.wires.len());
    let left = first_column as f32 * circuit.layout.column_gap - 0.52;
    let right = last_column as f32 * circuit.layout.column_gap + 0.52;
    let top = -(first_row as f32) * circuit.layout.wire_gap + 0.48;
    let bottom = -(last_row as f32) * circuit.layout.wire_gap - 0.48;

    let stroke = if group.style.shape == Some(Shape::None) {
        "none"
    } else {
        group.style.stroke.as_deref().unwrap_or("black")
    };
    let mut attributes = format!(
        " fill=\"{}\" stroke=\"{}\" stroke-width=\"{STROKE_WIDTH}\"",
        svg_color(group.style.fill.as_deref().unwrap_or("none")),
        svg_color(stroke)
    );
    push_dash(&mut attributes, &group.style);
    push_opacity(&mut attributes, &group.style);
    let radius = if matches!(group.style.shape, Some(Shape::Circle | Shape::Ellipse)) {
        5.0
    } else {
        0.0
    };
    canvas.rectangle(left, top, right, bottom, radius, &attributes);

    // Stagger nested group labels so their text does not overlap, matching the
    // LaTeX backend's per-index vertical offset.
    canvas.text(
        left,
        top + group_index as f32 * 0.24,
        &group.label,
        Anchor::SouthWest,
        &Style::default(),
        false,
    );
}

fn draw_wire(
    canvas: &mut Canvas,
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
            draw_wire_run(canvas, kind, start_x, x - half_width, source_y, &wire.style);
            draw_permutation_curve(
                canvas,
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
            draw_wire_run(canvas, kind, start_x, transition_x, y, &wire.style);
            kind = next_kind;
            start_x = transition_x;
        }
    }

    let y = -(row as f32) * circuit.layout.wire_gap;
    draw_wire_run(canvas, kind, start_x, end_x, y, &wire.style);
    if initial_kind != WireKind::Hidden || wire.ellipsis {
        let input = wire.input.as_deref().unwrap_or(&wire.name);
        let input_y = -(wire_index as f32) * circuit.layout.wire_gap;
        canvas.text(0.0, input_y, input, Anchor::East, &Style::default(), false);
    }
    if (kind != WireKind::Hidden || wire.ellipsis)
        && let Some(label) = &wire.output
    {
        canvas.text(end_x, y, label, Anchor::West, &Style::default(), false);
    }
}

fn draw_wire_run(
    canvas: &mut Canvas,
    kind: WireKind,
    start_x: f32,
    end_x: f32,
    y: f32,
    style: &Style,
) {
    if end_x <= start_x {
        return;
    }
    let attributes = line_attributes(style);
    match kind {
        WireKind::Quantum => canvas.line(start_x, y, end_x, y, &attributes),
        // A classical wire is drawn as the same double rule the TikZ backend
        // uses, so both backends agree on where a measurement changes the wire.
        WireKind::Classical => {
            for offset in [-0.035, 0.035] {
                canvas.line(start_x, y + offset, end_x, y + offset, &attributes);
            }
        }
        WireKind::Hidden => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_permutation_curve(
    canvas: &mut Canvas,
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
    let attributes = line_attributes(style);
    for offset in offsets {
        if corner_radius == 0.0 {
            canvas.line(
                start_x,
                source_y + offset,
                end_x,
                destination_y + offset,
                &attributes,
            );
            continue;
        }
        let bend = (corner_radius / 4.0).min(1.0);
        let first_control = start_x + (control_x - start_x) * bend;
        let second_control = end_x - (end_x - control_x) * bend;
        let start = canvas.point(start_x, source_y + offset);
        let first = canvas.point(first_control, source_y + offset);
        let second = canvas.point(second_control, destination_y + offset);
        let end = canvas.point(end_x, destination_y + offset);
        let data = format!(
            "M {:.3} {:.3} C {:.3} {:.3} {:.3} {:.3} {:.3} {:.3}",
            start.0, start.1, first.0, first.1, second.0, second.1, end.0, end.1
        );
        canvas.path(&data, &attributes);
    }
}

fn draw_operation(canvas: &mut Canvas, circuit: &Circuit, operation: &Scheduled<'_>) {
    let layout = &circuit.layout;
    let wire_gap = layout.wire_gap;
    let x = (operation.column + 1) as f32 * layout.column_gap;
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
            draw_gate(
                canvas,
                x,
                layout,
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
                draw_measurement(
                    canvas,
                    x,
                    layout,
                    operation.positions[*target],
                    label.as_deref(),
                    *shape,
                    operation.style,
                );
            }
        }
        OperationKind::Swap { left, right } => {
            let left_y = -(operation.positions[*left] as f32) * wire_gap;
            let right_y = -(operation.positions[*right] as f32) * wire_gap;
            let attributes = line_attributes(operation.style);
            canvas.line(x, left_y, x, right_y, &attributes);
            draw_cross(canvas, x, left_y, operation.style);
            draw_cross(canvas, x, right_y, operation.style);
        }
        OperationKind::Barrier { .. } => {
            let top = -(operation.first as f32) * wire_gap + 0.42;
            let bottom = -(operation.last as f32) * wire_gap - 0.42;
            let mut barrier_style = operation.style.clone();
            barrier_style.dashed = true;
            canvas.line(x, top, x, bottom, &line_attributes(&barrier_style));
        }
        OperationKind::WireChange { wires, kind, label } => {
            if let Some(label) = label {
                for wire in wires {
                    draw_value_transition(
                        canvas,
                        x,
                        -(operation.positions[*wire] as f32) * wire_gap,
                        label,
                        *kind,
                        operation.style,
                        &layout.background,
                    );
                }
            }
        }
        OperationKind::Endpoint {
            wires,
            start,
            label,
        } => {
            let attributes = line_attributes(operation.style);
            for wire in expanded_wires(wires, circuit.wires.len()) {
                let y = -(operation.positions[wire] as f32) * wire_gap;
                canvas.line(x, y - 0.13, x, y + 0.13, &attributes);
                if let Some(label) = label {
                    let anchor = if *start { Anchor::East } else { Anchor::West };
                    canvas.text(x, y, label, anchor, operation.style, true);
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
                draw_brace(
                    canvas,
                    x,
                    wire_gap,
                    first,
                    last,
                    label,
                    *side,
                    operation.style,
                    &layout.background,
                );
            } else {
                let y = -((first + last) as f32) * wire_gap / 2.0;
                canvas.text(x, y, label, Anchor::Center, operation.style, true);
            }
        }
        OperationKind::Bundle { wire, label } => {
            let y = -(operation.positions[*wire] as f32) * wire_gap;
            canvas.line(
                x - 0.10,
                y - 0.15,
                x + 0.10,
                y + 0.15,
                &line_attributes(operation.style),
            );
            canvas.text(
                x + 0.08,
                y + 0.08,
                label,
                Anchor::SouthWest,
                &Style::default(),
                false,
            );
        }
        OperationKind::Permute { .. } => {}
        OperationKind::Phantom { wires } => {
            // Invisible, but it still reserves space in the other backends, so
            // extend the drawing bounds by the requested box.
            if operation.style.width.is_some() || operation.style.height.is_some() {
                let half_width = operation.style.width.unwrap_or(0.0) / 2.0;
                let half_height = operation.style.height.unwrap_or(0.0) / 2.0;
                for wire in expanded_wires(wires, circuit.wires.len()) {
                    let y = -(operation.positions[wire] as f32) * wire_gap;
                    let (px, py) = canvas.point(x, y);
                    canvas.cover(px - half_width, py - half_height);
                    canvas.cover(px + half_width, py + half_height);
                }
            }
        }
        OperationKind::Touch { .. } => {
            if has_line_style(operation.style) {
                let top = -(operation.first as f32) * wire_gap + 0.35;
                let bottom = -(operation.last as f32) * wire_gap - 0.35;
                canvas.line(x, top, x, bottom, &line_attributes(operation.style));
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
                let y = -(operation.positions[*wire] as f32) * wire_gap;
                canvas.text(x, y, label, Anchor::Center, operation.style, true);
            }
        }
        OperationKind::Brace { wires, label, side } => {
            let mut rows = selected_wires(wires, circuit.wires.len())
                .iter()
                .map(|wire| operation.positions[*wire])
                .collect::<Vec<_>>();
            rows.sort_unstable();
            draw_brace(
                canvas,
                x,
                wire_gap,
                *rows.first().expect("circuit has a wire"),
                *rows.last().expect("circuit has a wire"),
                label,
                *side,
                operation.style,
                &layout.background,
            );
        }
        OperationKind::Note { wires, text, side } => {
            let mut rows = selected_wires(wires, circuit.wires.len())
                .iter()
                .map(|wire| operation.positions[*wire])
                .collect::<Vec<_>>();
            rows.sort_unstable();
            let above = *side == NoteSide::Above;
            let row = if above {
                *rows.first().expect("circuit has a wire") as f32
            } else {
                *rows.last().expect("circuit has a wire") as f32
            };
            let y = -row * wire_gap + if above { 0.42 } else { -0.42 };
            let anchor = if above { Anchor::South } else { Anchor::North };
            // TODO: notes are rendered on one line. The LaTeX and Typst
            // backends wrap them to `layout.comment_width`; wrapping here needs
            // real text metrics, which SVG does not provide.
            canvas.text(x, y, text, anchor, &Style::default(), false);
        }
        OperationKind::Cut { label, .. } => {
            let top = -(operation.first as f32) * wire_gap + 0.42;
            let bottom = -(operation.last as f32) * wire_gap - 0.42;
            let mut cut_style = operation.style.clone();
            cut_style.dashed = true;
            canvas.line(x, top, x, bottom, &line_attributes(&cut_style));
            if let Some(label) = label {
                canvas.text(x, top, label, Anchor::South, &Style::default(), false);
            }
        }
    }
}

// ==============================================================================
// Individual elements
// ==============================================================================

fn draw_gate(
    canvas: &mut Canvas,
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
        canvas.line(
            x,
            -(first as f32) * wire_gap,
            x,
            -(last as f32) * wire_gap,
            &line_attributes(style),
        );
        for control in controls {
            let y = -(control.wire as f32) * wire_gap;
            canvas.circle(x, y, 2.2, &marker_attributes(style, control.positive));
        }
    }

    if targets.len() > 1 {
        let first = *targets.iter().min().expect("gate has a target");
        let last = *targets.iter().max().expect("gate has a target");
        let midpoint = -((first + last) as f32) * wire_gap / 2.0;
        let height = style.height.unwrap_or_else(|| {
            (last - first) as f32 * wire_gap * PIXELS_PER_CENTIMETER + gate_size
        });
        draw_node(canvas, x, midpoint, label, gate_size, height, style);
        return;
    }

    let y = -(targets[0] as f32) * wire_gap;
    // A controlled X or Z with no hyperlink gets the conventional target
    // notation instead of a labelled box, matching the other backends.
    if !controls.is_empty() && label == "X" && style.link.is_none() {
        canvas.circle(x, y, 4.0, &marker_attributes(style, false));
        let attributes = line_attributes(style);
        canvas.line(x - 0.14, y, x + 0.14, y, &attributes);
        canvas.line(x, y - 0.14, x, y + 0.14, &attributes);
    } else if !controls.is_empty() && label == "Z" && style.link.is_none() {
        canvas.circle(x, y, 2.2, &marker_attributes(style, true));
    } else {
        draw_node(canvas, x, y, label, gate_size, gate_size, style);
    }
}

/// Draws a labelled box, circle, or ellipse sized like a TikZ node: at least
/// the requested minimum, but grown to fit the label.
fn draw_node(
    canvas: &mut Canvas,
    x: f32,
    y: f32,
    label: &str,
    minimum_width: f32,
    minimum_height: f32,
    style: &Style,
) {
    let width = style
        .width
        .unwrap_or(minimum_width)
        .max(text_width(label) + 2.0 * TEXT_PADDING);
    let height = style.height.unwrap_or(minimum_height);
    if style.shape != Some(Shape::None) {
        let attributes = node_attributes(style);
        match style.shape {
            Some(Shape::Circle) => {
                let radius = width.max(height) / 2.0;
                canvas.circle(x, y, radius, &attributes);
            }
            Some(Shape::Ellipse) => {
                canvas.ellipse(x, y, width / 2.0, height / 2.0, &attributes);
            }
            Some(Shape::Box | Shape::None) | None => {
                let half_width = width / (2.0 * PIXELS_PER_CENTIMETER);
                let half_height = height / (2.0 * PIXELS_PER_CENTIMETER);
                canvas.rectangle(
                    x - half_width,
                    y - half_height,
                    x + half_width,
                    y + half_height,
                    0.0,
                    &attributes,
                );
            }
        }
    }
    let mut label_style = style.clone();
    label_style.fill = None;
    canvas.text(x, y, label, Anchor::Center, &label_style, false);
}

fn draw_measurement(
    canvas: &mut Canvas,
    x: f32,
    layout: &Layout,
    target: usize,
    label: Option<&str>,
    shape: MeasurementShape,
    style: &Style,
) {
    let y = -(target as f32) * layout.wire_gap;
    let width = style.width.unwrap_or(layout.gate_size) / PIXELS_PER_CENTIMETER;
    let height = style.height.unwrap_or(layout.gate_size) / PIXELS_PER_CENTIMETER;
    let (left, right) = (x - width / 2.0, x + width / 2.0);
    let (top, bottom) = (y + height / 2.0, y - height / 2.0);

    if let Some(label) = label {
        let outline = marker_attributes(style, false);
        match shape {
            MeasurementShape::D => {
                let arc_x = right - height / 2.0;
                let start = canvas.point(left, bottom);
                let arc_start = canvas.point(arc_x, bottom);
                let arc_end = canvas.point(arc_x, top);
                let corner = canvas.point(left, top);
                let radius = height / 2.0 * PIXELS_PER_CENTIMETER;
                let data = format!(
                    "M {:.3} {:.3} L {:.3} {:.3} A {radius:.3} {radius:.3} 0 0 0 {:.3} {:.3} L {:.3} {:.3} Z",
                    start.0,
                    start.1,
                    arc_start.0,
                    arc_start.1,
                    arc_end.0,
                    arc_end.1,
                    corner.0,
                    corner.1
                );
                canvas.path(&data, &outline);
            }
            MeasurementShape::Tag => {
                let point = (height / 2.0).min(width / 3.0);
                let corners = [
                    (left, y),
                    (left + point, top),
                    (right, top),
                    (right, bottom),
                    (left + point, bottom),
                ]
                .map(|(px, py)| canvas.point(px, py));
                let mut data = format!("M {:.3} {:.3}", corners[0].0, corners[0].1);
                for corner in &corners[1..] {
                    let _ = write!(data, " L {:.3} {:.3}", corner.0, corner.1);
                }
                data.push_str(" Z");
                canvas.path(&data, &outline);
            }
        }
        let mut label_style = style.clone();
        label_style.fill = None;
        canvas.text(x, y, label, Anchor::Center, &label_style, false);
        return;
    }

    // The unlabelled meter is a box with a gauge arc and a needle.
    canvas.rectangle(
        left,
        bottom,
        right,
        top,
        0.0,
        &marker_attributes(style, false),
    );
    let arc_start = canvas.point(x - 0.22, y + 0.10);
    let arc_end = canvas.point(x + 0.22, y + 0.10);
    let arc_radius = 0.22 * PIXELS_PER_CENTIMETER;
    let arc_attributes = line_attributes(style);
    canvas.path(
        &format!(
            "M {:.3} {:.3} A {arc_radius:.3} {arc_radius:.3} 0 0 1 {:.3} {:.3}",
            arc_start.0, arc_start.1, arc_end.0, arc_end.1
        ),
        &arc_attributes,
    );
    canvas.line(x, y + 0.10, x + 0.17, y - 0.12, &line_attributes(style));
}

fn draw_value_transition(
    canvas: &mut Canvas,
    x: f32,
    y: f32,
    label: &str,
    kind: WireKind,
    style: &Style,
    background: &str,
) {
    let width = style
        .width
        .map_or(0.48, |width| width / PIXELS_PER_CENTIMETER);
    let height = style
        .height
        .map_or(0.34, |height| height / PIXELS_PER_CENTIMETER);
    let (left, right) = (x - width / 2.0, x + width / 2.0);
    let (top, bottom) = (y + height / 2.0, y - height / 2.0);

    // The filled patch hides the wire behind the label; the single rule marks
    // the edge on which the wire's value changes.
    let mut fill_attributes = format!(
        " fill=\"{}\" stroke=\"none\"",
        svg_color(style.fill.as_deref().unwrap_or(background))
    );
    push_opacity(&mut fill_attributes, style);
    canvas.rectangle(left, bottom, right, top, 0.0, &fill_attributes);

    let edge = if kind == WireKind::Hidden {
        left
    } else {
        right
    };
    canvas.line(edge, bottom, edge, top, &line_attributes(style));

    let mut label_style = style.clone();
    label_style.fill = None;
    label_style.shape = None;
    canvas.text(x, y, label, Anchor::Center, &label_style, false);
}

#[allow(clippy::too_many_arguments)]
fn draw_brace(
    canvas: &mut Canvas,
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
        let mut attributes = format!(
            " fill=\"none\" stroke=\"{}\" stroke-width=\"{STROKE_WIDTH}\"",
            svg_color(style.stroke.as_deref().unwrap_or("black"))
        );
        push_opacity(&mut attributes, style);
        draw_curly_brace(canvas, x + offset, top, bottom, mirror, &attributes);
    }
    let mut label_style = style.clone();
    label_style.fill.get_or_insert_with(|| background.into());
    canvas.text(
        x,
        (top + bottom) / 2.0,
        label,
        Anchor::Center,
        &label_style,
        true,
    );
}

/// Approximates TikZ's `decoration={brace,amplitude=4pt}`.
///
/// The two quadratic segments meet at a reflected control point, which makes
/// the centre reverse direction and form the pointed tip of a brace rather than
/// a smooth bulge.
fn draw_curly_brace(
    canvas: &mut Canvas,
    x: f32,
    top: f32,
    bottom: f32,
    mirror: bool,
    attributes: &str,
) {
    const AMPLITUDE: f32 = 4.0;
    let reach = if mirror { -AMPLITUDE } else { AMPLITUDE } / 2.0;
    let (spine_x, top_y) = canvas.point(x, top);
    let (_, bottom_y) = canvas.point(x, bottom);
    canvas.cover(spine_x - 2.0 * reach, top_y);
    canvas.cover(spine_x - 2.0 * reach, bottom_y);

    let middle_y = (top_y + bottom_y) / 2.0;
    let data = format!(
        "M {spine_x:.3} {top_y:.3} \
         Q {:.3} {top_y:.3} {:.3} {:.3} \
         T {:.3} {middle_y:.3} \
         Q {:.3} {middle_y:.3} {:.3} {:.3} \
         T {spine_x:.3} {bottom_y:.3}",
        spine_x - reach,
        spine_x - reach,
        (top_y + middle_y) / 2.0,
        spine_x - 2.0 * reach,
        spine_x - reach,
        spine_x - reach,
        (middle_y + bottom_y) / 2.0
    );
    canvas.path(&data, attributes);
}

fn draw_cross(canvas: &mut Canvas, x: f32, y: f32, style: &Style) {
    let attributes = line_attributes(style);
    canvas.line(x - 0.11, y - 0.11, x + 0.11, y + 0.11, &attributes);
    canvas.line(x - 0.11, y + 0.11, x + 0.11, y - 0.11, &attributes);
}

// ==============================================================================
// Attributes and colors
// ==============================================================================

fn line_attributes(style: &Style) -> String {
    let mut attributes = format!(
        " fill=\"none\" stroke=\"{}\" stroke-width=\"{STROKE_WIDTH}\"",
        svg_color(style.stroke.as_deref().unwrap_or("black"))
    );
    push_dash(&mut attributes, style);
    push_opacity(&mut attributes, style);
    attributes
}

/// Attributes for a filled marker: control dots, meter outlines, and swap
/// endpoints. `filled` selects the solid form of a control dot.
fn marker_attributes(style: &Style, filled: bool) -> String {
    let stroke = style.stroke.as_deref().unwrap_or("black");
    let fill = style
        .fill
        .as_deref()
        .unwrap_or(if filled { stroke } else { "white" });
    let mut attributes = format!(
        " fill=\"{}\" stroke=\"{}\" stroke-width=\"{STROKE_WIDTH}\"",
        svg_color(fill),
        svg_color(stroke)
    );
    push_dash(&mut attributes, style);
    push_opacity(&mut attributes, style);
    attributes
}

fn node_attributes(style: &Style) -> String {
    let mut attributes = format!(
        " fill=\"{}\" stroke=\"{}\" stroke-width=\"{STROKE_WIDTH}\"",
        svg_color(style.fill.as_deref().unwrap_or("white")),
        svg_color(style.stroke.as_deref().unwrap_or("black"))
    );
    push_dash(&mut attributes, style);
    push_opacity(&mut attributes, style);
    attributes
}

fn push_dash(attributes: &mut String, style: &Style) {
    if style.dashed {
        attributes.push_str(" stroke-dasharray=\"3 2\"");
    }
}

fn push_opacity(attributes: &mut String, style: &Style) {
    if let Some(opacity) = style.opacity {
        let _ = write!(attributes, " opacity=\"{opacity:.3}\"");
    }
}

/// Maps a portable color to CSS. Hex values pass through; the twelve named
/// colors use their `xcolor` definitions rather than the CSS keywords of the
/// same name, several of which differ (CSS `green` is half-intensity, and
/// `purple`, `olive`, and `lime` differ outright), so the SVG matches the
/// LaTeX output.
fn svg_color(color: &str) -> &str {
    match color {
        "black" => "#000000",
        "white" => "#FFFFFF",
        "gray" => "#808080",
        "red" => "#FF0000",
        "green" => "#00FF00",
        "blue" => "#0000FF",
        "teal" => "#008080",
        "purple" => "#BF0040",
        "orange" => "#FF8000",
        "yellow" => "#FFFF00",
        "olive" => "#808000",
        "lime" => "#BFFF00",
        other => other,
    }
}

fn escape_text(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            '\n' | '\r' => escaped.push(' '),
            _ => escaped.push(character),
        }
    }
    escaped
}
