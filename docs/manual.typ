// The qrab manual.
//
// Every snippet in this document is a real file under `manual/examples`, and
// every diagram beside a snippet is rendered from that same file by `qrab`
// itself through `cargo run -p xtask`. Neither can drift from the other, and
// neither can drift from the compiler.
//
// Build with: typst compile --root . docs/manual.typ docs/manual.pdf

// Read from the manifest so a release bump cannot leave the title page
// claiming a version the compiler no longer is.
#let version = toml("/Cargo.toml").package.version

#set document(title: "The qrab Manual", author: "qrab contributors")
#set page(
  paper: "a4",
  margin: (x: 2.3cm, top: 2.4cm, bottom: 2.2cm),
  numbering: "1",
  header: context {
    set text(size: 8.5pt, fill: luma(110))
    // A chapter that opens on this page names the page; otherwise the running
    // head carries the chapter the page continues.
    let opening = query(selector(heading.where(level: 1)).after(here()))
      .find(it => it.location().page() == here().page())
    let title = if opening != none {
      opening.body
    } else {
      let previous = query(selector(heading.where(level: 1)).before(here()))
      if previous.len() > 0 { previous.last().body }
    }
    grid(
      columns: (1fr, auto),
      align: (left, right),
      text(style: "italic")[The qrab Manual],
      title,
    )
    v(-0.55em)
    line(length: 100%, stroke: 0.4pt + luma(190))
  },
)

#set text(size: 10.5pt, font: ("Libertinus Serif", "New Computer Modern", "Liberation Serif"), lang: "en")
#set par(justify: true, leading: 0.62em, spacing: 1.05em)
#set heading(numbering: "1.1")
#set raw(syntaxes: "manual/qrab.sublime-syntax")

#show heading.where(level: 1): it => {
  pagebreak(weak: true)
  block(above: 0.6em, below: 0.9em)[
    #set text(size: 17pt, weight: 700)
    #if it.numbering != none [#counter(heading).display() #h(0.45em)]
    #it.body
  ]
}
#show heading.where(level: 2): it => block(above: 1.5em, below: 0.7em)[
  #set text(size: 12.5pt, weight: 700)
  #if it.numbering != none [#counter(heading).display() #h(0.4em)]
  #it.body
]
#show heading.where(level: 3): it => block(above: 1.2em, below: 0.5em)[
  #set text(size: 10.8pt, weight: 700)
  #it.body
]

#show raw.where(block: true): it => block(
  width: 100%,
  fill: luma(249),
  stroke: (left: 2pt + luma(205), rest: 0.4pt + luma(225)),
  inset: (x: 9pt, y: 7pt),
  radius: 2pt,
  breakable: true,
  text(size: 8.6pt, it),
)
#show raw.where(block: false): it => box(
  fill: luma(243),
  outset: (y: 2.5pt),
  inset: (x: 2pt),
  radius: 1.5pt,
  text(size: 9.2pt, it),
)
#show link: it => text(fill: rgb("#1F4E79"), it)
#show table.cell.where(y: 0): set text(weight: 700)
#set table(stroke: (x, y) => (bottom: 0.4pt + luma(200)), inset: (x: 6pt, y: 5pt))

// ============================================================================
// Helpers
// ============================================================================

// The source of a manual example, highlighted.
#let src(name) = raw(read("manual/examples/" + name + ".qrab").trim(), lang: "qrab", block: true)

// The diagram rendered from that example. Kept unbreakable so a picture never
// splits across a page, and centred because most circuits are wider than tall.
//
// The SVG's natural size is what a browser would show at 1x, which is smaller
// than the surrounding text wants; enlarging it up to the text width keeps a
// small circuit legible without letting a wide one run into the margin.
#let pic(name) = layout(bounds => {
  let file = "manual/images/" + name + ".svg"
  let natural = measure(image(file)).width
  block(breakable: false, width: 100%, above: 0.8em, below: 1.1em)[
    #align(center, image(file, width: calc.min(natural * 1.45, bounds.width)))
  ]
})

// An example: its source, then its picture. Kept together, because a snippet
// on one page and its diagram on the next is the one layout this manual must
// never produce.
#let ex(name) = block(breakable: false, width: 100%)[#src(name)#pic(name)]

