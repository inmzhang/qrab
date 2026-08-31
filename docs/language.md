# `.qrab` language

The design follows three rules: declarations read like declarations, operations read in execution order, and backend syntax never leaks into the common circuit model.

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

`repeat count { ... }` repeats a parsed operation block, including function calls. `parallel { ... }` aligns independent operations after any prior work and aligns following work after the block. Operations whose visual wire spans overlap are safely serialized rather than drawn on top of one another.

## Current grammar

Newlines terminate statements; `;` is accepted when several short statements belong on one line. `//` starts a comment.

```qrab
circuit teleportation {
  layout {
    orientation: horizontal
    scale: 1.2
    column_gap: 1.5
    wire_gap: 1
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
  gate "oracle" on work[0], work[1] if message with fill: yellow, stroke: blue, width: 24
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

Wire declarations are `qubit`, `bit`, or `hidden`, followed by a name or fixed-size array. `: "..."` supplies an input label and `-> "..."` an output label.

Built-in gates are `h`, `x`, `y`, `z`, `s`, and `t`. A gate's controls follow `if`; `!wire` is an open/negative control. Arbitrary single- or multi-wire boxes use `gate "label" on ...`. `phase`, `measure`, `swap`, and `barrier` are first-class statements rather than magic gate names.

The compiler packs operations into the earliest non-overlapping column while preserving source order. A barrier occupies its selected wire interval; an empty barrier wire list means every wire.

`set wires to quantum|classical|hidden` changes wire rendering. `start` and `end` defer or stop selected wires and may carry an `as "label"`. `bundle` draws a bundle slash, while `label` centers text across a wire span. `permute` lists selected wires in their new visual order; later operations and output labels follow that order. `space` reserves invisible room and `touch` aligns later operations with the preceding slice. Omitting the wire list for `start`, `end`, `label`, `space`, or `touch` selects every wire.

`layout` configures orientation, scale, abstract column/wire gaps, and background. A trailing `with` clause accepts portable `stroke`, `fill`, `width`, `height`, `size`, `shape`, `dash`, and `opacity` properties. Shapes are `box`, `circle`, `ellipse`, or `none`; numeric dimensions are points and opacity ranges from 0 to 1. Colors are checked named colors understood by both backends.

## Planned syntax

The remaining qpic surface will extend the same grammar rather than add uppercase directives:

```qrab
mark start
group "encoding" from start to here on q[0], q[1] {
  fill: green
  radius: 3pt
}
```

Backend-only escape blocks will be explicit and isolated for the few qpic preamble/TikZ hooks that cannot be represented portably.
