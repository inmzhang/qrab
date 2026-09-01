# Releasing

## Automated flow

1. Use conventional commit subjects and merge changes into `main`. Release-plz maintains a release PR containing the next version and changelog.
2. Run `just release-check`, review the release PR, and merge it with a clean CI run.
3. With publishing enabled, release-plz publishes the crate and creates the matching `vX.Y.Z` tag. Cargo-dist reacts to that tag, verifies it matches the crate version, builds all configured targets and installers, and creates the GitHub release.
4. After the GitHub release succeeds, cargo-dist publishes the generated npm package.

Cargo-dist produces archives for Linux GNU on x86-64 and Arm64, Linux musl on x86-64, macOS on Intel and Apple Silicon, and Windows MSVC on x86-64. Every archive includes the generated completions and man pages in `assets/`.

## Playground gate

The playground is a separate deployment with its own switch. `.github/workflows/playground.yml` builds the WebAssembly module on every push to `main`, but publishes to GitHub Pages only when the repository variable `ENABLE_PLAYGROUND` is `true`. Enable Pages with the GitHub Actions source, then run `gh variable set ENABLE_PLAYGROUND --body true`. Building it unconditionally means a change that breaks the WebAssembly build fails CI whether or not the page is live.

## Publish gate

The repository variable `ENABLE_PUBLISHING` is currently `false`. The release-plz release job—including `cargo publish` and tag creation—does not run until it is set to `true`; release PR generation remains active.

Before opening the gate:

1. Make the repository public.
2. Configure crates.io trusted publishing for `inmzhang/qrab` and `.github/workflows/release-plz.yml`.
3. Add an npm publish access token as the GitHub Actions secret `NPM_TOKEN`.
4. Set the gate with `gh variable set ENABLE_PUBLISHING --body true`.

For the first public release, no release PR exists because the unpublished crate is already at `0.1.0` with an up-to-date changelog. After opening the gate, run `gh workflow run release-plz.yml`; release-plz will publish and tag `0.1.0`, then cargo-dist takes over. Later releases use the normal release-PR merge flow above.

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