#let note-box(title, body) = block(
  width: 100%,
  fill: rgb("#F4F7FB"),
  stroke: (left: 2pt + rgb("#1F4E79"), rest: none),
  inset: (x: 10pt, y: 8pt),
  above: 1.1em,
  below: 1.1em,
)[
  #text(weight: 700, size: 9.6pt)[#title] \
  #text(size: 9.8pt)[#body]
]

// ============================================================================
// Title page
// ============================================================================

#page(numbering: none, header: none)[
  #align(center)[
  #v(4.5cm)
  #text(size: 34pt, weight: 700)[The qrab Manual]
  #v(0.4cm)
  #text(size: 13pt, fill: luma(90))[
    A readable language for quantum circuit diagrams, \
    and the compiler that turns it into LaTeX, Typst, and SVG
  ]
  #v(1.2cm)
  #image("manual/images/worked-teleport.svg", width: 88%)
  #v(1.4cm)
  #text(size: 11pt)[Version #version]
  #v(0.2cm)
  #text(size: 10pt, fill: luma(110))[#link("https://github.com/inmzhang/qrab")[github.com/inmzhang/qrab] · #link("https://inmzhang.com/qrab/")[playground]]
  ]
]

#counter(page).update(1)

#outline(depth: 2, indent: 1.2em)

// ============================================================================

= Introduction

== What qrab is

`qrab` compiles a small language for describing quantum circuits into three
finished formats: standalone LaTeX/TikZ, standalone Typst using
#link("https://github.com/Mc-Zen/quill")[Quill], and SVG. One source file
produces all three, and the three agree because they are rendered from a single
checked model rather than from the text you wrote.

The language is a descendant of #link("https://github.com/qpic/qpic")[qpic] and
draws the same pictures, but a circuit is written as statements rather than as
positional commands, and the compiler checks what you wrote before it draws
anything.

#ex("tour-3")

Everything in that snippet is ordinary language: `qubit` declares two wires and
gives them an input and an output label, `h` and `x` are gates, `if` makes a
gate controlled, and `measure` switches a wire to a classical double rule.

== Design philosophy

*A circuit is a program, not a picture.* Positional macro languages make the
easy diagram quick and the interesting diagram unmaintainable. `qrab` gives you
the constructs you already reach for --- naming a thing, calling it twice,
importing it from another file --- and checks that you used them correctly.

*Say it once, get every format.* The parser produces one semantic model, and
each backend renders that model without ever seeing the source text.

*Portable styling or none at all.* Style options exist only where every backend
can honour them. Anything genuinely backend-specific goes in a `backend` block,
where it is visibly quarantined rather than silently ignored somewhere else.

*Errors are part of the language.* Every diagnostic knows where it came from,
across `import` boundaries, and says what to do about it.

== The three backends

#table(
  columns: (auto, 1fr, auto),
  table.header[Target][What it produces][Needs],
  [`latex`], [A standalone `.tex` document using TikZ, with absolute
    coordinates in centimetres. Alias: `tikz`.], [A TeX engine],
  [`typst`], [A standalone `.typ` document using Quill's grid, so the circuit
    participates in Typst's own layout. Alias: `quill`.], [Typst 0.15.1],
  [`svg`], [Finished SVG. No external toolchain at all, which is what the
    browser playground runs on.], [nothing],
)

The first two emit _source_, not pictures: hand the file to its own compiler.
Quill lays out its own grid, so the Typst output is the same circuit drawn by a
different engine rather than a copy of the other two; @backends has the
details.

// ============================================================================

= Installation

The distribution channels below are configured but stay pending until the first
public release.

#table(
  columns: (auto, 1fr),
  table.header[Channel][Command],
  [Cargo], [`cargo install qrab`],
  [cargo-binstall], [`cargo binstall qrab`],
  [npm / Bun], [`npm i -g @inmzhang/qrab` · `npx @inmzhang/qrab`],
  [Homebrew], [`brew install --formula .../qrab.rb`],
  [Shell], [`curl -LsSf .../qrab-installer.sh | sh`],
  [PowerShell], [`irm .../qrab-installer.ps1 | iex`],
)

Until then, install from a checkout:

```sh
cargo install --path . --locked
```

