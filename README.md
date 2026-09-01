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

## Install and use

Build and install the dependency-free Rust binary:

```sh
cargo install --path . --locked
qrab check examples/teleportation.qrab
qrab compile examples/teleportation.qrab
```

`compile` writes standalone `.tex` and `.typ` sources by default. Select one backend with `--target latex|typst` and `-o <path>`:

```sh
qrab compile examples/teleportation.qrab
tectonic examples/teleportation.tex --outdir target
typst compile examples/teleportation.typ target/teleportation-typst.pdf
```

Tectonic is only needed to turn the generated TikZ source into a PDF. Typst 0.15.1 downloads Quill 0.8.0 on its first build. Neither renderer is required for `qrab check` or source generation.

## Library API

`qrab::compile` is the shortest in-memory API; `load_source` expands file imports, while `parse` and `render` expose the checked AST pipeline separately. Public AST structs are non-exhaustive and should be obtained through `parse`, with public fields available for adjustments. Enums remain exhaustive so model additions break downstream matches at compile time; before 1.0, minor releases may make such changes.

The language supports typed values and styles, functions and relative imports, checked wire arrays/ranges, repetition and marked reverse replay, safe parallel and explicit overlay blocks, lifecycle changes, persistent permutation, annotations, hyperlinks, and target-isolated raw hooks. Declaration-only modules use `import "relative/path.qrab"`; imports resolve relative to each source file, load once, and reject cycles. See [the import example](examples/imports.qrab).

## Development

Install stable Rust, Tectonic, Typst 0.15.1, and `just`, then run:

```sh
just install-hooks
just ci
```

The artifact suite requires all 44 translated qpic golden tests and all 64 documented examples, then compiles those plus 11 focused qrab fixtures through both Tectonic and Typst/Quill (238 PDFs total). Fourteen tolerant page-geometry baselines guard representative dense, vertical, annotated, overlaid, imported, lifecycle, and colored-background layouts without brittle pixel snapshots.

The frontend reference is in [docs/language.md](docs/language.md). The phased parity plan and its evidence are tracked in [docs/qpic-coverage.md](docs/qpic-coverage.md) and [docs/roadmap.md](docs/roadmap.md); maintainers can follow [docs/releasing.md](docs/releasing.md) for a verified tag release.

See [examples/styling.qrab](examples/styling.qrab) for portable styling, [examples/lifecycle.qrab](examples/lifecycle.qrab) for persistent permutations, [examples/programming.qrab](examples/programming.qrab) for structured code, [examples/measurements.qrab](examples/measurements.qrab) for measurement shapes, [examples/ellipsis.qrab](examples/ellipsis.qrab) for omitted register rows, [examples/escapes.qrab](examples/escapes.qrab) for isolated backend hooks, and [examples/regions.qrab](examples/regions.qrab) for annotations and marked regions.

## License

MIT. See [LICENSE](LICENSE).
