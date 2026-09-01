# Releasing

1. Before the first publication, create the canonical repository, configure its Git remote, and add its URL as `package.repository` in `Cargo.toml`.
2. Update `Cargo.toml` and `CHANGELOG.md`, then commit with a clean worktree.
3. Run `just release-check`. This formats, lints, runs all unit and 238-PDF artifact checks, builds the locked release binary, and verifies the source package.
4. Create and push a tag matching the crate version exactly, for example `v0.1.0`.
5. The release workflow rechecks the tag/version match, builds the Linux archive, writes its SHA-256 checksum, and attaches both files to the GitHub release.

The archive expands to a versioned directory containing `qrab`, `completions/`, and `man/`. From that directory, a per-user Bash installation is:

```sh
mkdir -p ~/.local/bin ~/.local/share/man/man1 ~/.local/share/bash-completion/completions
install -m 0755 qrab ~/.local/bin/qrab
install -m 0644 man/*.1 ~/.local/share/man/man1/
install -m 0644 completions/qrab.bash ~/.local/share/bash-completion/completions/qrab
```

For another shell, copy its file from `completions/` to that shell's user completion directory.

Publishing a tag is intentionally a maintainer action; local verification never pushes or publishes anything.
