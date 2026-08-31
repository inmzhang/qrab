# qrab

`qrab` compiles a small, readable quantum-circuit language to standalone LaTeX/TikZ or Typst using [Quill](https://github.com/Mc-Zen/quill). It is inspired by [qpic](https://github.com/qpic/qpic), but uses programming-language statements instead of positional commands and token-substitution macros.

```qrab
circuit bell {
  qubit q[2]: "|0>" -> "bell"

  h q[0]
  x q[1] if q[0]
  measure q[0], q[1]
}
```

Compile both targets:

```sh
cargo run -- compile examples/teleportation.qrab
tectonic examples/teleportation.tex --outdir target
typst compile examples/teleportation.typ target/teleportation-typst.pdf
```

Development uses `just`:

```sh
just install-hooks
just check
just test-artifacts
```

The frontend reference is in [docs/language.md](docs/language.md). The phased parity plan and its evidence are tracked in [docs/qpic-coverage.md](docs/qpic-coverage.md) and [docs/roadmap.md](docs/roadmap.md).

See [examples/styling.qrab](examples/styling.qrab) for portable styling, [examples/lifecycle.qrab](examples/lifecycle.qrab) for persistent permutations, [examples/programming.qrab](examples/programming.qrab) for structured code, and [examples/annotations.qrab](examples/annotations.qrab) for labels, braces, notes, and cuts.
