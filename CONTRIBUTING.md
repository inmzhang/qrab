# Contributing

Install stable Rust, Tectonic, Typst 0.15.1, Poppler (`pdfinfo`), [`pre-commit`](https://pre-commit.com), and `just`, then enable the tracked hooks:

```sh
just install-hooks
just ci
```

`just install-hooks` installs both a `pre-commit` hook (formatting, linting, and file hygiene) and a `commit-msg` hook that enforces Conventional Commits, which release-plz and git-cliff use to derive versions and changelog entries. The full test and artifact suites are not part of the hook; run `just ci` before opening a pull request.

Keep portable behavior in the shared AST and add one focused regression test for parser or scheduler changes. Renderer changes must compile in all three backends. Commits should be small and describe one completed behavior.

Generated files are committed and CI fails on any drift: `just gen-assets` rebuilds the shell completions, man pages, README diagrams, and the manual's diagrams, and `just manual` also typesets `docs/manual.pdf`. A new language construct belongs in the manual: drop a small circuit into `docs/manual/examples/`, which is rendered automatically, and describe it in `docs/manual.typ` beside the others.

See [docs/releasing.md](docs/releasing.md) for the tag release checklist.
