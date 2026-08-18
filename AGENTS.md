# Agent conventions

`docs/src/` is the source of truth for user-facing Sprs documentation.

When a change alters user-visible syntax, types, operators, macros, ownership, errors, CLI, or runtime contracts, update the matching chapter in the same change. Add new chapters to `docs/src/SUMMARY.md`. Changes to the CLI, `sprs.toml`, or diagnostic codes must update Getting Started, [Project Config](docs/src/reference/project-config.md), and [Compiler Errors](docs/src/reference/compiler-errors.md) in the same change.

Keep `README.md` as a project entry point only. Do not put the language specification or the runtime internals table there.

`docs/book/` is generated output. Do not commit it.

Verify documentation with:

```bash
mdbook build docs
```