Nothing else is required to check a circuit, to generate LaTeX or Typst source,
or to render SVG. A TeX engine and Typst are needed only to turn the generated
sources into PDFs.

#note-box[No installation at all][
  The #link("https://inmzhang.com/qrab/")[playground] runs the compiler as
  WebAssembly in your browser. Nothing is uploaded and nothing is installed;
  it is the fastest way to try anything in this manual.
]

// ============================================================================

= The command line

`qrab` has two subcommands.

== Checking

```sh
qrab check circuit.qrab
```

`check` parses the file, resolves its imports, runs every semantic check, and
prints a one-line summary. It renders nothing, so it is the fastest way to
find out whether a circuit is well formed:

```
bell_pair: 2 wire(s), 6 operation(s)
```

== Compiling

```sh
qrab compile circuit.qrab                    # circuit.tex, circuit.typ, circuit.svg
qrab compile circuit.qrab --target svg       # circuit.svg
qrab compile circuit.qrab -t latex -o out.tex
qrab compile circuit.qrab -t typst -o -      # write to stdout
```

`--target` (`-t`) takes `latex` (alias `tikz`), `typst` (alias `quill`), `svg`,
or `all`, and defaults to `all`. `--output` (`-o`) names a single output file
and therefore requires a single backend; without it, each output is written
next to the input with the matching extension.

== From source to PDF

```sh
qrab compile circuit.qrab -t latex -o build/circuit.tex
tectonic build/circuit.tex --outdir build

qrab compile circuit.qrab -t typst -o build/circuit.typ
typst compile build/circuit.typ build/circuit.pdf
```

Both generated documents are standalone and crop to the circuit, so they drop
straight into a paper with `\includegraphics` or Typst's `image`. Typst
downloads Quill 0.8.0 on its first build.

== Diagnostics

Errors carry a line, a column, a labelled span, and, where there is something
useful to say, a suggestion. A run reports every error it can recover from
rather than stopping at the first:

```
Error:   × 2 errors found

Error:
  × unknown wire `q[3]`
   ╭─[circuit.qrab:4:5]
 3 │
 4 │   h q[3]
   ·     ─
 5 │   gate "U" on q[0] if q[0]
   ╰────
  help: declare it before use or enable `autowires`
```

The second error --- `q[0]` used as both a target and a control on line 5 ---
is reported in the same run. Recovery happens at statement boundaries, so one
mistake does not hide the rest of the file.

Spans survive `import`: an error inside an imported module is reported against
that module's own file and line, not against the position the text ended up at
after expansion.

// ============================================================================

= A guided tour

This chapter builds one circuit from nothing. Every intermediate step is a
complete, compilable file.

== Wires and a gate

A circuit is a named block. Inside it, declarations come first and operations
follow in the order they happen.

#ex("tour-1")

`qubit q[2]` declares a fixed-size array of two quantum wires, addressed `q[0]`
and `q[1]`. `h q[0]` puts a Hadamard on the first of them. The second wire is
drawn even though nothing acts on it, because it was declared.

== A controlled gate

Any gate becomes controlled by naming its controls after `if`.

#ex("tour-2")

`x q[1] if q[0]` is the familiar CNOT: `qrab` draws a control dot on `q[0]` and
the target notation on `q[1]`, because a controlled `X` has a conventional
rendering. Everything else is drawn as a labelled box.

== Labels and measurement

Wires can carry an input label, an output label, or both, and `measure` turns a
quantum wire into a classical one.

#ex("tour-3")

That is the whole Bell-pair circuit. Compile it with `qrab compile bell.qrab`
and you get `bell.tex`, `bell.typ`, and `bell.svg`, all describing this
picture.

// ============================================================================

= Wires

== Kinds

Four declaration keywords cover every row a diagram can have.

#table(
  columns: (auto, 1fr),
  table.header[Keyword][Draws],
  [`qubit`], [A single quantum line.],
  [`bit`], [A classical double line.],
  [`hidden`], [Nothing, until something makes the wire visible. Useful for a
    row that only exists for part of the diagram.],
  [`ellipsis`], [One wireless row labelled `...` at both ends, which makes an
    omitted register range explicit without inventing a wire name.],
)

#ex("wire-kinds")

== Labels

A declaration may carry an input label after `:` and an output label after
`->`. Either may be omitted.

