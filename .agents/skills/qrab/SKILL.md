---
name: qrab
description: Author, validate, and compile `.qrab` quantum-circuit descriptions to LaTeX/TikZ, Typst/Quill, SVG, or Quirk. Use when creating or editing qrab circuits, choosing an export backend, or diagnosing qrab compiler errors. Do not use for executing circuits on quantum hardware or translating them to SDKs such as Qiskit or Cirq.
---

# qrab

Use qrab to describe and render quantum-circuit diagrams. It does not submit jobs to quantum hardware, and its Quirk output is a simulator URL with a deliberately limited semantic mapping.

## Ground work in the installed version

- Reuse nearby `.qrab` files and project conventions before writing new source.
- In a qrab checkout, consult `examples/` for patterns and `docs/manual.typ` for exact syntax. Otherwise inspect `qrab --help` and `qrab compile --help` rather than guessing an option.
- Use `qrab ...` when the binary is installed. In the qrab source checkout, use `cargo run -- ...` when it is not; do not install tools unless the user asks.

## Describe the circuit

A root file has one `circuit` block. Declare wires before operations and preserve the algorithm's operation order:

```qrab
circuit bell_pair {
  qubit q[2]: "|0>" -> "bell"

  h q[0]
  x q[1] if q[0]
  measure q[0], q[1]
}
```

Use the language's existing constructs instead of encoding layout manually:

- Declare `qubit`, `bit`, or `hidden` wires; arrays are fixed-size and ranges are end-exclusive (`q[1..4]` selects indices 1, 2, and 3).
- Use `h`, `x`, `y`, `z`, `s`, and `t` for built-ins; `gate "U" on ...` for named boxes; `phase`, `measure`, `swap`, and `barrier` for their standard notation.
- Add controls with `if`; prefix a control with `!` for an open control.
- Use `parity(a, b)` for odd Z-basis parity, or `parity_x(...)`,
  `parity_y(...)`, and `parity_z(...)` to select the basis explicitly.
- Use `parallel` for an aligned layer. Let qrab pack ordinary independent operations itself; use `overlay` only for deliberate same-column overlap.
- Use `fn`, `style`, `let`, and relative `import` for repeated maintained source. Imported modules contain declarations; the root file owns the circuit.
- Keep strict wire declarations for maintained files. Use `autowires` only when the user explicitly wants a disposable sketch because typos otherwise become wires.
- In every visible string, keep ordinary text literal and delimit mathematics with `$...$` LaTeX; write `\$` inside a quoted qrab string for a literal dollar. LaTeX emits math directly, Typst uses MiTeX, SVG renders paths, and Quirk strips the delimiters because it cannot typeset math.
- Prefer portable `with` styling; use `backend latex` or `backend typst` escapes only when the common model cannot express the requested result. There is no SVG escape.

## Check, compile, and verify

Run the checker before producing outputs:

```sh
qrab check circuit.qrab
```

Then compile only the requested target, or use the default `all` target when the user really wants every representation:

```sh
qrab compile circuit.qrab -t svg
qrab compile circuit.qrab -t latex -o circuit.tex
qrab compile circuit.qrab -t typst -o circuit.typ
qrab compile circuit.qrab -t quirk -o circuit.url
qrab compile circuit.qrab                 # latex, typst, svg, and quirk
```

Without `-o`, qrab writes beside the input as `.tex`, `.typ`, `.svg`, or `.url`. `--target all` cannot be combined with `--output`.

Choose the backend by the requested deliverable:

- `svg`: finished, dependency-free image; prefer it for web use and quick visual inspection.
- `latex` / `tikz`: standalone TikZ `.tex`; run a TeX engine such as `tectonic` only when a PDF is requested.
- `typst` / `quill`: standalone Quill-based `.typ`; run `typst compile` only when a PDF is requested.
- `quirk`: `.url` containing an interactive Quirk link. It supports at most 16 qubits, omits drawing-only features, and turns arbitrary named boxes into labeled no-op gates; never present those boxes as simulated unitaries.

After semantic or layout changes, re-run `check` and render SVG for visual inspection when an image viewer is available. Also compile the requested document backend when its engine-specific layout matters.
