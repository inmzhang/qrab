# Contributing

Install stable Rust, Tectonic, Typst 0.15.1, Poppler (`pdfinfo`), and `just`, then enable the tracked hook:

```sh
just install-hooks
just ci
```

Keep portable behavior in the shared AST and add one focused regression test for parser or scheduler changes. Renderer changes must compile in both artifact backends. Commits should be small and describe one completed behavior.

See [docs/releasing.md](docs/releasing.md) for the tag release checklist.
