# Changelog

All notable changes are documented here. This project follows Semantic Versioning.

## [Unreleased]

- Fixed `space` width, which widened its column in the Typst output but was a no-op in the LaTeX one and, by inheritance, the SVG one. All three backends now reserve the requested room.
- Fixed the unlabelled measurement meter in the LaTeX backend, which drew its dial arc above centre with the needle pointing down. It now matches Quill and standard notation, and the SVG backend gained the arrowhead the LaTeX one already had.
- Added a browser playground built on the SVG backend and WebAssembly, deployed to GitHub Pages behind the `ENABLE_PLAYGROUND` repository variable.
- Added an SVG backend (`qrab compile --target svg`) that needs no external toolchain. `Target` gained an `Svg` variant, which is a breaking change for exhaustive downstream matches.
- Adopted Logos, Clap, Miette, and supporting test/build tooling; the measured stripped release binary grew from 756,952 B to 1,570,280 B (+107%).
- Added release-plz versioning and cargo-dist delivery through crates.io, cargo-binstall, npm/Bun, Homebrew, shell, and PowerShell across six binary targets.

## [0.1.0] - 2026-09-01

- Introduced the typed `.qrab` language and standalone TikZ and Typst/Quill backends.
- Added functions, imports, arrays, structured repetition/replay, parallel/overlay layout, lifecycle changes, annotations, styles, and isolated backend hooks.
- Ported all 44 qpic golden circuits and 64 manual examples into a 238-PDF artifact gate.
