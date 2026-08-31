# `.qrab` language

The design follows three rules: declarations read like declarations, operations read in execution order, and backend syntax never leaks into the common circuit model.

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
}
```

Wire declarations are `qubit`, `bit`, or `hidden`, followed by a name or fixed-size array. `: "..."` supplies an input label and `-> "..."` an output label.

Built-in gates are `h`, `x`, `y`, `z`, `s`, and `t`. A gate's controls follow `if`; `!wire` is an open/negative control. Arbitrary single- or multi-wire boxes use `gate "label" on ...`. `phase`, `measure`, `swap`, and `barrier` are first-class statements rather than magic gate names.

The compiler packs operations into the earliest non-overlapping column while preserving source order. A barrier occupies its selected wire interval; an empty barrier wire list means every wire.

`layout` configures orientation, scale, abstract column/wire gaps, and background. A trailing `with` clause accepts portable `stroke`, `fill`, `width`, `height`, `size`, `shape`, `dash`, and `opacity` properties. Shapes are `box`, `circle`, `ellipse`, or `none`; numeric dimensions are points and opacity ranges from 0 to 1. Colors are checked named colors understood by both backends.

## Planned syntax

The remaining qpic surface will extend the same grammar rather than add uppercase directives:

```qrab
fn majority(a, b, c) {
  x b if a
  x c if a, b
}

parallel {
  h q[0]
  h q[1]
}

mark start
group "encoding" from start to here on q[0], q[1] {
  fill: green
  radius: 3pt
}
```

Functions will bind wire parameters in the parsed syntax tree; qrab will not implement qpic-style textual `DEFINE` substitution. Backend-only escape blocks will be explicit and isolated for the few qpic preamble/TikZ hooks that cannot be represented portably.
