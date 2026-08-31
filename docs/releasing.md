# Releasing

1. Update `Cargo.toml` and `CHANGELOG.md`, then commit with a clean worktree.
2. Run `just release-check`. This formats, lints, runs all unit and 238-PDF artifact checks, builds the locked release binary, and verifies the source package.
3. Create and push a tag matching the crate version exactly, for example `v0.1.0`.
4. The release workflow rechecks the tag/version match, builds the Linux archive, writes its SHA-256 checksum, and attaches both files to the GitHub release.

Publishing a tag is intentionally a maintainer action; local verification never pushes or publishes anything.
