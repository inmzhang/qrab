# Changelog

All notable changes are documented here. This project follows Semantic Versioning.

## [Unreleased]

- Fixed the direction of vertical circuits, which ran bottom to top. qpic reads a vertical circuit downward, and so does every convention it follows; all three backends now put the input labels at the top and the first column beneath them. Vertical rows are dealt out bottom-up so the first wire stays on the left.
- Fixed vertical gate boxes in the LaTeX backend, which came out a quarter turn from their wires because TikZ leaves node shapes axis-aligned under a canvas `rotate`, and the labels in the SVG backend, which were left lying on their side.
- Fixed braces, whose fixed offset put the label on top of the central tip that tells a brace apart from a parenthesis. The two sides now clear the label they enclose.
- Fixed `label` and per-wire labels, which had their wire struck through the text. qpic fills every label with the background colour, and now so does qrab; a gate with `shape: none` still takes no fill, which is also what qpic does.
- Fixed column spacing, which reserved room only for `space`. Any gate wider than the column gap, by request or because its label made it so, now widens its own column instead of overlapping its neighbours, and a lifecycle label reserves the room it hangs into.
- Fixed labelled measurement markers, which drew a fixed-size shape and let a long label run outside it.
- Fixed multi-wire gates, whose requested `height` could shrink the box until it no longer reached the wires it acts on. As in qpic, the request is now a floor.
- Fixed the `phase` gate label, which was rendered as `P(<label>)`. qpic draws a phase label verbatim inside its circle, and the three extra characters inflated a shape whose diameter has to span its own label. The ported QFT circuits now carry qpic's own R_k indices, and `QFT3v1`/`QFT4vert` no longer invert the gate's target and control.
- Fixed `space` width, which widened its column in the Typst output but was a no-op in the LaTeX one and, by inheritance, the SVG one. All three backends now reserve the requested room.
- Fixed the unlabelled measurement meter in the LaTeX backend, which drew its dial arc above centre with the needle pointing down. It now matches Quill and standard notation, and the SVG backend gained the arrowhead the LaTeX one already had.
- Added a browser playground built on the SVG backend and WebAssembly, offering the guided examples and all 44 ported qpic circuits, deployed to GitHub Pages behind the `ENABLE_PLAYGROUND` repository variable.
- Added an SVG backend (`qrab compile --target svg`) that needs no external toolchain. `Target` gained an `Svg` variant, which is a breaking change for exhaustive downstream matches.
- Adopted Logos, Clap, Miette, and supporting test/build tooling; the measured stripped release binary grew from 756,952 B to 1,570,280 B (+107%).
- Added release-plz versioning and cargo-dist delivery through crates.io, cargo-binstall, npm/Bun, Homebrew, shell, and PowerShell across six binary targets.

## [0.1.0] - 2026-09-01

- Introduced the typed `.qrab` language and standalone TikZ and Typst/Quill backends.
- Added functions, imports, arrays, structured repetition/replay, parallel/overlay layout, lifecycle changes, annotations, styles, and isolated backend hooks.
- Ported all 44 qpic golden circuits and 64 manual examples into a 238-PDF artifact gate.