```qrab
qubit message: "|psi>" -> "|psi>"
qubit work: "|0>"
qubit result -> "m"
qubit plain
```

Labels are plain text in every backend. They are not LaTeX maths: write
`"|psi>"` rather than `"$\ket{\psi}$"`, and the same string appears in the SVG,
the TikZ, and the Typst output. If you need real mathematics in a paper, reach
for a `backend latex` block (@escapes).

== Arrays and ranges

A declaration may be an array of fixed size, and a range selects several of its
wires at once. Ranges are end-exclusive, so `q[1..4]` is `q[1]`, `q[2]`, and
`q[3]`.

#ex("wire-arrays")

Ranges work anywhere a wire list is accepted: targets, controls, annotation
spans, and function arguments. A statement that takes exactly one wire, such as
`h`, rejects a range rather than silently picking the first.

An array declaration shares one input and output label across all its wires,
which is why the picture above repeats `|0>` and `out` on every row.

== Declaring wires implicitly

Strict declaration is the default: an undeclared name is an error, which is
what catches a typo. For a quick sketch, `autowires` opts the rest of the
circuit into creating an unknown quantum wire on first use, labelled with its
own name.

#ex("wire-autowires")

Use it for a sketch and turn it off for anything you intend to keep; with
`autowires` on, a misspelled wire quietly becomes a new one.

// ============================================================================

= Operations

== Built-in gates

`h`, `x`, `y`, `z`, `s`, and `t` take one wire each and draw a box with the
uppercase letter in it.

#ex("gates-builtin")

The `parallel` block in that example is explained in @scheduling; without it,
the six gates would pack into the earliest free column each and produce the
same picture here anyway.

== Controls

Controls follow `if`, separated by commas. A control prefixed with `!` is an
open (negative) control and is drawn as a hollow dot.

#ex("gates-controls")

A controlled `X` with no styling and no link is drawn with the conventional
target circle, and a controlled `Z` with the conventional dot; every other gate
is drawn as a labelled box joined to its controls by a vertical line. A wire
cannot be both a target and a control of the same operation.

== Named boxes

`gate "label" on wires` draws an arbitrary box. Given several targets it spans
them, and it may take controls like any other gate.

#ex("gates-box")

A box grows to fit its label rather than clipping it, and the column grows with
it, so a long name spreads the diagram out instead of overlapping its
neighbour. Set `width` to ask for more room than the label needs.

== Custom target operators

The shape and the label are yours, so any operator notation is available
without new syntax.

#ex("gates-operator")

== Phase gates

`phase` draws its label verbatim in a circle, exactly as qpic does.

#ex("gates-phase")

The conventional label is the index #box[$k$] of #box[$R_k$], not the angle, which is why
the QFT examples in this manual read `phase "2"` rather than `phase "pi/2"`.
Unlike qpic the circle grows to fit, so a long label widens the gate instead of
being cut off; see @growth for what that costs.

== Measurement

An unlabelled `measure` draws a meter. `measure q as "Z"` draws a D-shaped
result marker, and `using tag` switches it to a pointed tag. All three switch
each target to a classical wire.

#ex("gates-measure")

== Swap and barrier

`swap` connects two wires with the crossed notation. `barrier` draws a dashed
line across its selected wires; with no selection it covers every currently
active wire.

#ex("gates-swap-barrier")

// ============================================================================

= Layout and scheduling <scheduling>

== How columns are chosen

You never write a column number. The compiler packs each operation into the
earliest column that is free across every wire it occupies, while preserving
source order on each wire.

#ex("schedule-packing")

The three Hadamards are independent, so they share the first column. The CNOT
occupies rows 0 through 3, so it has to wait for the column after the
Hadamard on `q[0]`. The second Hadamard on `q[1]` waits for the CNOT, because
the CNOT's span crosses `q[1]` even though it does not act on it.

== `parallel`

A `parallel` block aligns its operations after all prior work and aligns
everything after the block behind it, which is how you draw a layer.

#ex("schedule-parallel")

Operations whose visual spans overlap are still serialised inside the block, so
`parallel` cannot produce an overlapping picture.

== `overlay`

When two operations genuinely belong in one column even though their spans
collide, say so explicitly.

#ex("schedule-overlay")

