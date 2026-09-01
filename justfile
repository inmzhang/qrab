default: check

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

lint:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

test:
    cargo nextest run --workspace --all-targets
    cargo test --doc

test-artifacts:
    cargo nextest run --test artifacts --run-ignored ignored-only

check: fmt-check lint test

ci: check test-artifacts

package:
    cargo package --locked

release-check: ci
    cargo build --release --locked
    cargo package --locked

gen-assets:
    cargo run --locked --quiet -p xtask

# Regenerate the manual's diagrams and typeset it.
manual: gen-assets
    typst compile --root . docs/manual.typ docs/manual.pdf

# Build the WebAssembly module the playground loads.
playground:
    wasm-pack build playground --target web --out-dir www/pkg --no-typescript

# Serve the playground at http://localhost:8080 after building it.
playground-serve: playground
    python3 -m http.server 8080 --directory playground/www

install-local:
    cargo install --path . --locked

# Compile one example through every backend, writing to target/ so nothing
# generated lands next to the sources.
example:
    mkdir -p target/example
    cargo run -- compile examples/teleportation.qrab -t latex -o target/example/teleportation.tex
    cargo run -- compile examples/teleportation.qrab -t typst -o target/example/teleportation.typ
    cargo run -- compile examples/teleportation.qrab -t svg -o target/example/teleportation.svg
    tectonic target/example/teleportation.tex --outdir target/example
    typst compile target/example/teleportation.typ target/example/teleportation-typst.pdf

install-hooks:
    pre-commit install --install-hooks
