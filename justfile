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

# Build the WebAssembly module the playground loads.
playground:
    wasm-pack build playground --target web --out-dir www/pkg --no-typescript

# Serve the playground at http://localhost:8080 after building it.
playground-serve: playground
    python3 -m http.server 8080 --directory playground/www

install-local:
    cargo install --path . --locked

example:
    cargo run -- compile examples/teleportation.qrab
    tectonic examples/teleportation.tex --outdir target
    typst compile examples/teleportation.typ target/teleportation-typst.pdf

install-hooks:
    pre-commit install --install-hooks