Two gates still cannot share a wire, and lifecycle changes and permutations are
rejected inside an overlay.

== `touch`

`touch` consumes no column of its own; it aligns everything after it with the
deepest column reached so far. Give it a `stroke` to draw the alignment as a
visible slice.

#ex("schedule-touch")

== `space`

`space` reserves invisible room. A `width` wider than the layout's `column_gap`
widens that column and shifts everything after it; a narrower one changes
nothing, because the abstract gap is a floor.

#ex("schedule-space")

== Geometry

`layout` sets the shared geometry every backend reads. Sizes are in points;
gaps are abstract units that each backend maps to its own grid.

#table(
  columns: (auto, auto, 1fr),
  table.header[Property][Default][Meaning],
  [`orientation`], [`horizontal`], [`horizontal` or `vertical`.],
  [`scale`], [`1`], [Overall scale factor.],
  [`column_gap`], [`1.5`], [Distance between columns.],
  [`wire_gap`], [`1`], [Distance between wires.],
  [`gate_size`], [`20`], [Default box size, in points.],
  [`corner_radius`], [`4`], [Corner rounding of permutation curves, in points.],
  [`comment_width`], [`144`], [Wrapping width of `note` text, in points.],
  [`background`], [`white`], [Background colour of the whole diagram.],
)

#ex("style-layout")

== Vertical circuits

A vertical circuit reads top to bottom, as qpic's does: the input labels sit
above the first column, time runs down the page, and the wires run left to
right in declaration order.

#ex("style-vertical")

// ============================================================================

= Wire lifecycle

Wires do not have to exist for the whole diagram, they do not have to keep the
same kind, and they do not have to stay in one row.

== Starting and ending

`start` defers a wire until the column it appears in; `end` stops it. Either
may carry an `as "label"`, which is drawn at the tick.

#ex("life-start-end")

An omitted wire list on `start` selects every wire that has not started yet; on
`end` it selects every active wire.

== Changing kind

`set wires to quantum | classical | hidden` changes what the line draws from
that column on. A transition to `hidden` or `quantum` may add `as "value"` to
draw a known-value exit or entry marker.

#ex("life-set-kind")

== Bundles

`bundle` draws the multiplicity slash used for a register drawn as one line.

#ex("life-bundle")

== Permutations

`permute` lists the selected wires in their new visual order. Every later
operation, and every output label, follows the new order.

#ex("life-permute")

The `width` of a permutation controls how much horizontal room the crossing
curves get, and `layout.corner_radius` controls how sharply they turn.

// ============================================================================

= Annotations

Annotations describe the circuit without becoming part of it. Most of them
share a column with the operation they annotate rather than consuming one.

== `label` and `labels`

`label` centres one piece of text across a wire span; `labels` puts text on
each wire of a selection, taking either one repeated label or one per wire.

#ex("ann-labels")

Label text is drawn on the diagram background, so the wire breaks around it
rather than striking through it. That is what qpic does too. A `gate` with
`shape: none` takes no fill, also as in qpic, so its wire does run behind the
text.

== Braces

`brace left | right | both` draws a curly brace beside or around a label
spanning the selected wires.

#ex("ann-brace")

== `equals`

`equals` centres `=` across the wires, which is how a decomposition is written:
the composite gate, then `=`, then what it expands to. It accepts a label of
its own, an `on` selection, and `braced left | right | both`.

#ex("ann-equals")

== Notes

`note above | below` attaches free text to the preceding slice without
consuming a column of its own. The text wraps at `layout.comment_width`.

#ex("ann-note")

== Cuts

A `cut` is a stage separator and does occupy its own column.

#ex("ann-cut")

== Marks and groups

A `mark` is a named position in the operation stream; it is not an operation
and draws nothing. A `group` highlights everything between two marks.

#ex("ann-group")

The end mark is exclusive, and `here` means every operation parsed so far.
Groups may overlap or nest, and are drawn behind the circuit.

// ============================================================================

= Programming the source

Everything in this chapter happens before a picture exists. `let`, `style`,
`fn`, and `import` are resolved into the same typed operation tree that a
literal circuit produces --- none of them performs textual substitution, so a
parameter name that also appears inside a gate label stays ordinary label text.

== `let`

`let` names a label or a colour.

