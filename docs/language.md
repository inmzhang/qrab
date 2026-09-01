# `.qrab` language

The design follows three rules: declarations read like declarations, operations read in execution order, and backend syntax never leaks into the common circuit model.

Declaration-only modules can be imported before the circuit. Paths are relative to the importing file; duplicate imports load once and cycles are rejected:

```qrab
import "modules/gates.qrab"

circuit main {
  qubit q[2]
  entangle(q[0], q[1])
}
```

Modules may contain `import`, `let`, `style`, and `fn` declarations, but the root file owns the single `circuit`. Path-aware loading is used by the CLI and is also available as `qrab::load_source` for library callers.

Typed values and reusable styles may precede functions and the circuit:

```qrab
let oracle_name = "U_f"
let accent = "#DDEEFF"

style oracle {
  fill: accent
  stroke: blue
  shape: circle
}

circuit styled_value {
  qubit q
  gate oracle_name on q with oracle, opacity: 0.7
}
```

`let` binds a label or color value, while `style` binds checked style fields. A use may add comma-separated overrides. Both are resolved to typed AST values; neither performs textual substitution.

Functions are parsed before the circuit and take wire parameters:

```qrab
fn entangle(control, target) {
  h control
  x target if control
}

fn prepare_ghz(a, b, c) {
  entangle(a, b)
  x c if b
}

circuit ghz {
  qubit q[3]
  prepare_ghz(q[0], q[1], q[2])
}
```

Function bodies contain operations or calls to earlier functions. Calls have exact arity and cannot alias two parameters to the same wire. The compiler lowers typed operation trees; it never performs token or string substitution, so a parameter name appearing in a gate label remains ordinary label text. Declarations, captures, forward calls, and recursion are intentionally not part of the current function subset.

Array slices are end-exclusive: `q[1..4]` selects `q[1]`, `q[2]`, and `q[3]` anywhere a wire list is accepted. A single-wire statement such as `h` rejects a range.

`repeat count { ... }` repeats a parsed operation block, including function calls. `reverse { ... }` emits its operations in reverse source order. Marks can replay an earlier range with `reverse from start_mark to end_mark` (or `to here`); this reverses statement order, not the mathematical action of each gate. `parallel { ... }` aligns independent operations after any prior work and aligns following work after the block. Operations whose visual wire spans overlap are safely serialized. Use an explicit `overlay { ... }` when disjoint operations must occupy one column even though their visual spans collide. Two gates cannot share the same wire/cell, and lifecycle changes and permutations are rejected inside an overlay.

## Current grammar

Newlines terminate statements; `;` is accepted when several short statements belong on one line. `//` starts a comment.

```qrab
circuit teleportation {
  layout {
    orientation: horizontal
    scale: 1.2
    column_gap: 1.5
    wire_gap: 1
    gate_size: 20
    corner_radius: 4
    comment_width: 144
    background: white
  }

  qubit message: "|psi>" -> "|psi>"
  qubit work[2]: "|0>" with stroke: blue
  bit result: "0"
  hidden spacer

  h work[0]
  x work[1] if work[0]
  x work[0] if message
  z work[1] if !message
  gate "oracle" on work[0], work[1] if message with fill: yellow, stroke: blue, width: 24, link: "https://example.com/oracle"
  phase "theta/2" on work[0]
  swap work[0], work[1]
  measure message, work[0] as "Z"
  barrier
  bundle "8" on work[1]
  label "decode" on message, work[1] with fill: yellow
  permute work[1], message, work[0]
  set message to classical
  space work[0] with width: 12
  touch message, work[1]
  end work[1] as "discard"
}
```

Wire declarations are `qubit`, `bit`, or `hidden`, followed by a name or fixed-size array. `: "..."` supplies an input label and `-> "..."` an output label. `ellipsis name` declares one wireless row labeled `...` at both ends, making an omitted register range explicit without a magic wire name.

Strict declarations are the default. An `autowires` statement opts the rest of the circuit into creating unknown quantum wires on first use, labeled with their source names. This provides qpic-style compact sketches while keeping accidental misspellings diagnosable everywhere else.

