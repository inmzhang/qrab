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
| Quantum/classical/off changes and START/END | `set`, `start`, `end`, active-wire defaults, late start scheduling | done |
| Ellipsis wires and IN/OUT value bullets | `ellipsis` declarations and `set ... as "value"` | done |
| Portable dimensions, colors, fill, dash, and opacity | trailing `with` properties | done |
| Box, circle, ellipse, and unboxed shapes | `with shape: ...` | done |
| Custom target operators and hyperlinks | labeled/shaped gates and checked `with link: "..."` URLs | done |
| Centered mid-circuit labels | `label "text" [on wires]` | done |
| Per-wire labels and left/right braces | `labels`, `brace` | done |
| Equals shorthand and qpic brace defaults | `equals [label] [on wires] [braced side]` | done |
| Comments beside operations | non-consuming `note above|below` annotations | done |
| Marks and highlighted `@` regions | `mark` and `group ... from ... to ...`; named ranges replace fragile numeric offsets | done |
| TOUCH and PHANTOM | `touch`, `space`, and structured slice blocks | done |
| MIXGATES, LB/LE, explicit time slices | safe `parallel` and explicit forced `overlay` blocks | done |
| PERMUTE and persistent wire reordering | `permute` statement | done |
| Repeat/reverse (`R`) | typed `repeat`, `reverse { ... }`, and `reverse from mark to mark` | done |
| CUT separators | source-local `cut` statements replace after-the-fact global slice numbers | done |
| Global spacing, scale, and background | `layout { ... }` | done |
| Horizontal/vertical orientation | `layout.orientation` | done |
| Global gate size, corners, and comment size | `layout.gate_size`, `corner_radius`, `comment_width` | done |
| Measurement shapes | named measurements default to D; `using tag` selects a tag | done |
| Custom colors | named colors or quoted `#RRGGBB` values | done |
| DEFINE composition | parsed `fn`, `let`, and named `style` declarations, never text macros | done |
| PREAMBLE/PRETIKZ/POSTTIKZ/HEADER/HYPERTARGET | isolated `backend latex|typst` escape blocks | done |
| AUTOWIRES | explicit `autowires` opt-in; checked declarations remain the default | done |

`tests/qpic/` contains a checked `.qrab` translation for every one of qpic's 44 golden-test stems, and `tests/qpic-manual/` contains all 64 documented example stems. `tests/artifacts.rs` requires both exact lists and compiles them plus 11 focused qrab examples through both backends: 119 source fixtures and 238 PDFs per artifact run. Fourteen tolerant PDF page-geometry baselines cover representative dense, vertical, annotated, overlaid, imported, lifecycle, and colored-background layouts; generated TikZ text is not compared byte-for-byte because qrab has a different frontend and renderer.
