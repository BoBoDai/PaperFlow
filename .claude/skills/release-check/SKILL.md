---
name: release-check
description: Run PaperFlow's pre-commit and release verification: Rust tests, clippy, frontend typecheck/build, optional live tests, and a focused manual smoke test.
---

# Release check

Use before committing a non-trivial change or producing a build.

## Automatic checks

Run from the repo root, in this order. Stop and report the exact failing output if
any step fails; do not commit a red tree.

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
pnpm typecheck
pnpm build
```

Notes:

- `cargo test --workspace` uses fakes and does not hit the network.
- Online tests are ignored by default:
  `cargo test -p paperflow-adapters -- --ignored` and
  `cargo test -p paperflow-usecases -- --ignored`. Run them only when network and
  API limits allow; report skips.
- `pnpm build` may print the existing chunk-size warning for the PDF.js bundle;
  that is not a failure.
- `cargo fmt --all -- --check` is optional; do not reformat unrelated files just to
  make it pass.

## Targeted checks by area

| Changed | Also verify |
|---|---|
| arXiv / OpenAlex parsing | `cargo test -p paperflow-adapters`; one online test if possible |
| LLM prompts / parsing / streaming | `llm/tests.rs`; manual summary + one streamed Q&A |
| Settings / credentials | `cargo test -p paperflow-app`; save, restart, key still works |
| Search / feed / QA budgets | `cargo test -p paperflow-usecases` |
| PDF download / reader | `pdf/tests.rs`; manual open + selection + page/full Q&A |
| IPC command | `pnpm app:dev`, call the new path, plus one error case |
| Styling | `pnpm dev` or `pnpm app:dev`; check dark and light mode |

## Manual smoke test

For any user-visible change, run the actual app:

```bash
pnpm app:dev
```

At minimum:

1. Search an English query and a Chinese query.
2. Load the interest feed and page once.
3. Open a paper → summary appears (streaming if configured).
4. Open the reader, select text, ask a current-page question.
5. If settings changed: save, restart the app, confirm the active provider.

## Commit

Only after the checks pass:

- Commit on `main`.
- One-line English commit message.
- Do not include generated build output, logs, API keys, or `credentials.json`.
- Keep the working tree clean before reporting done.