Built-in gates are `h`, `x`, `y`, `z`, `s`, and `t`. A gate's controls follow `if`; `!wire` is an open/negative control. Arbitrary single- or multi-wire boxes use `gate "label" on ...`. `phase`, `measure`, `swap`, and `barrier` are first-class statements rather than magic gate names.

An unlabeled `measure q` draws a meter. `measure q as "Z"` draws a D-shaped result marker; append `using tag` for a pointed tag marker. Measurement changes each target to a classical wire in either form.

The compiler packs operations into the earliest non-overlapping column while preserving source order. A barrier occupies its selected wire interval; omitting that interval selects the currently active wires.

`set wires to quantum|classical|hidden` changes wire rendering. A transition to `hidden` or `quantum` may add `as "value"` to draw a known-value exit or entry marker. `start` and `end` defer or stop selected wires and may carry an `as "label"`. `bundle` draws a bundle slash, while `label` centers text across a wire span. `permute` lists selected wires in their new visual order; later operations and output labels follow that order. `space` reserves invisible room and `touch` aligns later operations with the preceding slice. An omitted `start` list selects every inactive wire; omitted lists for `end`, `barrier`, `label`/`equals`, `space`, `touch`, `labels`, `brace`, `note`, and `cut` select every active wire. A targetless statement is rejected when that default selection is empty.

Circuit annotations are portable too:

```qrab
labels "data", "work", "flag" on q[0..3] with fill: yellow
brace left "input" on q[0..3] with stroke: blue
note above "prepare" on q[1]
note below "result" on q[1]
equals "encode" on q[0..3] braced both
cut on q[0..3] as "stage" with stroke: red
brace both "repeat" on q[0..3]
```

`labels` accepts either one repeated label or one label per selected wire. Braces may be `left`, `right`, or `both`. `note above|below` annotates the preceding slice without consuming another one. `equals` centers `=` across all wires by default; it accepts another label, an `on` selection, and `braced left|right|both`. A source-local `cut` occupies its own separator column.

Named marks delimit highlighted regions without becoming gates:

```qrab
mark encoding
h q[0]
x q[1] if q[0]
mark encoded

group "encode" from encoding to encoded on q[0..2] with fill: yellow, opacity: 0.2
group "whole pass" from encoding to here with stroke: purple, dash: dashed
```

The end mark is exclusive; `here` means all operations parsed so far. Groups may overlap or nest, and are drawn behind the circuit.

Backend-only code is deliberately isolated from the portable model:

```qrab
backend latex {
  preamble: "\\usepackage{amsmath}"
  before: "\\node at (0, 1) {TikZ-only};"
}

backend typst {
  preamble: "#let backend-label = [Typst-only]"
  after: "#backend-label"
}
```

Each block may repeat `preamble`, `before`, or `after`. These strings are emitted verbatim only for the named target and are the explicit replacement for qpic's preamble/TikZ hooks; they should be reserved for effects the common AST cannot express.

`layout` configures orientation, scale, abstract column/wire gaps, default `gate_size`, permutation `corner_radius`, note `comment_width`, and background. The three size properties use points. A trailing `with` clause accepts `stroke`, `fill`, `width`, `height`, `size`, `shape`, `dash`, `opacity`, and `link`. Shapes are `box`, `circle`, `ellipse`, or `none`; numeric dimensions are points and opacity ranges from 0 to 1. Colors are checked names understood by both backends or quoted six-digit values such as `"#336699"`. A link is a checked absolute HTTP(S) or mailto URL and wraps visible gate or label text in both outputs. Custom target operators use an ordinary gate label and shape, for example `gate "+" on target if control with shape: circle`.

The style grammar is shared so named styles can be reused, but each visual consumes only relevant fields. Gates, measurements, and value markers use the full box/text set; wire declarations use `stroke`, `dash`, and `opacity`; `space` uses only `width` and `height`; line-like `barrier`, `cut`, `touch`, `swap`, and `permute` operations use their stroke, dash, opacity, and applicable width; labels and endpoints portably use `stroke`, `fill`, `opacity`, and `link`. Other fields are accepted and ignored. In particular, `width` or `link` on a wire declaration, `fill` on a barrier, and `shape` on a touch do not affect output.

The language extends this grammar rather than add uppercase directives.
