# Grouped Label Enum Documentation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Document the grouped `label` syntax that creates an enum-compatible compile-time frame, then publish the verified integer-overflow and documentation changes.

**Architecture:** Add the explanation only to the `src/main.rs` crate documentation, which is the source of truth for the generated README. Regenerate `README.md`, run the existing verification commands, commit every current project change, and push `dev` to `origin/dev`.

**Tech Stack:** Rust crate documentation, cargo-rdme 1.5.0, Cargo, Git

## Global Constraints

- Do not edit `README.md` directly.
- Preserve the existing integer-overflow implementation and tests.
- Keep the grouped `label` explanation in the existing English style of the `enum` section.
- Do not change parser or runtime behavior.
- Push only after every verification command exits with status 0.

---

### Task 1: Document and publish grouped label enum syntax

**Files:**
- Modify: `src/main.rs:167-209`
- Generate: `README.md`
- Include existing changes: `BUG_REPORT.md`, `src/llvm/arithmetic.rs`, `src/llvm/codegen.rs`, `src/llvm/value.rs`, `tests/src/errors.sprs`, `tests/src/main.sprs`
- Include plan: `docs/superpowers/plans/2026-08-16-grouped-label-enum-doc.md`

**Interfaces:**
- Consumes: Existing grouped declaration syntax `pub label :Color{:red, :blue}` and generated Atom references `Color.red`, `Color.blue`.
- Produces: Generated README text that explains the syntax without changing compiler behavior.

- [x] **Step 1: Add the grouped label explanation and example**

Insert the following text after the existing `enum_match_red` example and before the `struct` section in `src/main.rs`:

````rust
//! Grouped `label` declarations provide enum-compatible syntax for namespaced Atoms.
//! `pub label :Color{:red, :blue}` creates the same kind of compile-time frame as a
//! source `enum`, exports it, and exposes its variants as `Color.red` and `Color.blue`.
//! Both declaration forms produce framed Atom intern keys at runtime.
//!
//! ```sprs
//! pub label :Color{:red, :blue}
//!
//! fn print_grouped_label_color() {
//!   @println(Color.red);
//! }
//! ```
//!
````

- [x] **Step 2: Regenerate the README**

Run:

```bash
cargo rdme --force
```

Expected: exit status 0, and the generated `README.md` contains the grouped `label` paragraph and example in its `enum` section.

- [x] **Step 3: Verify README synchronization**

Run:

```bash
cargo rdme --check
```

Expected: exit status 0 with no diff.

- [x] **Step 4: Run the Rust tests**

Run:

```bash
cargo test
```

Expected: exit status 0 and no failed tests.

- [x] **Step 5: Run the sprs integration project**

Run:

```bash
cargo run -- run --dest tests
```

Expected: exit status 0, four consecutive `true` results at the end of `=== Error Mechanism ===`, and final output `=== All Tests Done ===`.

- [x] **Step 6: Review the documentation diff**

Run:

```bash
git diff --check
git diff -- src/main.rs README.md
```

Expected: no whitespace errors; `README.md` mirrors the crate documentation with only cargo-rdme heading-level adjustments.

- [ ] **Step 7: Commit all verified project changes**

Run:

```bash
git add BUG_REPORT.md README.md src/main.rs src/llvm/arithmetic.rs src/llvm/codegen.rs src/llvm/value.rs tests/src/errors.sprs tests/src/main.sprs docs/superpowers/plans/2026-08-16-grouped-label-enum-doc.md
git commit -m "feat: handle integer overflow with error labels"
```

Expected: one commit containing the integer-overflow implementation, tests, generated documentation, bug-report update, grouped `label` explanation, and this implementation plan.

- [ ] **Step 8: Push the current branch**

Run:

```bash
git push origin dev
```

Expected: exit status 0 and `origin/dev` advances through the design commit and implementation commit.
