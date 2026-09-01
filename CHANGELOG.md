# Changelog

All notable changes are documented here. This project follows Semantic Versioning.

## [0.1.1] - 2026-09-01

### Documentation

- Read the manual's version from the manifest

- Rebuild the manual, not just the assets, on a release



## [0.1.0] - 2026-09-01

- Introduced the typed `.qrab` language: wires and wire arrays, gates with controls, measurement, packing and parallel/overlay layout, wire lifecycle changes, annotations, styles, functions, imports, structured repetition and replay, and isolated per-backend escape hooks.
- Added three standalone backends: TikZ (`--target latex`), Typst/Quill (`--target typst`), and SVG (`--target svg`), which needs no external toolchain.
- Matched qpic's geometry across the ported corpus: vertical circuits read downward with the first wire on the left, labels mask the wire behind them, columns grow to fit whatever they hold, braces clear their own label, `space` reserves room in every backend, and the phase, measurement, and multi-wire gate shapes follow qpic's own rules.
- Ported all 44 qpic golden circuits and 64 manual examples into a 238-PDF artifact gate.
- Added a browser playground built on the SVG backend and WebAssembly, deployed to GitHub Pages behind the `ENABLE_PLAYGROUND` repository variable.
- Added a typeset manual, `docs/manual.pdf`, and shell completions and man pages in `assets/`.
