# Agent conventions

`docs/en/src/` is the source of truth for user-facing Sprs documentation. `docs/ja/src/` is the Japanese translation and must stay in sync in the same change.

When a change alters user-visible syntax, types, operators, macros, ownership, errors, CLI, or runtime contracts, update the matching chapter in both `docs/en/src/` and `docs/ja/src/` in the same change. Add new chapters to both `docs/en/src/SUMMARY.md` and `docs/ja/src/SUMMARY.md`. Changes to the CLI, `sprs.toml`, or diagnostic codes must update Getting Started, [Project Config](docs/en/src/reference/project-config.md), and [Compiler Errors](docs/en/src/reference/compiler-errors.md) (and the same relative paths under `docs/ja/src/`) in the same change.

Keep `README.md` as a project entry point only. Do not put the language specification or the runtime internals table there.

`docs/book/` is generated output. Do not commit it.

Verify documentation with:

```bash
diff \
  <(find docs/en/src -type f -name '*.md' -printf '%P\n' | sort) \
  <(find docs/ja/src -type f -name '*.md' -printf '%P\n' | sort)
mdbook build docs/en
mdbook build docs/ja
```
