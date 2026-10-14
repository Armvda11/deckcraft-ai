# Contributing to DeckCraft AI

DeckCraft AI is a fork of [DeckCraft](https://github.com/storytold/deckcraft). Its public repository
is [Armvda11/deckcraft-ai](https://github.com/Armvda11/deckcraft-ai). Preserve the upstream history,
licenses, and attribution when changing the application.

## Branches and updates

- Keep `main` as the reviewed, working version. Develop on short-lived `feat/`, `fix/`, or `docs/`
  branches and open a pull request into this fork's `main`.
- Use small commits that each explain one coherent change. The repository convention is a milestone
  or task prefix, such as `M2: add provider connection test` or `fix: preserve edited equation`.
- This checkout uses `origin` for `Armvda11/deckcraft-ai` and `upstream` for
  `storytold/deckcraft`. Check `git remote -v` before pushing. Push to `origin` and never to
  `upstream`.
- To bring in upstream changes, fetch `upstream`, create an `upstream-update/` branch from the
  fork's `main`, merge `upstream/main`, resolve conflicts, run the gates, and review the resulting
  pull request.
  Do not force-push shared branches. This keeps upstream changes visible in Git history.
- Use GitHub releases only from a tested commit on the fork's `main`. Packaging and signing need
  the fork's own credentials; upstream release secrets do not transfer to forks.

## Code organization

- `crates/ai` owns provider adapters, source extraction, generation, and the deterministic compiler.
  Keep provider-specific protocol details out of the document model.
- `crates/model` defines saved presentation data. Change it with backward-compatible defaults and
  serialization tests so older `.deckcraft` files still open.
- `crates/engine` owns document commands, revision checks, and undo. Apply AI candidates through
  the same command path as manual edits.
- `crates/ui-egui` owns the AI pane and progress display. Keep network and extraction work in
  background jobs so the editor stays responsive.
- Keep public interfaces and non-obvious invariants documented. Comments should explain *why* a
  constraint exists or a choice is surprising; prefer clear names over comments that restate code.
- Put a regression test beside a bug fix. For features, test behavior at the narrowest useful layer
  and add an integration test when correctness crosses a crate boundary. Avoid tests that only copy
  implementation details.

## Before a pull request

1. Run `cargo fmt --all -- --check` and `git diff --check`.
2. Run `cargo xtask ci` when the local graphics environment supports it. The fork's macOS and Linux
   GitHub Actions run this gate on every push and pull request.
3. For UI, slide layout, animation, or export changes, inspect the actual app or rendered output.
   Record any platform or provider checks that could not be run.
4. Check new assets against `AGENTS.md`, then update `ATTRIBUTION.md` and `NOTICE` if needed.
5. Explain the behavior change, verification, and remaining limits in the pull request. Keep
   credentials, private source documents, generated build outputs, and local recovery files out of
   Git.