== `style`

`style` names a set of checked style fields, which any operation can then apply
with `with`. A use may add comma-separated overrides after the style name.

#ex("prog-let-style")

== Functions

`fn` names a sequence of operations over wire parameters. Functions may call
functions defined before them, arity is checked at the call site, and two
parameters cannot be bound to the same wire.

#ex("prog-fn")

Declarations, captures, forward calls, and recursion are deliberately not part
of the current function subset. A function is a reusable block of operations,
not a general procedure.

== `repeat` and `reverse`

`repeat count { ... }` repeats a parsed block, including function calls.
`reverse { ... }` emits its operations in reverse source order, and
`reverse from start_mark to end_mark` replays an earlier marked range
backwards.

#ex("prog-repeat-reverse")

#note-box[What `reverse` does and does not do][
  `reverse` reverses the order of statements. It does not invert the
  mathematical action of each gate: an `s` stays an `s`, it does not become
  its adjoint. For a self-inverse block --- most encoders, most syndrome
  circuits --- that is exactly the uncomputation you want, and for anything
  else you should spell out the adjoint gates.
]

== Imports

A module holds declarations --- `import`, `let`, `style`, and `fn` --- and the
root file owns the single `circuit`. Paths are relative to the importing file,
duplicate imports load once, and cycles are rejected.

```qrab
// modules/prep.qrab
style prepared {
  fill: yellow
}

fn make_bell(control, target) {
  h control with prepared
  x target if control
}
```

#ex("prog-import")

// ============================================================================

= Styling

Every operation accepts a trailing `with` clause. The grammar is shared so a
named `style` can be reused anywhere, but each visual consumes only the fields
that mean something for it; the rest are accepted and ignored.

== Fields

#table(
  columns: (auto, auto, 1fr),
  table.header[Field][Value][Meaning],
  [`stroke`], [colour], [Outline and text colour.],
  [`fill`], [colour], [Interior colour.],
  [`width`], [points], [Extent along the time axis.],
  [`height`], [points], [Extent across the wires.],
  [`size`], [points], [Sets `width` and `height` at once.],
  [`shape`], [name], [`box`, `circle`, `ellipse`, or `none`.],
  [`dash`], [name], [`solid` or `dashed`.],
  [`opacity`], [0 to 1], [Transparency of the shape.],
  [`link`], [URL], [Wraps the visible text in a hyperlink.],
)

== Colours

Twelve named colours are portable across all three backends: `black`, `white`,
`gray`, `red`, `green`, `blue`, `teal`, `purple`, `orange`, `yellow`, `olive`,
and `lime`. Anything else is written as a quoted six-digit hex value such as
`"#336699"`. A name that is not on the list is an error rather than a silent
fallback, because the point of the list is that every backend draws the same
colour.

#ex("style-colors")

== Shapes

#ex("style-shapes")

`shape: none` draws the label with no outline and no fill, which is qpic's
behaviour for a shapeless operator, and which is why the wire is visible
through the text.

== Where each field applies

#table(
  columns: (auto, 1fr),
  table.header[Visual][Honours],
  [Gates, measurements, value markers], [the full set],
  [Wire declarations], [`stroke`, `dash`, `opacity`],
  [`space`], [`width`, `height`],
  [`barrier`, `cut`], [`stroke`, `opacity` (always dashed)],
  [`touch`], [`stroke`, `dash`, `opacity`],
  [`swap`, `permute`], [`stroke`, `dash`, `opacity`, and an applicable `width`],
  [Labels, endpoints], [`stroke`, `fill`, `opacity`, `link`],
)

In particular, `width` or `link` on a wire declaration, `fill` or
`dash: solid` on a barrier or a cut, and `shape` on a label or a `touch` have
no effect.

== Links

A link must be an absolute `http`, `https`, or `mailto` URL, and is checked.
It wraps the visible text of a gate or a label in every backend that can carry
one.

#ex("style-link")

#note-box[One backend-specific style][
  `bundle` styling is not fully portable: the LaTeX backend applies `stroke`,
  `dash`, and `opacity` to the slash, while Quill 0.8 exposes no style
  parameters for its `nwire`, so the Typst output uses Quill's default slash
  and label appearance.
]

// ============================================================================

