# Releasing

## Automated flow

1. Use conventional commit subjects and merge changes into `main`. Release-plz maintains a release PR containing the next version and changelog.
2. On the release PR's branch, run `just manual` and commit the result. The man pages and the manual's title page both carry the crate version, so a bump leaves them stale; the asset drift check and `shipped_man_pages_carry_this_crate_version` fail until the pages are regenerated, and nothing at all catches a stale `manual.pdf`. Release-plz edits manifests rather than building the crate, so nothing does this for you.
3. Run `just release-check`, review the release PR, and merge it with a clean CI run.
4. Release-plz publishes the crate and pushes the matching `vX.Y.Z` tag. Cargo-dist reacts to that tag, verifies it matches the crate version, builds all configured targets and installers, and creates the GitHub release.

`release_always` is true, so the release job publishes on any push to `main` whose manifest version is not yet on crates.io. Merging a release PR is the ordinary way that happens; a hand-edited bump publishes just the same. It has to be true because the first release could not come from a PR at all: release-plz opens one only when it has something to change, and the unpublished crate already sat at the version it would have released.

Cargo-dist produces archives for Linux GNU on x86-64 and Arm64, Linux musl on x86-64, macOS on Intel and Apple Silicon, and Windows MSVC on x86-64. Every archive includes the generated completions and man pages in `assets/`.

## Playground gate

The playground is a separate deployment with its own switch. `.github/workflows/playground.yml` builds the WebAssembly module on every push to `main`, but publishes to GitHub Pages only when the repository variable `ENABLE_PLAYGROUND` is `true`. Pages is enabled with the GitHub Actions source and the gate is set, so a push to `main` publishes to <https://inmzhang.com/qrab/>. That page is public even while this repository is private. Building it unconditionally means a change that breaks the WebAssembly build fails CI whether or not the page is live.

## Publish gate

`ENABLE_PUBLISHING` is `true` and `0.1.0` is on crates.io. Setting the variable back to `false` disables the release job—`cargo publish` and tag creation—while leaving release PR generation active.

Publishing authenticates with the `CARGO_REGISTRY_TOKEN` secret, which the first release had to use: crates.io accepts a trusted publisher only for a crate that already exists. Now that `qrab` does, configure trusted publishing for `inmzhang/qrab` and `.github/workflows/release-plz.yml`, then delete both the secret and the `CARGO_REGISTRY_TOKEN` line from `.github/workflows/release-plz.yml`. The release job already requests the `id-token: write` permission that replaces it.

No Homebrew tap is required: the generated formula is attached to each GitHub release and can be installed directly by URL. Add a tap and cargo-dist Homebrew publisher only if a shorter `brew install owner/tap/qrab` command becomes worthwhile.

## Manual archive installation

Each archive expands to `qrab-<target>/`, containing the executable, `assets/`, the README, changelog, and license. From an extracted Unix archive, a per-user Bash installation is:

```sh
mkdir -p ~/.local/bin ~/.local/share/man/man1 ~/.local/share/bash-completion/completions
install -m 0755 qrab ~/.local/bin/qrab
install -m 0644 assets/*.1 ~/.local/share/man/man1/
install -m 0644 assets/qrab.bash ~/.local/share/bash-completion/completions/qrab
```

For another shell, copy its file from `assets/` to that shell's user completion directory. Checksums are published alongside every archive. Local verification never pushes a tag or publishes a package.
