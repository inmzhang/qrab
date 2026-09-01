# Delivery phases

- [x] Phase 0 — pin the qpic and Quill reference versions; establish formatting, linting, tests, artifact builds, hooks, CI, CD, and development recipes.
- [x] Phase 1 — parse the core `.qrab` language into one semantic model and render wires, common gates, arbitrary boxes, controls, measurement, swaps, barriers, and labels to both backends.
- [x] Phase 2 — add the full qpic visual model: styles, shapes, wire changes, annotations, braces, cuts, permutation, sizing, orientation, regions, and repeat/reverse.
- [x] Phase 3 — add programming-language composition: functions with wire parameters, arrays/ranges, loops, explicit parallel blocks, imports, and backend escape blocks.
- [x] Phase 4 — port all 44 qpic golden tests and 64 documented examples to `.qrab`; compile every fixture through Tectonic and Typst/Quill and add tolerant page-geometry regression baselines. All 108 qpic translations, their 216 PDF builds, and 14 representative geometry baselines are complete.
- [x] Phase 5 — finish source-aware diagnostics and reference documentation, verify source/install/binary packages, and exercise the checksummed tag release workflow. Publishing the first tag remains an explicit maintainer action.

Each phase ends in a passing `just ci` and its own commit. A checked phase means the implementation and its executable verification are both present.
