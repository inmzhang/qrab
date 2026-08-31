# qpic parity ledger

Reference: qpic commit `a1f052c4a8d995b80605a6aef06243a4242494f3` (44 golden test circuits and 64 manual examples). Typst output targets Quill `0.8.0`.

Legend: **done** is implemented in both backends and exercised by a runnable test; **partial** has a portable implementation but still lacks a qpic behavior; **next** is designed but not yet complete.

| qpic capability | `.qrab` model | Status |
| --- | --- | --- |
| Declared quantum, classical, and off wires | `qubit`, `bit`, `hidden` | done |
| Wire arrays and input/output labels | `qubit q[8]: "in" -> "out"` | done |
| H, X/N/CNOT, Y, Z, S, T | lowercase gate statements | done |
| Positive and negative controls | `x target if control, !open` | done |
| Arbitrary one/multi-wire G/P gates | `gate "name" on targets`, `phase` | done |
| Measurement and quantum-to-classical transition | `measure wires [as "label"]` | done |
| SWAP | `swap a, b` | done |
| Automatic placement and barriers | scheduler and `barrier [wires]` | done |
| Wire bundles/slashes | `bundle "count" on wire` | done |
| Quantum/classical/off changes and START/END | `set`, `start`, `end` | partial — active-wire defaults and qpic's late START placement remain |
| Ellipsis wires and IN/OUT value bullets | structured wire/value statements | next |
| Portable dimensions, colors, fill, dash, and opacity | trailing `with` properties | done |
| Box, circle, ellipse, and unboxed shapes | `with shape: ...` | done |
| Custom target operators and hyperlinks | typed operator/link values | next |
| Centered mid-circuit labels | `label "text" [on wires]` | done |
| Per-wire labels and left/right braces | `labels`, `brace` | done |
| Equals shorthand and qpic brace defaults | label/brace sugar | next |
| Comments beside operations | `note` | partial — portable notes occupy scheduled space |
| Marks and highlighted `@` regions | `mark` and `group ... from ... to ...` | partial — named ranges are done; numeric relative ranges remain |
| TOUCH and PHANTOM | `touch`, `space` | partial — PHANTOM subslices remain |
| MIXGATES, LB/LE, explicit time slices | `parallel` blocks | partial — non-overlapping operations align; nested forced-overlap levels remain |
| PERMUTE and persistent wire reordering | `permute` statement | done |
| Repeat/reverse (`R`) | typed `repeat count { ... }`, later marked ranges/reverse | partial |
| CUT separators | source-local `cut` statement | partial — global numbered cut rules remain |
| Global spacing, scale, and background | `layout { ... }` | done |
| Horizontal/vertical orientation | `layout.orientation` | done |
| Global gate size, corners, and comment size | remaining `layout` properties | next |
| Measurement shapes | measurement options | next |
| Custom colors | named colors or quoted `#RRGGBB` values | done |
| DEFINE composition | parsed `fn` declarations, never text macros | partial — nested wire-operation functions are done; value/style declarations remain |
| PREAMBLE/PRETIKZ/POSTTIKZ/HEADER/HYPERTARGET | explicit backend escape blocks | next |
| AUTOWIRES | intentionally replaced by checked declarations | next |

`tests/artifacts.rs` currently compiles representative teleportation, styled/vertical, wire-lifecycle/permutation, nested-function, structured-programming, annotation, and marked-region fixtures to both PDFs. Phase 4 expands this into one parity fixture per upstream golden test and manual example; generated TikZ text is not compared byte-for-byte because qrab has a different frontend and renderer, but every backend artifact must compile and match its circuit-level visual baseline.