= Backend escapes <escapes>

Backend-only code is deliberately isolated. A `backend` block names one target
and carries verbatim strings that are emitted only for it.

#src("backend-escape")

Each block may repeat `preamble`, `before`, or `after`. `preamble` lands in the
document preamble, `before` just inside the picture, and `after` just before it
closes. The SVG backend has no escape block and ignores both of these, so the
picture below is what every backend draws with the hooks removed:

#pic("backend-escape")

Escapes are the explicit replacement for qpic's preamble and TikZ hooks. Reach
for one when you need real LaTeX mathematics in a label, a package only one
document needs, or an effect the common model cannot express --- and not
otherwise, because anything you put here exists in exactly one output.

// ============================================================================

= How the picture is built <backends>

== Coordinates

The LaTeX and SVG backends place everything at absolute coordinates: wires are
`wire_gap` apart, columns are `column_gap` apart, and the compiler computes
both. The Typst backend instead hands the circuit to Quill, which lays out its
own grid from the contents of each cell. This is why the Typst output is not
pixel-identical to the other two: it is the same circuit, drawn by a different
engine, and Quill's grid will size a column to its contents where the
coordinate backends use the abstract gap.

Anything that needs more room than the column gap widens its own column and
shifts everything after it --- a `space`, a gate with an explicit `width`, a
box whose label outgrew the default size, or a lifecycle label that hangs into
the gap. The result is that a long label spreads a diagram out rather than
overlapping its neighbour.

== Growing, not clipping <growth>

Shapes grow to fit their labels. qpic clips instead, which keeps the layout
exact and loses the end of the label; `qrab` keeps the label and moves the
layout.

Along the time axis this is free, because the column grows too. Across the
wires there is nowhere to grow into: a circle or an ellipse whose label is
wider than `layout.wire_gap` will reach into the row above and below. When that
happens, give the gate an explicit `size` or widen `wire_gap`.

== Vertical circuits

A vertical circuit is laid out horizontally and then turned a quarter turn, so
that time runs down the page and the first-declared wire stays on the left.
Text is turned back the other way and stays upright. Wire labels are drawn
horizontally, so in a vertical circuit with long labels they can crowd each
other; shorten them or widen `wire_gap`.

// ============================================================================

= Worked examples

== Quantum Fourier transform

Nothing here is new: `phase` with a control, and `swap` for the bit reversal at
the end. The label of each phase gate is the index of #box[$R_k$], as in qpic.

#ex("worked-qft")

== A decomposition

`equals` is what makes a decomposition figure read as one. The composite gate
is drawn on the left, then `=`, then the circuit it expands to, all in a single
diagram.

#ex("worked-adder")

== Teleportation, annotated

Groups highlight a marked range, a `cut` separates the classical channel from
the quantum part, and a `note` explains it. Notice that the groups are declared
at the end, after the marks they refer to exist, but are drawn behind
everything.

#ex("worked-teleport")

== A sketch with omitted registers

`ellipsis` stands in for the rows a real circuit would have, `bundle` says how
many wires each drawn line really carries, and `brace` picks out the register
the algorithm reads.

#ex("worked-shor")

== Syndrome extraction

A `hidden`-to-`quantum` transition brings the ancillas in with a known value,
`parallel` draws the layers, and a `group` marks one round.

#ex("worked-syndrome")

// ============================================================================

= Library API

The crate exposes the same pipeline the binary uses.

```rust
use qrab::{Target, compile, load_source, parse, render};

// Shortest path, for source already in memory.
let svg = compile("circuit c { qubit q\n h q }", Target::Svg)?;

// With relative imports resolved from a path on disk.
let source = load_source("circuit.qrab")?;
let circuit = parse(source.as_str())?;
let latex = render(&circuit, Target::Latex);
let typst = render(&circuit, Target::Typst);
```

`compile` is `parse` followed by `render`. `load_source` expands `import`
statements and remembers where each expanded line came from, which is what lets
a diagnostic point into the right module: `LoadedSource::origin` maps a line of
the expanded source back to its file and line.

`parse` returns `Result<Circuit, Diagnostic>`, and `Diagnostic` implements
`std::error::Error` as well as `miette::Diagnostic`, so it prints with source
context when a `miette` reporter is installed.

Public AST structs are `#[non_exhaustive]` and should be obtained through
`parse`, with public fields available for adjustments afterwards. Enums stay
exhaustive so that additions to the model break downstream matches at compile
time; before 1.0, minor releases may make such changes.

// ============================================================================

= Relationship to qpic

`qrab` draws qpic's visual model. All 44 of qpic's golden circuits and its 64
documented examples are translated into the test corpus, and every one of them
is compiled through both TeX and Typst on every run.

The differences are deliberate:

#table(
  columns: (auto, 1fr),
  table.header[qpic][qrab],
  [Positional commands, wire names in the first columns],
    [Statements with keywords, wires named in declarations],
  [Token substitution macros], [`let`, `style`, and `fn`, resolved into a typed
    tree],
  [`\input` of another file], [`import`, path-aware and cycle-checked],
  [Labels are LaTeX], [Labels are plain text in every backend; LaTeX goes in a
    `backend` block],
  [Uppercase directives (`VERTICAL`, `SCALE`, ...)], [A `layout` block],
  [Shapes clip their labels], [Shapes grow to fit them (@growth)],
  [One output: TikZ], [Three outputs from one model],
)

Parity and its evidence are tracked in `docs/qpic-coverage.md`.

// ============================================================================

#pagebreak()

= Appendix: statement index

Newlines terminate statements, `;` separates several on one line, and `//`
starts a comment. An omitted wire list selects every active wire on `end`,
`barrier`, `label`, `labels`, `equals`, `brace`, `note`, `cut`, `space`, and
`touch`, and every _inactive_ wire on `start`; a statement is rejected when that
default selection turns out to be empty.

// Tightened so the whole index stays on one openable spread.
#[
#set table(inset: (x: 6pt, y: 3.4pt))
#table(
  columns: (auto, 1fr),
  table.header[Statement][Summary],

  [`import "path"`], [Load a declaration-only module, relative to this file.],
  [`let name = value`], [Name a label or a colour.],
  [`style name { ... }`], [Name a set of style fields.],
  [`fn name(a, b) { ... }`], [Name a sequence of operations over wire parameters.],
  [`circuit name { ... }`], [The circuit. One per root file.],

  [`layout { ... }`], [Shared geometry.],
  [`backend latex|typst { ... }`], [Verbatim code for one target only.],

  [`qubit`, `bit`, `hidden`], [Declare a wire or a fixed-size array.],
  [`ellipsis name`], [Declare a wireless `...` row.],
  [`autowires`], [Create unknown quantum wires on first use.],

  [`h x y z s t`], [Built-in single-wire gates.],
  [`gate "L" on ...`], [An arbitrary box.],
  [`phase "L" on ...`], [A labelled circle.],
  [`measure ... [as "L"] [using tag]`], [Meter, D marker, or tag marker.],
  [`swap a, b`], [Swap notation.],
  [`barrier [wires]`], [Dashed barrier.],

  [`set ... to quantum|classical|hidden [as "v"]`], [Change wire kind.],
  [`start ... [as "L"]`], [Begin a deferred wire.],
  [`end ... [as "L"]`], [Stop a wire.],
  [`bundle "n" on wire`], [Multiplicity slash.],
  [`permute ...`], [Reorder rows from here on.],
  [`space ... with width: n`], [Reserve invisible room.],
  [`touch [wires]`], [Align later work; optionally draw a slice.],

  [`label "L" on ...`], [Text centred across a span.],
  [`labels "A", "B" on ...`], [Text on each selected wire.],
  [`brace left|right|both "L" on ...`], [Brace around a span.],
  [`equals ["L"] [on ...] [braced ...]`], [Decomposition separator.],
  [`note above|below "L" [on ...]`], [Wrapped free text beside a slice.],
  [`cut [on ...] [as "L"]`], [Stage separator in its own column.],
  [`mark name`], [A named position in the operation stream.],
  [`group "L" from m to m|here on ...`], [Highlight a marked range.],

  [`repeat n { ... }`], [Repeat a block.],
  [`reverse { ... }`], [Emit a block backwards.],
  [`reverse from m to m|here`], [Replay a marked range backwards.],
  [`parallel { ... }`], [Align a layer.],
  [`overlay { ... }`], [Force one column.],
)
]
